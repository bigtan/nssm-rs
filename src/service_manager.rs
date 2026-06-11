use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use log::{debug, info, warn};
use windows::Win32::Foundation::{ERROR_SERVICE_NOT_ACTIVE, ERROR_SERVICE_SPECIFIC_ERROR};
use windows::Win32::System::Registry::{KEY_READ, KEY_WRITE};
use windows::Win32::System::Services::*;
use windows::core::PCWSTR;

use windows::core::PWSTR;

use crate::config::{ProcessPriority, ServiceConfig, ServiceStartType};
use crate::error::{AppError, AppResult};
use crate::parameters::ServiceParameter;
use crate::registry::{RegistryKey, to_wide};

const SERVICES_ROOT: &str = "SYSTEM\\CurrentControlSet\\Services";
const PARAMETERS_SUBKEY: &str = "Parameters";

/// Standard DELETE access right, needed for DeleteService.
const DELETE_ACCESS: u32 = 0x0001_0000;

const START_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_TIMEOUT: Duration = Duration::from_secs(60);

/// Polls the service status until it reaches `target`, the service stops
/// unexpectedly while waiting for SERVICE_RUNNING, or the timeout expires.
fn wait_for_service_state(
    service_handle: SC_HANDLE,
    target: SERVICE_STATUS_CURRENT_STATE,
    timeout: Duration,
) -> AppResult<()> {
    let start = std::time::Instant::now();
    loop {
        let mut status = SERVICE_STATUS::default();
        unsafe { QueryServiceStatus(service_handle, &mut status) }?;

        if status.dwCurrentState == target {
            return Ok(());
        }
        if target == SERVICE_RUNNING && status.dwCurrentState == SERVICE_STOPPED {
            let exit_code = if status.dwWin32ExitCode == ERROR_SERVICE_SPECIFIC_ERROR.0 {
                status.dwServiceSpecificExitCode
            } else {
                status.dwWin32ExitCode
            };
            return Err(AppError::Message(format!(
                "Service stopped during startup (exit code {exit_code})"
            )));
        }
        if start.elapsed() >= timeout {
            return Err(AppError::Message(format!(
                "Timed out waiting for service state {} (current state {})",
                target.0, status.dwCurrentState.0
            )));
        }

        thread::sleep(Duration::from_millis(250));
    }
}

pub struct ServiceManager {
    handle: SC_HANDLE,
}

impl ServiceManager {
    /// Connects to the SCM with the minimal rights needed for everything
    /// except service creation. Per-service rights are requested when the
    /// individual service handle is opened.
    pub fn new() -> AppResult<Self> {
        Self::with_access(SC_MANAGER_CONNECT)
    }

    /// Connects with the additional right required by CreateServiceW.
    pub fn new_for_install() -> AppResult<Self> {
        Self::with_access(SC_MANAGER_CONNECT | SC_MANAGER_CREATE_SERVICE)
    }

    fn with_access(access: u32) -> AppResult<Self> {
        debug!("Creating new ServiceManager instance (access 0x{access:x})");
        let handle = unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), access) }?;
        Ok(Self { handle })
    }

    pub fn install_service(
        &self,
        service_name: &str,
        application: &Path,
        arguments: &[String],
    ) -> AppResult<()> {
        info!("Creating service configuration for '{service_name}'");

        let application = std::path::absolute(application)?;
        if !application.is_file() {
            warn!(
                "Application '{}' does not exist (yet); the service will fail to start until it does",
                application.display()
            );
        }

        let config = ServiceConfig {
            app_directory: Some(
                application
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf(),
            ),
            app_parameters: (!arguments.is_empty())
                .then(|| crate::cmdline::join_arguments(arguments)),
            application,
            ..Default::default()
        };

        self.create_service(service_name, &config)
    }

    pub fn create_service(&self, service_name: &str, config: &ServiceConfig) -> AppResult<()> {
        let nssm_path = std::env::current_exe()?;
        let service_command = format!(
            "\"{}\" run {}",
            nssm_path.to_string_lossy(),
            crate::cmdline::quote_argument(service_name)
        );

        let service_name_wide = to_wide(service_name);
        let display_name = config.display_name.as_deref().unwrap_or(service_name);
        let display_name_wide = to_wide(display_name);
        let service_command_wide = to_wide(&service_command);

        let service_handle = unsafe {
            CreateServiceW(
                self.handle,
                PCWSTR::from_raw(service_name_wide.as_ptr()),
                PCWSTR::from_raw(display_name_wide.as_ptr()),
                SERVICE_ALL_ACCESS,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_START_TYPE(config.start_type.to_windows_value()),
                SERVICE_ERROR_NORMAL,
                PCWSTR::from_raw(service_command_wide.as_ptr()),
                PCWSTR::null(),
                None,
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
            )
        }?;

        unsafe {
            let _ = CloseServiceHandle(service_handle);
        }

        if let Some(description) = &config.description {
            self.set_scm_description(service_name, description)?;
        }

        self.save_service_config(service_name, config)?;
        info!("Service '{service_name}' installed successfully");
        Ok(())
    }

    pub fn remove_service(&self, service_name: &str, confirm: bool) -> AppResult<()> {
        if !confirm {
            println!("Are you sure you want to remove service '{service_name}'? (y/N)");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().to_lowercase().starts_with('y') {
                info!("Service removal cancelled by user");
                return Ok(());
            }
        }

        if let Err(error) = self.stop_service(service_name) {
            warn!("Could not stop service before removal: {error}");
        }

        self.with_service_handle(service_name, DELETE_ACCESS, |service_handle| unsafe {
            DeleteService(service_handle)?;
            Ok(())
        })?;

        self.remove_service_config(service_name)?;
        info!("Service '{service_name}' removed successfully");
        Ok(())
    }

    pub fn start_service(&self, service_name: &str) -> AppResult<()> {
        self.with_service_handle(
            service_name,
            SERVICE_START | SERVICE_QUERY_STATUS,
            |service_handle| {
                unsafe { StartServiceW(service_handle, None) }?;
                wait_for_service_state(service_handle, SERVICE_RUNNING, START_TIMEOUT)
            },
        )?;

        info!("Service '{service_name}' started successfully");
        Ok(())
    }

    pub fn stop_service(&self, service_name: &str) -> AppResult<()> {
        let was_running = self.with_service_handle(
            service_name,
            SERVICE_STOP | SERVICE_QUERY_STATUS,
            |service_handle| {
                let mut status = SERVICE_STATUS::default();
                match unsafe { ControlService(service_handle, SERVICE_CONTROL_STOP, &mut status) } {
                    Ok(()) => {}
                    Err(error) if error.code() == ERROR_SERVICE_NOT_ACTIVE.to_hresult() => {
                        return Ok(false);
                    }
                    Err(error) => return Err(error.into()),
                }
                wait_for_service_state(service_handle, SERVICE_STOPPED, STOP_TIMEOUT)?;
                Ok(true)
            },
        )?;

        if was_running {
            info!("Service '{service_name}' stopped successfully");
        } else {
            info!("Service '{service_name}' is not running");
        }
        Ok(())
    }

    pub fn restart_service(&self, service_name: &str) -> AppResult<()> {
        self.stop_service(service_name)?;
        self.start_service(service_name)
    }

    pub fn set_service_parameter(
        &self,
        service_name: &str,
        parameter: &str,
        values: &[String],
    ) -> AppResult<()> {
        let parameter = ServiceParameter::parse(parameter)?;
        match parameter {
            ServiceParameter::Start => {
                let value = single_value(parameter, values)?;
                let start_type = ServiceStartType::from_str(value).ok_or_else(|| {
                    AppError::InvalidParameterValue {
                        parameter: parameter.as_str().to_string(),
                        value: value.to_string(),
                    }
                })?;
                self.set_scm_start_type(service_name, start_type)?;
            }
            ServiceParameter::DisplayName => {
                self.set_scm_display_name(service_name, single_value(parameter, values)?)?;
            }
            ServiceParameter::Description => {
                self.set_scm_description(service_name, single_value(parameter, values)?)?;
            }
            ServiceParameter::AppEnvironmentExtra => {
                // A single empty value clears the list.
                let entries: Vec<String> = match values {
                    [value] if value.is_empty() => Vec::new(),
                    _ => values.to_vec(),
                };
                for entry in &entries {
                    if !entry.contains('=') {
                        return Err(AppError::InvalidParameterValue {
                            parameter: parameter.as_str().to_string(),
                            value: entry.clone(),
                        });
                    }
                }
                let mut config = self.load_service_config(service_name)?;
                config.app_environment_extra = entries;
                self.save_service_config(service_name, &config)?;
            }
            ServiceParameter::AppParameters if values.len() > 1 => {
                let joined = crate::cmdline::join_arguments(values);
                let mut config = self.load_service_config(service_name)?;
                parameter.apply(&mut config, &joined)?;
                self.save_service_config(service_name, &config)?;
            }
            _ => {
                let value = single_value(parameter, values)?;
                let mut config = self.load_service_config(service_name)?;
                parameter.apply(&mut config, value)?;
                self.save_service_config(service_name, &config)?;
            }
        }

        info!(
            "Parameter '{}' set to '{}' for service '{}'",
            parameter.as_str(),
            values.join(" "),
            service_name
        );
        Ok(())
    }

    pub fn get_service_parameter(&self, service_name: &str, parameter: &str) -> AppResult<String> {
        let parameter = ServiceParameter::parse(parameter)?;
        let value = match parameter {
            ServiceParameter::Start => {
                let (start_type, _) = self.query_scm_config(service_name)?;
                match ServiceStartType::from_windows_value(start_type) {
                    Some(start_type) => start_type.as_cli_value().to_string(),
                    None => start_type.to_string(),
                }
            }
            ServiceParameter::DisplayName => self.query_scm_config(service_name)?.1,
            ServiceParameter::Description => self.query_scm_description(service_name)?,
            _ => {
                let config = self.load_service_config(service_name)?;
                parameter.read(&config)
            }
        };
        println!("{}: {}", parameter.as_str(), value);
        Ok(value)
    }

    fn set_scm_start_type(
        &self,
        service_name: &str,
        start_type: ServiceStartType,
    ) -> AppResult<()> {
        self.with_service_handle(service_name, SERVICE_CHANGE_CONFIG, |handle| unsafe {
            ChangeServiceConfigW(
                handle,
                ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
                SERVICE_START_TYPE(start_type.to_windows_value()),
                SERVICE_ERROR(SERVICE_NO_CHANGE),
                PCWSTR::null(),
                PCWSTR::null(),
                None,
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
            )?;
            Ok(())
        })
    }

    fn set_scm_display_name(&self, service_name: &str, display_name: &str) -> AppResult<()> {
        let display_name_wide = to_wide(display_name);
        self.with_service_handle(service_name, SERVICE_CHANGE_CONFIG, |handle| unsafe {
            ChangeServiceConfigW(
                handle,
                ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
                SERVICE_START_TYPE(SERVICE_NO_CHANGE),
                SERVICE_ERROR(SERVICE_NO_CHANGE),
                PCWSTR::null(),
                PCWSTR::null(),
                None,
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::from_raw(display_name_wide.as_ptr()),
            )?;
            Ok(())
        })
    }

    fn set_scm_description(&self, service_name: &str, description: &str) -> AppResult<()> {
        let description_wide = to_wide(description);
        self.with_service_handle(service_name, SERVICE_CHANGE_CONFIG, |handle| unsafe {
            let info = SERVICE_DESCRIPTIONW {
                lpDescription: PWSTR::from_raw(description_wide.as_ptr() as *mut u16),
            };
            ChangeServiceConfig2W(
                handle,
                SERVICE_CONFIG_DESCRIPTION,
                Some(&info as *const _ as *const core::ffi::c_void),
            )?;
            Ok(())
        })
    }

    /// Returns the raw SCM start type value and the display name.
    fn query_scm_config(&self, service_name: &str) -> AppResult<(u32, String)> {
        self.with_service_handle(service_name, SERVICE_QUERY_CONFIG, |handle| unsafe {
            let mut needed = 0u32;
            let _ = QueryServiceConfigW(handle, None, 0, &mut needed);
            // Use u64 storage so the buffer is aligned for QUERY_SERVICE_CONFIGW.
            let mut buffer = vec![0u64; needed.div_ceil(8) as usize];
            QueryServiceConfigW(
                handle,
                Some(buffer.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW),
                needed,
                &mut needed,
            )?;
            let config = &*(buffer.as_ptr() as *const QUERY_SERVICE_CONFIGW);
            Ok((config.dwStartType.0, pwstr_to_string(config.lpDisplayName)))
        })
    }

    fn query_scm_description(&self, service_name: &str) -> AppResult<String> {
        self.with_service_handle(service_name, SERVICE_QUERY_CONFIG, |handle| unsafe {
            let mut needed = 0u32;
            let _ = QueryServiceConfig2W(handle, SERVICE_CONFIG_DESCRIPTION, None, &mut needed);
            let mut buffer = vec![0u64; needed.div_ceil(8) as usize];
            let bytes =
                std::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u8, needed as usize);
            QueryServiceConfig2W(handle, SERVICE_CONFIG_DESCRIPTION, Some(bytes), &mut needed)?;
            let info = &*(buffer.as_ptr() as *const SERVICE_DESCRIPTIONW);
            Ok(pwstr_to_string(info.lpDescription))
        })
    }

    fn save_service_config(&self, service_name: &str, config: &ServiceConfig) -> AppResult<()> {
        save_service_config(service_name, config)
    }

    fn load_service_config(&self, service_name: &str) -> AppResult<ServiceConfig> {
        load_service_config(service_name)
    }
    fn remove_service_config(&self, service_name: &str) -> AppResult<()> {
        match RegistryKey::delete_tree_local_machine(&parameters_key_path(service_name)) {
            Ok(()) => Ok(()),
            Err(error) => {
                warn!("Failed to delete registry configuration: {error}");
                Ok(())
            }
        }
    }
}

pub fn save_service_config(service_name: &str, config: &ServiceConfig) -> AppResult<()> {
    let key = RegistryKey::create_local_machine(&parameters_key_path(service_name), KEY_WRITE)?;

    key.set_string("Application", &config.application.to_string_lossy())?;
    set_or_delete_path(&key, "AppDirectory", config.app_directory.as_ref())?;
    set_or_delete_string(&key, "AppParameters", config.app_parameters.as_deref())?;
    key.set_dword("AppPriority", config.app_priority.to_windows_value())?;
    key.set_dword("AppNoConsole", u32::from(config.app_no_console))?;
    key.set_dword("AppThrottle", config.app_throttle)?;
    key.set_dword("AppStopMethodSkip", config.app_stop_method_skip)?;
    key.set_dword("AppStopMethodConsole", config.app_stop_method_console)?;
    key.set_dword("AppStopMethodWindow", config.app_stop_method_window)?;
    key.set_dword("AppStopMethodThreads", config.app_stop_method_threads)?;
    key.set_dword("AppRestartDelay", config.app_restart_delay)?;
    key.set_string(
        "AppExitDefault",
        config.app_exit_default.as_registry_value(),
    )?;
    set_or_delete_path(&key, "AppStdout", config.app_stdout.as_ref())?;
    set_or_delete_path(&key, "AppStderr", config.app_stderr.as_ref())?;
    set_or_delete_path(&key, "AppStdin", config.app_stdin.as_ref())?;
    if config.app_environment_extra.is_empty() {
        key.delete_value("AppEnvironmentExtra")?;
    } else {
        key.set_multi_string("AppEnvironmentExtra", &config.app_environment_extra)?;
    }

    Ok(())
}

pub fn load_service_config(service_name: &str) -> AppResult<ServiceConfig> {
    let key = RegistryKey::open_local_machine(&parameters_key_path(service_name), KEY_READ)?;
    let mut config = ServiceConfig::default();

    if let Some(value) = key.get_string("Application")? {
        config.application = PathBuf::from(value);
    }
    if let Some(value) = key.get_string("AppDirectory")? {
        config.app_directory = (!value.is_empty()).then(|| PathBuf::from(value));
    }
    if let Some(value) = key.get_string("AppParameters")? {
        config.app_parameters = (!value.is_empty()).then_some(value);
    }
    if let Some(value) = key.get_dword("AppPriority")? {
        config.app_priority = ProcessPriority::from_windows_value(value);
    }
    if let Some(value) = key.get_dword("AppNoConsole")? {
        config.app_no_console = value != 0;
    }
    if let Some(value) = key.get_dword("AppThrottle")? {
        config.app_throttle = value;
    }
    if let Some(value) = key.get_dword("AppStopMethodSkip")? {
        config.app_stop_method_skip = value;
    }
    if let Some(value) = key.get_dword("AppStopMethodConsole")? {
        config.app_stop_method_console = value;
    }
    if let Some(value) = key.get_dword("AppStopMethodWindow")? {
        config.app_stop_method_window = value;
    }
    if let Some(value) = key.get_dword("AppStopMethodThreads")? {
        config.app_stop_method_threads = value;
    }
    if let Some(value) = key.get_dword("AppRestartDelay")? {
        config.app_restart_delay = value;
    }
    if let Some(value) = key.get_string("AppExitDefault")?
        && let Some(exit_action) = crate::config::ExitAction::from_str(&value)
    {
        config.app_exit_default = exit_action;
    }
    if let Some(value) = key.get_string("AppStdout")? {
        config.app_stdout = (!value.is_empty()).then(|| PathBuf::from(value));
    }
    if let Some(value) = key.get_string("AppStderr")? {
        config.app_stderr = (!value.is_empty()).then(|| PathBuf::from(value));
    }
    if let Some(value) = key.get_string("AppStdin")? {
        config.app_stdin = (!value.is_empty()).then(|| PathBuf::from(value));
    }
    if let Some(values) = key.get_multi_string("AppEnvironmentExtra")? {
        config.app_environment_extra = values;
    }

    Ok(config)
}

impl ServiceManager {
    pub fn query_service_status(&self, service_name: &str) -> AppResult<()> {
        self.with_service_handle(
            service_name,
            SERVICE_QUERY_STATUS,
            |service_handle| unsafe {
                let mut status = SERVICE_STATUS::default();
                QueryServiceStatus(service_handle, &mut status)?;

                let state_str = match status.dwCurrentState.0 {
                    1 => "STOPPED",
                    2 => "START_PENDING",
                    3 => "STOP_PENDING",
                    4 => "RUNNING",
                    5 => "CONTINUE_PENDING",
                    6 => "PAUSE_PENDING",
                    7 => "PAUSED",
                    state => return Err(AppError::InvalidServiceState(state)),
                };

                println!("Service Name: {service_name}");
                println!("State: {state_str}");
                println!("Exit Code: {}", status.dwWin32ExitCode);
                println!(
                    "Service Specific Exit Code: {}",
                    status.dwServiceSpecificExitCode
                );
                println!("Checkpoint: {}", status.dwCheckPoint);
                println!("Wait Hint: {}ms", status.dwWaitHint);
                Ok(())
            },
        )
    }

    pub fn list_nssm_services(&self) -> AppResult<()> {
        let services = RegistryKey::open_local_machine(SERVICES_ROOT, KEY_READ)?.enum_subkeys()?;

        println!("Services managed by nssm-rs:");
        let mut found_any = false;

        for service_name in services {
            if self.has_nssm_config(&service_name) {
                println!("  {service_name}");
                found_any = true;
            }
        }

        if !found_any {
            println!("  (none)");
        }

        Ok(())
    }

    fn has_nssm_config(&self, service_name: &str) -> bool {
        RegistryKey::open_local_machine(&parameters_key_path(service_name), KEY_READ)
            .and_then(|key| key.get_string("Application"))
            .map(|value| value.is_some())
            .unwrap_or(false)
    }

    fn with_service_handle<F, T>(
        &self,
        service_name: &str,
        access: u32,
        callback: F,
    ) -> AppResult<T>
    where
        F: FnOnce(SC_HANDLE) -> AppResult<T>,
    {
        let service_name_wide = to_wide(service_name);
        let service_handle = unsafe {
            OpenServiceW(
                self.handle,
                PCWSTR::from_raw(service_name_wide.as_ptr()),
                access,
            )
        }?;

        let result = callback(service_handle);
        unsafe {
            let _ = CloseServiceHandle(service_handle);
        }
        result
    }
}

impl Drop for ServiceManager {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseServiceHandle(self.handle);
        }
    }
}

fn pwstr_to_string(value: PWSTR) -> String {
    if value.is_null() {
        String::new()
    } else {
        unsafe { value.to_string().unwrap_or_default() }
    }
}

fn parameters_key_path(service_name: &str) -> String {
    format!("{SERVICES_ROOT}\\{service_name}\\{PARAMETERS_SUBKEY}")
}

fn single_value<'a>(parameter: ServiceParameter, values: &'a [String]) -> AppResult<&'a str> {
    match values {
        [value] => Ok(value),
        _ => Err(AppError::Message(format!(
            "Parameter '{}' expects exactly one value, got {}",
            parameter.as_str(),
            values.len()
        ))),
    }
}

fn set_or_delete_path(key: &RegistryKey, name: &str, value: Option<&PathBuf>) -> AppResult<()> {
    match value {
        Some(path) => key.set_string(name, &path.to_string_lossy()),
        None => key.delete_value(name),
    }
}

fn set_or_delete_string(key: &RegistryKey, name: &str, value: Option<&str>) -> AppResult<()> {
    match value {
        Some(value) => key.set_string(name, value),
        None => key.delete_value(name),
    }
}
