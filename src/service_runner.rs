use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use log::{debug, error, info, warn};
use windows::Win32::System::Console::{AllocConsole, CTRL_C_EVENT};
use windows::Win32::System::Threading::{PROCESS_CREATION_FLAGS, SetPriorityClass};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle},
    service_dispatcher,
};

use crate::config::{ExitAction, ServiceConfig};
use crate::error::{AppError, AppResult};
use crate::service_manager::ServiceManager;

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

define_windows_service!(ffi_service_main, service_main);

#[derive(Debug)]
enum ProcessStatus {
    Running,
    Exited(i32),
    Terminated,
}

enum LoopControl {
    Restart(Option<Instant>, ServiceExitCode),
    Exit(ServiceExitCode),
}

struct RunningChild {
    child: Child,
    stdout_thread: Option<thread::JoinHandle<()>>,
    stderr_thread: Option<thread::JoinHandle<()>>,
}

pub fn run_service(service_name: String) -> AppResult<()> {
    service_dispatcher::start(service_name, ffi_service_main)?;
    Ok(())
}

fn service_main(arguments: Vec<OsString>) {
    info!("Service main function started");
    debug!("Service arguments: {arguments:?}");

    unsafe {
        if AllocConsole().is_err() {
            error!("Windows AllocConsole failed");
        }
    }

    let service_name = arguments
        .first()
        .and_then(|arg| arg.to_str())
        .unwrap_or("nssm-rs")
        .to_string();

    if let Err(error) = run_service_main(service_name.clone()) {
        error!("Service '{service_name}' failed: {error}");
    }
}

fn run_service_main(service_name: String) -> AppResult<()> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel();
    let status_handle = register_service_handler(&service_name, shutdown_tx)?;
    let config = ServiceManager::new()?.load_service_config_for_run(&service_name)?;

    set_running_status(&status_handle)?;
    install_ctrlc_guard()?;

    let stop_ctrlc = Arc::new(AtomicBool::new(false));
    let mut restart_after: Option<Instant> = None;
    let mut consecutive_failures = 0u32;
    let mut service_exit_code = ServiceExitCode::NO_ERROR;

    'outer: loop {
        if wait_for_restart_delay(&shutdown_rx, restart_after)? {
            break 'outer;
        }
        let mut running_child = match launch_child(&config) {
            Ok(child) => child,
            Err(error) => {
                error!("Failed to launch application: {error}");
                service_exit_code = ServiceExitCode::ServiceSpecific(1);
                break 'outer;
            }
        };

        match monitor_child(
            &status_handle,
            &shutdown_rx,
            &config,
            &stop_ctrlc,
            &mut running_child.child,
            &mut consecutive_failures,
        )? {
            LoopControl::Restart(next_restart, exit_code) => {
                service_exit_code = exit_code;
                restart_after = next_restart;
            }
            LoopControl::Exit(exit_code) => {
                service_exit_code = exit_code;
                finalize_child_threads(running_child);
                break 'outer;
            }
        }

        finalize_child_threads(running_child);
    }

    set_stopped_status(&status_handle, service_exit_code)?;
    Ok(())
}

fn register_service_handler(
    service_name: &str,
    shutdown_tx: mpsc::Sender<()>,
) -> AppResult<ServiceStatusHandle> {
    let service_name_for_handler = service_name.to_string();
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop | ServiceControl::Shutdown => {
                info!("Service '{service_name_for_handler}' received stop signal");
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    Ok(service_control_handler::register(
        service_name,
        event_handler,
    )?)
}

fn set_running_status(status_handle: &ServiceStatusHandle) -> AppResult<()> {
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::NO_ERROR,
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;
    Ok(())
}

fn set_stop_pending_status(status_handle: &ServiceStatusHandle) -> AppResult<()> {
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::StopPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::NO_ERROR,
        checkpoint: 0,
        wait_hint: Duration::from_millis(5000),
        process_id: None,
    })?;
    Ok(())
}

fn set_stopped_status(
    status_handle: &ServiceStatusHandle,
    exit_code: ServiceExitCode,
) -> AppResult<()> {
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code,
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;
    Ok(())
}

fn install_ctrlc_guard() -> AppResult<()> {
    ctrlc::set_handler(|| {}).map_err(AppError::from)
}

fn wait_for_restart_delay(
    shutdown_rx: &mpsc::Receiver<()>,
    restart_after: Option<Instant>,
) -> AppResult<bool> {
    if let Some(delay_until) = restart_after {
        loop {
            let now = Instant::now();
            if now >= delay_until {
                return Ok(false);
            }

            let sleep_duration = (delay_until - now).min(Duration::from_millis(100));
            match shutdown_rx.recv_timeout(sleep_duration) {
                Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(true),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
            }
        }
    }

    Ok(false)
}

fn launch_child(config: &ServiceConfig) -> AppResult<RunningChild> {
    let mut command = build_command(config);
    let mut child = command.spawn()?;
    let child_id = child.id();
    info!("Application launched with PID: {child_id}");

    set_child_priority(child_id, config)?;

    let stdout_thread = spawn_output_thread(child.stdout.take(), config.app_stdout.clone(), false);
    let stderr_thread = spawn_output_thread(child.stderr.take(), config.app_stderr.clone(), true);

    Ok(RunningChild {
        child,
        stdout_thread,
        stderr_thread,
    })
}

fn build_command(config: &ServiceConfig) -> Command {
    let mut command = Command::new(&config.application);
    command.current_dir(resolve_working_dir(config));

    if let Some(parameters) = &config.app_parameters {
        command.args(parse_command_line(parameters));
    }

    configure_stdio(&mut command, config);

    for env_var in &config.app_environment_extra {
        if let Some((key, value)) = env_var.split_once('=') {
            command.env(key, value);
        }
    }

    command
}

fn resolve_working_dir(config: &ServiceConfig) -> PathBuf {
    config.app_directory.as_ref().cloned().unwrap_or_else(|| {
        config
            .application
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    })
}

fn configure_stdio(command: &mut Command, config: &ServiceConfig) {
    if config.app_stdout.is_some() || config.app_stderr.is_some() {
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
    } else if config.app_no_console {
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
    }
}

fn set_child_priority(child_id: u32, config: &ServiceConfig) -> AppResult<()> {
    unsafe {
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_INFORMATION};

        match OpenProcess(PROCESS_SET_INFORMATION, false, child_id) {
            Ok(process_handle) => {
                let _ = SetPriorityClass(
                    process_handle,
                    PROCESS_CREATION_FLAGS(config.app_priority.to_windows_value()),
                );
                let _ = windows::Win32::Foundation::CloseHandle(process_handle);
                Ok(())
            }
            Err(error) => {
                warn!("Failed to set process priority for child process: {error}");
                Ok(())
            }
        }
    }
}

fn spawn_output_thread<T>(
    stream: Option<T>,
    output_path: Option<PathBuf>,
    is_stderr: bool,
) -> Option<thread::JoinHandle<()>>
where
    T: std::io::Read + Send + 'static,
{
    stream.map(|stream| {
        thread::spawn(move || {
            if is_stderr {
                handle_output(stream, output_path, true);
            } else {
                handle_output(stream, output_path, false);
            }
        })
    })
}

fn monitor_child(
    status_handle: &ServiceStatusHandle,
    shutdown_rx: &mpsc::Receiver<()>,
    config: &ServiceConfig,
    stop_ctrlc: &Arc<AtomicBool>,
    child: &mut Child,
    consecutive_failures: &mut u32,
) -> AppResult<LoopControl> {
    let start_time = Instant::now();

    loop {
        match shutdown_rx.recv_timeout(Duration::from_secs(1)) {
            Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                info!("Shutting down service");
                set_stop_pending_status(status_handle)?;
                stop_child_process(child, config, stop_ctrlc);
                return Ok(LoopControl::Exit(ServiceExitCode::NO_ERROR));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        match check_process_status(child)? {
            ProcessStatus::Running => continue,
            ProcessStatus::Exited(exit_code) => {
                let runtime = start_time.elapsed();
                info!("Application exited with code {exit_code} after {runtime:?}");
                let service_exit_code = exit_code_to_service_code(exit_code);

                if should_restart(&config.app_exit_default) {
                    return Ok(LoopControl::Restart(
                        calculate_restart_delay(config, runtime, consecutive_failures),
                        service_exit_code,
                    ));
                }

                return Ok(LoopControl::Exit(service_exit_code));
            }
            ProcessStatus::Terminated => {
                info!("Application was terminated");
                return Ok(LoopControl::Restart(
                    Some(Instant::now() + Duration::from_millis(config.app_throttle as u64)),
                    ServiceExitCode::ServiceSpecific(259),
                ));
            }
        }
    }
}

fn calculate_restart_delay(
    config: &ServiceConfig,
    runtime: Duration,
    consecutive_failures: &mut u32,
) -> Option<Instant> {
    let delay = if runtime.as_millis() < config.app_throttle as u128 {
        *consecutive_failures += 1;
        let millis = (2u64.pow((*consecutive_failures).min(8))) * 1000;
        Duration::from_millis(millis.min(256000))
    } else {
        *consecutive_failures = 0;
        Duration::from_millis(config.app_restart_delay as u64)
    };

    (delay.as_millis() > 0).then(|| Instant::now() + delay)
}

fn exit_code_to_service_code(exit_code: i32) -> ServiceExitCode {
    if exit_code == 0 {
        ServiceExitCode::NO_ERROR
    } else {
        ServiceExitCode::ServiceSpecific(exit_code as u32)
    }
}

fn finalize_child_threads(running_child: RunningChild) {
    if let Some(thread) = running_child.stdout_thread {
        let _ = thread.join();
    }
    if let Some(thread) = running_child.stderr_thread {
        let _ = thread.join();
    }
}

fn check_process_status(child: &mut Child) -> AppResult<ProcessStatus> {
    match child.try_wait()? {
        None => Ok(ProcessStatus::Running),
        Some(status) => match status.code() {
            Some(code) => Ok(ProcessStatus::Exited(code)),
            None => Ok(ProcessStatus::Terminated),
        },
    }
}

fn should_restart(exit_action: &ExitAction) -> bool {
    matches!(exit_action, ExitAction::Restart)
}

fn stop_child_process(child: &mut Child, config: &ServiceConfig, stop_ctrlc: &Arc<AtomicBool>) {
    info!("Stopping child process with PID: {}", child.id());
    stop_ctrlc.store(true, Ordering::SeqCst);

    let child_id = child.id();

    if !config.app_no_console && (config.app_stop_method_skip & 1) == 0 {
        try_console_ctrl_c(child, child_id, config.app_stop_method_console, stop_ctrlc);
    }
    if (config.app_stop_method_skip & 2) == 0 {
        try_close_windows(child, child_id, config.app_stop_method_window, stop_ctrlc);
    }
    if (config.app_stop_method_skip & 4) == 0 {
        try_terminate_process(child, child_id, config.app_stop_method_threads, stop_ctrlc);
    }
    if (config.app_stop_method_skip & 8) == 0
        && let Err(error) = child.kill()
    {
        warn!("Failed to kill child process: {error}");
    }

    let _ = child.wait();
    stop_ctrlc.store(false, Ordering::SeqCst);
}

fn try_console_ctrl_c(
    child: &mut Child,
    child_id: u32,
    timeout_ms: u32,
    stop_ctrlc: &Arc<AtomicBool>,
) {
    info!("Sending Ctrl-C to child process");
    unsafe {
        use windows::Win32::System::Console::{
            AttachConsole, FreeConsole, GenerateConsoleCtrlEvent,
        };

        if AttachConsole(child_id).is_ok() {
            let _ = GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0);
            let _ = FreeConsole();
        } else if GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0).is_err() {
            warn!("Failed to send Ctrl-C event");
        }
    }

    if wait_for_process_exit(child, timeout_ms) {
        info!("Child process stopped after Ctrl-C");
        stop_ctrlc.store(false, Ordering::SeqCst);
    }
}

fn try_close_windows(
    child: &mut Child,
    child_id: u32,
    timeout_ms: u32,
    stop_ctrlc: &Arc<AtomicBool>,
) {
    info!("Sending WM_CLOSE to child process windows");
    unsafe {
        use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
        };
        use windows::core::BOOL;

        unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let target_pid = lparam.0 as u32;
            let mut window_pid = 0u32;
            unsafe {
                GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
                if window_pid == target_pid {
                    let _ = PostMessageW(
                        Some(hwnd),
                        WM_CLOSE,
                        windows::Win32::Foundation::WPARAM(0),
                        LPARAM(0),
                    );
                }
            }
            TRUE
        }

        let _ = EnumWindows(Some(enum_window_proc), LPARAM(child_id as isize));
    }

    if wait_for_process_exit(child, timeout_ms) {
        info!("Child process stopped after WM_CLOSE");
        stop_ctrlc.store(false, Ordering::SeqCst);
    }
}

fn try_terminate_process(
    child: &mut Child,
    child_id: u32,
    timeout_ms: u32,
    stop_ctrlc: &Arc<AtomicBool>,
) {
    info!("Terminating child process");
    unsafe {
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

        if let Ok(process_handle) = OpenProcess(PROCESS_TERMINATE, false, child_id) {
            let _ = TerminateProcess(process_handle, 1);
            let _ = windows::Win32::Foundation::CloseHandle(process_handle);
        }
    }

    if wait_for_process_exit(child, timeout_ms) {
        info!("Child process stopped after terminate");
        stop_ctrlc.store(false, Ordering::SeqCst);
    }
}

fn wait_for_process_exit(child: &mut Child, timeout_ms: u32) -> bool {
    let start = Instant::now();
    while start.elapsed().as_millis() < timeout_ms as u128 {
        match check_process_status(child) {
            Ok(ProcessStatus::Running) => thread::sleep(Duration::from_millis(50)),
            _ => return true,
        }
    }
    false
}

fn handle_output<T>(stream: T, output_path: Option<PathBuf>, is_stderr: bool)
where
    T: std::io::Read,
{
    let reader = BufReader::new(stream);
    match output_path {
        Some(path) => {
            if let Err(error) = write_output_to_file(reader, &path, is_stderr) {
                error!("Failed to write redirected output to {path:?}: {error}");
            }
        }
        None => {
            for line in reader.lines().map_while(Result::ok) {
                log_output_line(&line, is_stderr);
            }
        }
    }
}

fn write_output_to_file<T>(reader: BufReader<T>, path: &Path, is_stderr: bool) -> AppResult<()>
where
    T: std::io::Read,
{
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    for line in reader.lines() {
        let line = line?;
        log_output_line(&line, is_stderr);
        writeln!(file, "{line}")?;
    }

    Ok(())
}

fn log_output_line(line: &str, is_stderr: bool) {
    if is_stderr {
        warn!("stderr: {line}");
    } else {
        info!("stdout: {line}");
    }
}

fn parse_command_line(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current_arg = String::new();
    let mut in_quotes = false;

    for ch in input.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current_arg.is_empty() {
                    args.push(std::mem::take(&mut current_arg));
                }
            }
            _ => current_arg.push(ch),
        }
    }

    if !current_arg.is_empty() {
        args.push(current_arg);
    }

    args
}
