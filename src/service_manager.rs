use crate::cli::{ServiceConfig, ServiceStartType, ProcessPriority};
use log::{debug, error, info, warn};
use std::path::PathBuf;
use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Services::*;
use windows::Win32::System::Registry::*;

pub struct ServiceManager {
    handle: SC_HANDLE,
}

impl ServiceManager {
    pub fn new() -> Result<Self, String> {
        debug!("Creating new ServiceManager instance");
        unsafe {
            let handle = OpenSCManagerW(
                PCWSTR::null(),
                PCWSTR::null(),
                SC_MANAGER_ALL_ACCESS,
            ).map_err(|e| {
                error!("Failed to open service control manager: {}", e);
                format!("Failed to open service control manager: {}", e)
            })?;

            info!("ServiceManager created successfully");
            Ok(ServiceManager { handle })
        }
    }

    pub fn install_service(
        &self,
        service_name: &str,
        application: &PathBuf,
        arguments: &[String],
    ) -> Result<(), String> {
        info!("Creating service configuration for '{}'", service_name);
        debug!("Application: {:?}", application);
        debug!("Arguments: {:?}", arguments);
        
        let config = ServiceConfig {
            application: application.clone(),
            app_directory: Some(application.parent().unwrap_or_else(|| std::path::Path::new(".")).to_path_buf()),
            app_parameters: if arguments.is_empty() {
                None
            } else {
                Some(arguments.join(" "))
            },
            ..Default::default()
        };

        debug!("Service configuration created: {:?}", config);
        self.create_service(service_name, &config)
    }

    pub fn create_service(&self, service_name: &str, config: &ServiceConfig) -> Result<(), String> {
        info!("Creating Windows service '{}'", service_name);
        debug!("Service configuration: {:?}", config);
        
        unsafe {
            // Get current executable path (nssm-rs.exe)
            let nssm_path = std::env::current_exe()
                .map_err(|e| {
                    error!("Failed to get current executable path: {}", e);
                    format!("Failed to get current executable path: {}", e)
                })?;

            debug!("NSSM-RS executable path: {:?}", nssm_path);

            // Construct the service command line
            let service_command = format!(
                "\"{}\" run {}",
                nssm_path.to_string_lossy(),
                service_name
            );

            debug!("Service command line: {}", service_command);

            let service_name_wide: Vec<u16> = service_name.encode_utf16().chain(std::iter::once(0)).collect();
            let default_display_name = service_name.to_string();
            let display_name = config.display_name.as_ref().unwrap_or(&default_display_name);
            let display_name_wide: Vec<u16> = display_name.encode_utf16().chain(std::iter::once(0)).collect();
            let service_command_wide: Vec<u16> = service_command.encode_utf16().chain(std::iter::once(0)).collect();

            let service_handle = CreateServiceW(
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
            ).map_err(|e| {
                error!("Failed to create Windows service '{}': {}", service_name, e);
                format!("Failed to create service: {}", e)
            })?;

            info!("Windows service '{}' created successfully", service_name);
            let _ = CloseServiceHandle(service_handle);
        }

        // Save service configuration to registry
        info!("Saving service configuration to registry");
        self.save_service_config(service_name, config)?;

        info!("Service '{}' installed successfully", service_name);
        Ok(())
    }

    pub fn remove_service(&self, service_name: &str, confirm: bool) -> Result<(), String> {
        info!("Attempting to remove service '{}'", service_name);
        
        if !confirm {
            println!("Are you sure you want to remove service '{}'? (y/N)", service_name);
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).map_err(|e| {
                error!("Failed to read user input: {}", e);
                format!("Failed to read input: {}", e)
            })?;
            if !input.trim().to_lowercase().starts_with('y') {
                info!("Service removal cancelled by user");
                return Ok(());
            }
        }

        unsafe {
            let service_name_wide: Vec<u16> = service_name.encode_utf16().chain(std::iter::once(0)).collect();
            
            let service_handle = OpenServiceW(
                self.handle,
                PCWSTR::from_raw(service_name_wide.as_ptr()),
                SERVICE_ALL_ACCESS,
            ).map_err(|e| format!("Failed to open service '{}': {}", service_name, e))?;

            DeleteService(service_handle).map_err(|e| format!("Failed to delete service '{}': {}", service_name, e))?;
            let _ = CloseServiceHandle(service_handle);
        }

        // Remove service configuration from registry
        self.remove_service_config(service_name)?;

        info!("Service '{}' removed successfully", service_name);
        Ok(())
    }

    pub fn start_service(&self, service_name: &str) -> Result<(), String> {
        unsafe {
            let service_name_wide: Vec<u16> = service_name.encode_utf16().chain(std::iter::once(0)).collect();
            
            let service_handle = OpenServiceW(
                self.handle,
                PCWSTR::from_raw(service_name_wide.as_ptr()),
                SERVICE_START,
            ).map_err(|e| format!("Failed to open service '{}': {}", service_name, e))?;

            StartServiceW(service_handle, None).map_err(|e| format!("Failed to start service '{}': {}", service_name, e))?;
            let _ = CloseServiceHandle(service_handle);
        }

        info!("Service '{}' started successfully", service_name);
        Ok(())
    }

    pub fn stop_service(&self, service_name: &str) -> Result<(), String> {
        unsafe {
            let service_name_wide: Vec<u16> = service_name.encode_utf16().chain(std::iter::once(0)).collect();
            
            let service_handle = OpenServiceW(
                self.handle,
                PCWSTR::from_raw(service_name_wide.as_ptr()),
                SERVICE_STOP,
            ).map_err(|e| format!("Failed to open service '{}': {}", service_name, e))?;

            let mut status = SERVICE_STATUS::default();
            ControlService(service_handle, SERVICE_CONTROL_STOP, &mut status)
                .map_err(|e| format!("Failed to stop service '{}': {}", service_name, e))?;
            let _ = CloseServiceHandle(service_handle);
        }

        info!("Service '{}' stopped successfully", service_name);
        Ok(())
    }

    pub fn set_service_parameter(&self, service_name: &str, parameter: &str, value: &str) -> Result<(), String> {
        let mut config = self.load_service_config(service_name)?;

        match parameter.to_uppercase().as_str() {
            "APPLICATION" => {
                config.application = PathBuf::from(value);
            }
            "APPDIRECTORY" => {
                config.app_directory = if value.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(value))
                };
            }
            "APPPARAMETERS" => {
                config.app_parameters = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            "DISPLAYNAME" => {
                config.display_name = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            "DESCRIPTION" => {
                config.description = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            "START" => {
                config.start_type = ServiceStartType::from_str(value)
                    .ok_or_else(|| format!("Invalid start type: {}", value))?;
            }
            "APPPRIORITY" => {
                config.app_priority = ProcessPriority::from_str(value)
                    .ok_or_else(|| format!("Invalid priority: {}", value))?;
            }
            "APPNOCONSOLE" => {
                config.app_no_console = value != "0";
            }
            "APPTHROTTLE" => {
                config.app_throttle = value.parse()
                    .map_err(|_| format!("Invalid throttle value: {}", value))?;
            }
            "APPSTDOUT" => {
                config.app_stdout = if value.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(value))
                };
            }
            "APPSTDERR" => {
                config.app_stderr = if value.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(value))
                };
            }
            "APPSTDIN" => {
                config.app_stdin = if value.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(value))
                };
            }
            "APPSTOPMETHOD" => {
                config.app_stop_method_skip = value.parse()
                    .map_err(|_| format!("Invalid stop method value: {}", value))?;
            }
            "APPSTOPMETHOD_CONSOLE" => {
                config.app_stop_method_console = value.parse()
                    .map_err(|_| format!("Invalid stop method console value: {}", value))?;
            }
            "APPSTOPMETHOD_WINDOW" => {
                config.app_stop_method_window = value.parse()
                    .map_err(|_| format!("Invalid stop method window value: {}", value))?;
            }
            "APPSTOPMETHOD_THREADS" => {
                config.app_stop_method_threads = value.parse()
                    .map_err(|_| format!("Invalid stop method threads value: {}", value))?;
            }
            "APPRESTARTDELAY" => {
                config.app_restart_delay = value.parse()
                    .map_err(|_| format!("Invalid restart delay value: {}", value))?;
            }
            "APPEXITACTION" => {
                config.app_exit_default = crate::cli::ExitAction::from_str(value)
                    .ok_or_else(|| format!("Invalid exit action: {}", value))?;
            }
            _ => {
                return Err(format!("Unknown parameter: {}", parameter));
            }
        }

        self.save_service_config(service_name, &config)?;
        info!("Parameter '{}' set to '{}' for service '{}'", parameter, value, service_name);
        Ok(())
    }

    pub fn get_service_parameter(&self, service_name: &str, parameter: &str) -> Result<String, String> {
        let config = self.load_service_config(service_name)?;

        let value = match parameter.to_uppercase().as_str() {
            "APPLICATION" => config.application.to_string_lossy().to_string(),
            "APPDIRECTORY" => config.app_directory
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            "APPPARAMETERS" => config.app_parameters.unwrap_or_default(),
            "DISPLAYNAME" => config.display_name.unwrap_or_default(),
            "DESCRIPTION" => config.description.unwrap_or_default(),
            "START" => match config.start_type {
                ServiceStartType::Auto => "SERVICE_AUTO_START".to_string(),
                ServiceStartType::Manual => "SERVICE_DEMAND_START".to_string(),
                ServiceStartType::Disabled => "SERVICE_DISABLED".to_string(),
            },
            "APPPRIORITY" => match config.app_priority {
                ProcessPriority::Normal => "NORMAL_PRIORITY_CLASS".to_string(),
                ProcessPriority::High => "HIGH_PRIORITY_CLASS".to_string(),
                ProcessPriority::Realtime => "REALTIME_PRIORITY_CLASS".to_string(),
                ProcessPriority::AboveNormal => "ABOVE_NORMAL_PRIORITY_CLASS".to_string(),
                ProcessPriority::BelowNormal => "BELOW_NORMAL_PRIORITY_CLASS".to_string(),
                ProcessPriority::Idle => "IDLE_PRIORITY_CLASS".to_string(),
            },
            "APPNOCONSOLE" => if config.app_no_console { "1" } else { "0" }.to_string(),
            "APPTHROTTLE" => config.app_throttle.to_string(),
            "APPSTDOUT" => config.app_stdout
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            "APPSTDERR" => config.app_stderr
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            "APPSTDIN" => config.app_stdin
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            "APPSTOPMETHOD" => config.app_stop_method_skip.to_string(),
            "APPSTOPMETHOD_CONSOLE" => config.app_stop_method_console.to_string(),
            "APPSTOPMETHOD_WINDOW" => config.app_stop_method_window.to_string(),
            "APPSTOPMETHOD_THREADS" => config.app_stop_method_threads.to_string(),
            "APPRESTARTDELAY" => config.app_restart_delay.to_string(),
            "APPEXITACTION" => config.app_exit_default.to_str().to_string(),
            _ => {
                return Err(format!("Unknown parameter: {}", parameter));
            }
        };

        println!("{}: {}", parameter, value);
        Ok(value)
    }

    fn save_service_config(&self, service_name: &str, config: &ServiceConfig) -> Result<(), String> {
        unsafe {
            let key_path = format!("SYSTEM\\CurrentControlSet\\Services\\{}\\Parameters", service_name);
            let key_path_wide: Vec<u16> = key_path.encode_utf16().chain(std::iter::once(0)).collect();

            let mut key_handle = HKEY::default();
            let result = RegCreateKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR::from_raw(key_path_wide.as_ptr()),
                0,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut key_handle,
                None,
            );

            if result != ERROR_SUCCESS {
                return Err("Failed to create registry key".to_string());
            }

            // Save application path
            self.set_registry_string(&key_handle, "Application", &config.application.to_string_lossy())?;

            // Save app directory
            if let Some(ref app_dir) = config.app_directory {
                self.set_registry_string(&key_handle, "AppDirectory", &app_dir.to_string_lossy())?;
            }

            // Save app parameters
            if let Some(ref params) = config.app_parameters {
                self.set_registry_string(&key_handle, "AppParameters", params)?;
            }

            // Save other settings
            self.set_registry_dword(&key_handle, "AppPriority", config.app_priority.to_windows_value())?;
            self.set_registry_dword(&key_handle, "AppNoConsole", if config.app_no_console { 1 } else { 0 })?;
            self.set_registry_dword(&key_handle, "AppThrottle", config.app_throttle)?;
            self.set_registry_dword(&key_handle, "AppStopMethodSkip", config.app_stop_method_skip)?;
            self.set_registry_dword(&key_handle, "AppStopMethodConsole", config.app_stop_method_console)?;
            self.set_registry_dword(&key_handle, "AppStopMethodWindow", config.app_stop_method_window)?;
            self.set_registry_dword(&key_handle, "AppStopMethodThreads", config.app_stop_method_threads)?;
            self.set_registry_dword(&key_handle, "AppRestartDelay", config.app_restart_delay)?;
            self.set_registry_string(&key_handle, "AppExitDefault", config.app_exit_default.to_str())?;

            // Save I/O redirection settings
            if let Some(ref stdout_path) = config.app_stdout {
                self.set_registry_string(&key_handle, "AppStdout", &stdout_path.to_string_lossy())?;
            }
            if let Some(ref stderr_path) = config.app_stderr {
                self.set_registry_string(&key_handle, "AppStderr", &stderr_path.to_string_lossy())?;
            }
            if let Some(ref stdin_path) = config.app_stdin {
                self.set_registry_string(&key_handle, "AppStdin", &stdin_path.to_string_lossy())?;
            }

            let _ = RegCloseKey(key_handle);
        }

        Ok(())
    }

    fn load_service_config(&self, service_name: &str) -> Result<ServiceConfig, String> {
        unsafe {
            let key_path = format!("SYSTEM\\CurrentControlSet\\Services\\{}\\Parameters", service_name);
            let key_path_wide: Vec<u16> = key_path.encode_utf16().chain(std::iter::once(0)).collect();

            let mut key_handle = HKEY::default();
            let result = RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR::from_raw(key_path_wide.as_ptr()),
                0,
                KEY_READ,
                &mut key_handle,
            );

            if result != ERROR_SUCCESS {
                return Err(format!("Failed to open registry key for service '{}'", service_name));
            }

            let mut config = ServiceConfig::default();

            // Load application path
            if let Ok(app_path) = self.get_registry_string(&key_handle, "Application") {
                config.application = PathBuf::from(app_path);
            }

            // Load app directory
            if let Ok(app_dir) = self.get_registry_string(&key_handle, "AppDirectory") {
                if !app_dir.is_empty() {
                    config.app_directory = Some(PathBuf::from(app_dir));
                }
            }

            // Load app parameters
            if let Ok(params) = self.get_registry_string(&key_handle, "AppParameters") {
                if !params.is_empty() {
                    config.app_parameters = Some(params);
                }
            }

            // Load other settings
            if let Ok(priority) = self.get_registry_dword(&key_handle, "AppPriority") {
                config.app_priority = match priority {
                    0x00000100 => ProcessPriority::Realtime,
                    0x00000080 => ProcessPriority::High,
                    0x00008000 => ProcessPriority::AboveNormal,
                    0x00000020 => ProcessPriority::Normal,
                    0x00004000 => ProcessPriority::BelowNormal,
                    0x00000040 => ProcessPriority::Idle,
                    _ => ProcessPriority::Normal,
                };
            }

            if let Ok(no_console) = self.get_registry_dword(&key_handle, "AppNoConsole") {
                config.app_no_console = no_console != 0;
            }

            if let Ok(throttle) = self.get_registry_dword(&key_handle, "AppThrottle") {
                config.app_throttle = throttle;
            }

            if let Ok(stop_method_skip) = self.get_registry_dword(&key_handle, "AppStopMethodSkip") {
                config.app_stop_method_skip = stop_method_skip;
            }

            if let Ok(stop_method_console) = self.get_registry_dword(&key_handle, "AppStopMethodConsole") {
                config.app_stop_method_console = stop_method_console;
            }

            if let Ok(stop_method_window) = self.get_registry_dword(&key_handle, "AppStopMethodWindow") {
                config.app_stop_method_window = stop_method_window;
            }

            if let Ok(stop_method_threads) = self.get_registry_dword(&key_handle, "AppStopMethodThreads") {
                config.app_stop_method_threads = stop_method_threads;
            }

            if let Ok(restart_delay) = self.get_registry_dword(&key_handle, "AppRestartDelay") {
                config.app_restart_delay = restart_delay;
            }

            if let Ok(exit_default) = self.get_registry_string(&key_handle, "AppExitDefault") {
                if let Some(exit_action) = crate::cli::ExitAction::from_str(&exit_default) {
                    config.app_exit_default = exit_action;
                }
            }

            // Load I/O redirection settings
            if let Ok(stdout_path) = self.get_registry_string(&key_handle, "AppStdout") {
                if !stdout_path.is_empty() {
                    config.app_stdout = Some(PathBuf::from(stdout_path));
                }
            }

            if let Ok(stderr_path) = self.get_registry_string(&key_handle, "AppStderr") {
                if !stderr_path.is_empty() {
                    config.app_stderr = Some(PathBuf::from(stderr_path));
                }
            }

            if let Ok(stdin_path) = self.get_registry_string(&key_handle, "AppStdin") {
                if !stdin_path.is_empty() {
                    config.app_stdin = Some(PathBuf::from(stdin_path));
                }
            }

            let _ = RegCloseKey(key_handle);
            Ok(config)
        }
    }

    fn remove_service_config(&self, service_name: &str) -> Result<(), String> {
        unsafe {
            let key_path = format!("SYSTEM\\CurrentControlSet\\Services\\{}\\Parameters", service_name);
            let key_path_wide: Vec<u16> = key_path.encode_utf16().chain(std::iter::once(0)).collect();

            let result = RegDeleteTreeW(
                HKEY_LOCAL_MACHINE,
                PCWSTR::from_raw(key_path_wide.as_ptr()),
            );

            if result != ERROR_SUCCESS && result.0 != 2 { // ERROR_FILE_NOT_FOUND
                warn!("Failed to delete service registry configuration");
            }
        }

        Ok(())
    }

    fn set_registry_string(&self, key: &HKEY, name: &str, value: &str) -> Result<(), String> {
        unsafe {
            let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let value_wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();

            let result = RegSetValueExW(
                *key,
                PCWSTR::from_raw(name_wide.as_ptr()),
                0,
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    value_wide.as_ptr() as *const u8,
                    value_wide.len() * 2,
                )),
            );

            if result != ERROR_SUCCESS {
                return Err("Failed to set registry string value".to_string());
            }
        }

        Ok(())
    }

    fn set_registry_dword(&self, key: &HKEY, name: &str, value: u32) -> Result<(), String> {
        unsafe {
            let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();

            let result = RegSetValueExW(
                *key,
                PCWSTR::from_raw(name_wide.as_ptr()),
                0,
                REG_DWORD,
                Some(std::slice::from_raw_parts(
                    &value as *const u32 as *const u8,
                    4,
                )),
            );

            if result != ERROR_SUCCESS {
                return Err("Failed to set registry DWORD value".to_string());
            }
        }

        Ok(())
    }

    fn get_registry_string(&self, key: &HKEY, name: &str) -> Result<String, String> {
        unsafe {
            let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let mut buffer = vec![0u16; 1024];
            let mut buffer_size = (buffer.len() * 2) as u32;

            let result = RegQueryValueExW(
                *key,
                PCWSTR::from_raw(name_wide.as_ptr()),
                None,
                None,
                Some(buffer.as_mut_ptr() as *mut u8),
                Some(&mut buffer_size),
            );

            if result != ERROR_SUCCESS {
                return Err("Failed to get registry string value".to_string());
            }

            let len = (buffer_size / 2) as usize;
            if len > 0 && buffer[len - 1] == 0 {
                buffer.truncate(len - 1);
            }

            Ok(String::from_utf16_lossy(&buffer))
        }
    }

    fn get_registry_dword(&self, key: &HKEY, name: &str) -> Result<u32, String> {
        unsafe {
            let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let mut value = 0u32;
            let mut buffer_size = 4u32;

            let result = RegQueryValueExW(
                *key,
                PCWSTR::from_raw(name_wide.as_ptr()),
                None,
                None,
                Some(&mut value as *mut u32 as *mut u8),
                Some(&mut buffer_size),
            );

            if result != ERROR_SUCCESS {
                return Err("Failed to get registry DWORD value".to_string());
            }

            Ok(value)
        }
    }

    pub fn load_service_config_for_run(&self, service_name: &str) -> Result<ServiceConfig, String> {
        self.load_service_config(service_name)
    }

    pub fn query_service_status(&self, service_name: &str) -> Result<(), String> {
        unsafe {
            let service_name_wide: Vec<u16> = service_name.encode_utf16().chain(std::iter::once(0)).collect();
            
            let service_handle = OpenServiceW(
                self.handle,
                PCWSTR::from_raw(service_name_wide.as_ptr()),
                SERVICE_QUERY_STATUS,
            ).map_err(|e| format!("Failed to open service '{}': {}", service_name, e))?;

            let mut status = SERVICE_STATUS::default();
            QueryServiceStatus(service_handle, &mut status)
                .map_err(|e| format!("Failed to query service status: {}", e))?;
            
            let _ = CloseServiceHandle(service_handle);

            let state_str = match status.dwCurrentState.0 {
                1 => "STOPPED",
                2 => "START_PENDING", 
                3 => "STOP_PENDING",
                4 => "RUNNING",
                5 => "CONTINUE_PENDING",
                6 => "PAUSE_PENDING",
                7 => "PAUSED",
                _ => "UNKNOWN",
            };

            println!("Service Name: {}", service_name);
            println!("State: {}", state_str);
            println!("Exit Code: {}", status.dwWin32ExitCode);
            println!("Service Specific Exit Code: {}", status.dwServiceSpecificExitCode);
            println!("Checkpoint: {}", status.dwCheckPoint);
            println!("Wait Hint: {}ms", status.dwWaitHint);
        }

        Ok(())
    }

    pub fn list_nssm_services(&self) -> Result<(), String> {
        unsafe {
            use windows::Win32::System::Registry::{RegEnumKeyExW, RegOpenKeyExW};
            
            let services_key_path = "SYSTEM\\CurrentControlSet\\Services";
            let services_key_path_wide: Vec<u16> = services_key_path.encode_utf16().chain(std::iter::once(0)).collect();

            let mut services_key = HKEY::default();
            let result = RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR::from_raw(services_key_path_wide.as_ptr()),
                0,
                KEY_READ,
                &mut services_key,
            );

            if result != ERROR_SUCCESS {
                return Err("Failed to open services registry key".to_string());
            }

            let mut index = 0u32;
            let mut service_name_buffer = vec![0u16; 256];


            println!("Services managed by nssm-rs:");
            let mut found_any = false;

            loop {
                let mut service_name_len = service_name_buffer.len() as u32;
                let result = RegEnumKeyExW(
                    services_key,
                    index,
                    windows::core::PWSTR::from_raw(service_name_buffer.as_mut_ptr()),
                    &mut service_name_len,
                    None,
                    windows::core::PWSTR::null(),
                    None,
                    None,
                );

                if result != ERROR_SUCCESS {
                    break;
                }

                let service_name = String::from_utf16_lossy(&service_name_buffer[..service_name_len as usize]);
                
                // Check if this service has nssm-rs parameters
                if self.has_nssm_config(&service_name) {
                    println!("  {}", service_name);
                    found_any = true;
                }

                index += 1;
            }

            if !found_any {
                println!("  (none)");
            }

            let _ = RegCloseKey(services_key);
        }

        Ok(())
    }

    fn has_nssm_config(&self, service_name: &str) -> bool {
        let key_path = format!("SYSTEM\\CurrentControlSet\\Services\\{}\\Parameters", service_name);
        let key_path_wide: Vec<u16> = key_path.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let mut key_handle = HKEY::default();
            let result = RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR::from_raw(key_path_wide.as_ptr()),
                0,
                KEY_READ,
                &mut key_handle,
            );

            if result == ERROR_SUCCESS {
                let has_application = self.get_registry_string(&key_handle, "Application").is_ok();
                let _ = RegCloseKey(key_handle);
                has_application
            } else {
                false
            }
        }
    }

}

impl Drop for ServiceManager {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseServiceHandle(self.handle);
        }
    }
}
