use crate::cli::ServiceConfig;
use crate::service_manager::ServiceManager;
use log::{debug, error, info, warn};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::System::Console::{AllocConsole, CTRL_C_EVENT};
use windows::Win32::System::Threading::{PROCESS_CREATION_FLAGS, SetPriorityClass};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

define_windows_service!(ffi_service_main, service_main);

#[derive(Debug)]
enum ProcessStatus {
    Running,
    Exited(i32),
    Terminated,
}

pub fn run_service(service_name: String) -> windows_service::Result<()> {
    service_dispatcher::start(service_name, ffi_service_main)
}

fn service_main(arguments: Vec<OsString>) {
    info!("Service main function started");
    debug!("Service arguments: {arguments:?}");

    // Windows services don't start with a console, so we have to allocate one
    // in order to send ctrl-C to children.
    unsafe {
        if AllocConsole().is_err() {
            error!("Windows AllocConsole failed");
        } else {
            debug!("Console allocated successfully for service");
        }
    }

    let service_name = arguments
        .first()
        .and_then(|arg| arg.to_str())
        .unwrap_or("nssm-rs")
        .to_string();

    info!("Running service: '{service_name}'");

    if let Err(e) = run_service_main(service_name.clone()) {
        error!("Service '{service_name}' failed: {e:?}");
    }
}

fn run_service_main(service_name: String) -> windows_service::Result<()> {
    info!("Starting service main logic for: '{service_name}'");

    let (shutdown_tx, shutdown_rx) = mpsc::channel();
    let mut service_exit_code = ServiceExitCode::NO_ERROR;

    // Set up service control handler
    let service_name_for_handler = service_name.clone();
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Interrogate => {
                debug!("Service '{service_name_for_handler}' received interrogate event");
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Stop => {
                info!("Service '{service_name_for_handler}' received stop event");
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Shutdown => {
                info!("Service '{service_name_for_handler}' received shutdown event");
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            _ => {
                debug!(
                    "Service '{service_name_for_handler}' received unhandled control event: {control_event:?}"
                );
                ServiceControlHandlerResult::NotImplemented
            }
        }
    };

    info!("Registering service control handler for '{service_name}'");
    let status_handle = service_control_handler::register(&service_name, event_handler)?;

    // Load service configuration
    info!("Loading service configuration for '{service_name}'");
    let service_manager = ServiceManager::new().map_err(|e| {
        error!("Failed to create ServiceManager: {e}");
        windows_service::Error::Winapi(std::io::Error::other(e))
    })?;

    let config = service_manager
        .load_service_config_for_run(&service_name)
        .map_err(|e| {
            error!("Failed to load service configuration for '{service_name}': {e}");
            windows_service::Error::Winapi(std::io::Error::other(e))
        })?;

    info!("Service configuration loaded successfully for '{service_name}'");
    debug!("Configuration: {config:?}");

    // Set service status to running
    info!("Setting service '{service_name}' status to Running");
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::NO_ERROR,
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    let stop_ctrlc = Arc::new(AtomicBool::new(false));
    let stop_ctrlc_clone = stop_ctrlc.clone();

    // Set up ctrl-C handler to prevent service from being killed
    ctrlc::set_handler(move || {
        if !stop_ctrlc_clone.load(Ordering::SeqCst) {
            // Ignore ctrl-C when not stopping
        }
    })
    .expect("Error setting ctrl-C handler");

    let mut restart_after: Option<Instant> = None;
    let mut consecutive_failures = 0u32;

    info!("Entering main service loop");

    // Main service loop
    'outer: loop {
        // Handle restart delay
        if let Some(delay_until) = restart_after {
            let now = Instant::now();
            if now < delay_until {
                let sleep_duration = (delay_until - now).min(Duration::from_millis(100));
                debug!("Sleeping for restart delay: {sleep_duration:?}");

                match shutdown_rx.recv_timeout(sleep_duration) {
                    Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                        info!("Cancelling restart due to shutdown signal");
                        break 'outer;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                }
            } else {
                info!("Restart delay complete");
                restart_after = None;
            }
        }

        info!("Launching application: {:?}", config.application);

        // Build command
        let mut cmd = Command::new(&config.application);

        // Set working directory - default to application directory
        let working_dir = config.app_directory.as_ref().cloned().unwrap_or_else(|| {
            config
                .application
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("."))
        });
        cmd.current_dir(&working_dir);

        // Add parameters
        if let Some(ref params) = config.app_parameters {
            // Simple parameter parsing - split by spaces but respect quotes
            let args = parse_command_line(params);
            cmd.args(args);
        }

        // Set up I/O redirection
        if config.app_stdout.is_some() || config.app_stderr.is_some() {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        } else if config.app_no_console {
            cmd.stdout(Stdio::null());
            cmd.stderr(Stdio::null());
        }

        // Set environment variables
        for env_var in &config.app_environment_extra {
            if let Some((key, value)) = env_var.split_once('=') {
                cmd.env(key, value);
            }
        }

        // Launch the process
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                error!("Failed to launch application: {e}");
                service_exit_code = ServiceExitCode::ServiceSpecific(1);
                break 'outer;
            }
        };

        let child_id = child.id();
        info!("Application launched with PID: {child_id}");

        // Set process priority for the child process
        unsafe {
            use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_INFORMATION};

            if let Ok(process_handle) = OpenProcess(PROCESS_SET_INFORMATION, false, child_id) {
                let _ = SetPriorityClass(
                    process_handle,
                    PROCESS_CREATION_FLAGS(config.app_priority.to_windows_value()),
                );
                let _ = windows::Win32::Foundation::CloseHandle(process_handle);
            } else {
                warn!("Failed to set process priority for child process");
            }
        }

        // Handle I/O redirection
        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        let stdout_thread = if let Some(stdout) = stdout_handle {
            let stdout_path = config.app_stdout.clone();
            Some(thread::spawn(move || {
                handle_stdout(stdout, stdout_path);
            }))
        } else {
            None
        };

        let stderr_thread = if let Some(stderr) = stderr_handle {
            let stderr_path = config.app_stderr.clone();
            Some(thread::spawn(move || {
                handle_stderr(stderr, stderr_path);
            }))
        } else {
            None
        };

        let start_time = Instant::now();

        // Monitor child process
        'inner: loop {
            // Check for shutdown signal
            match shutdown_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    info!("Shutting down service");

                    // Set service status to stopping
                    status_handle.set_service_status(ServiceStatus {
                        service_type: SERVICE_TYPE,
                        current_state: ServiceState::StopPending,
                        controls_accepted: ServiceControlAccept::empty(),
                        exit_code: ServiceExitCode::NO_ERROR,
                        checkpoint: 0,
                        wait_hint: Duration::from_millis(5000),
                        process_id: None,
                    })?;

                    // Stop child process gracefully
                    stop_child_process(&mut child, &config, &stop_ctrlc);
                    service_exit_code = ServiceExitCode::NO_ERROR;
                    break 'outer;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Continue monitoring
                }
            }

            // Check child process status
            match check_process_status(&mut child) {
                Ok(ProcessStatus::Running) => {
                    // Process is still running
                    continue 'inner;
                }
                Ok(ProcessStatus::Exited(exit_code)) => {
                    let runtime = start_time.elapsed();
                    info!("Application exited with code {exit_code} after {runtime:?}");

                    service_exit_code = if exit_code == 0 {
                        ServiceExitCode::NO_ERROR
                    } else {
                        ServiceExitCode::ServiceSpecific(exit_code as u32)
                    };

                    // Decide whether to restart based on exit action
                    if should_restart(exit_code, &config.app_exit_default) {
                        // Calculate restart delay with throttling
                        let throttle_delay = if runtime.as_millis() < config.app_throttle as u128 {
                            consecutive_failures += 1;
                            let delay = (2u64.pow(consecutive_failures.min(8))) * 1000;
                            Duration::from_millis(delay.min(256000)) // Max 4+ minutes
                        } else {
                            consecutive_failures = 0;
                            Duration::from_millis(config.app_restart_delay as u64)
                        };

                        if throttle_delay.as_millis() > 0 {
                            info!("Scheduling restart in {throttle_delay:?}");
                            restart_after = Some(Instant::now() + throttle_delay);
                        }

                        break 'inner; // Restart
                    } else {
                        info!("Not restarting application based on exit action");
                        break 'outer; // Exit service
                    }
                }
                Ok(ProcessStatus::Terminated) => {
                    info!("Application was terminated");
                    service_exit_code = ServiceExitCode::ServiceSpecific(259); // STILL_ACTIVE

                    // Always restart if terminated unexpectedly
                    let throttle_delay = Duration::from_millis(config.app_throttle as u64);
                    if throttle_delay.as_millis() > 0 {
                        restart_after = Some(Instant::now() + throttle_delay);
                    }
                    break 'inner;
                }
                Err(e) => {
                    error!("Error checking process status: {e}");
                    service_exit_code = ServiceExitCode::ServiceSpecific(1);
                    break 'outer;
                }
            }
        }

        // Wait for I/O threads to complete
        if let Some(thread) = stdout_thread {
            let _ = thread.join();
        }
        if let Some(thread) = stderr_thread {
            let _ = thread.join();
        }
    }

    info!("Service loop ended");

    // Set service status to stopped
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: service_exit_code,
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

fn check_process_status(child: &mut Child) -> Result<ProcessStatus, std::io::Error> {
    match child.try_wait() {
        Ok(None) => Ok(ProcessStatus::Running),
        Ok(Some(status)) => match status.code() {
            Some(code) => Ok(ProcessStatus::Exited(code)),
            None => Ok(ProcessStatus::Terminated),
        },
        Err(e) => Err(e),
    }
}

fn should_restart(_exit_code: i32, exit_action: &crate::cli::ExitAction) -> bool {
    match exit_action {
        crate::cli::ExitAction::Restart => true,
        crate::cli::ExitAction::Ignore => false,
        crate::cli::ExitAction::Exit => false,
    }
}

fn stop_child_process(child: &mut Child, config: &ServiceConfig, stop_ctrlc: &Arc<AtomicBool>) {
    info!("Stopping child process with PID: {}", child.id());

    // Enable ctrl-C for stopping
    stop_ctrlc.store(true, Ordering::SeqCst);

    let child_id = child.id();

    // Try console Ctrl+C first if not skipped
    if !config.app_no_console && (config.app_stop_method_skip & 1) == 0 {
        info!("Sending Ctrl-C to child process");
        unsafe {
            use windows::Win32::System::Console::{
                AttachConsole, FreeConsole, GenerateConsoleCtrlEvent,
            };

            // Try to attach to child's console
            if AttachConsole(child_id).is_ok() {
                let _ = GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0);
                let _ = FreeConsole();
            } else if GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0).is_err() {
                warn!("Failed to send Ctrl-C event");
            }
        }

        // Wait for console timeout
        let start = Instant::now();
        while start.elapsed().as_millis() < config.app_stop_method_console as u128 {
            match check_process_status(child) {
                Ok(ProcessStatus::Running) => {
                    thread::sleep(Duration::from_millis(50));
                }
                _ => {
                    info!("Child process stopped after Ctrl-C");
                    stop_ctrlc.store(false, Ordering::SeqCst);
                    return;
                }
            }
        }
    }

    // Try WM_CLOSE to main window if not skipped
    if (config.app_stop_method_skip & 2) == 0 {
        info!("Sending WM_CLOSE to child process windows");
        unsafe {
            use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
            use windows::Win32::UI::WindowsAndMessaging::{
                EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
            };

            unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
                unsafe {
                    let target_pid = lparam.0 as u32;
                    let mut window_pid = 0u32;
                    GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
                    if window_pid == target_pid {
                        let _ = PostMessageW(
                            hwnd,
                            WM_CLOSE,
                            windows::Win32::Foundation::WPARAM(0),
                            LPARAM(0),
                        );
                    }
                    BOOL::from(true)
                }
            }

            let _ = EnumWindows(Some(enum_window_proc), LPARAM(child_id as isize));
        }

        // Wait for window close timeout
        let start = Instant::now();
        while start.elapsed().as_millis() < config.app_stop_method_window as u128 {
            match check_process_status(child) {
                Ok(ProcessStatus::Running) => {
                    thread::sleep(Duration::from_millis(50));
                }
                _ => {
                    info!("Child process stopped after WM_CLOSE");
                    stop_ctrlc.store(false, Ordering::SeqCst);
                    return;
                }
            }
        }
    }

    // Try to terminate threads if not skipped
    if (config.app_stop_method_skip & 4) == 0 {
        info!("Terminating child process threads");
        unsafe {
            use windows::Win32::System::Threading::{
                OpenProcess, PROCESS_TERMINATE, TerminateProcess,
            };

            if let Ok(process_handle) = OpenProcess(PROCESS_TERMINATE, false, child_id) {
                let _ = TerminateProcess(process_handle, 1);
                let _ = windows::Win32::Foundation::CloseHandle(process_handle);
            }
        }

        // Wait for threads timeout
        let start = Instant::now();
        while start.elapsed().as_millis() < config.app_stop_method_threads as u128 {
            match check_process_status(child) {
                Ok(ProcessStatus::Running) => {
                    thread::sleep(Duration::from_millis(50));
                }
                _ => {
                    info!("Child process stopped after thread termination");
                    stop_ctrlc.store(false, Ordering::SeqCst);
                    return;
                }
            }
        }
    }

    // Finally try to kill the process if not skipped
    if (config.app_stop_method_skip & 8) == 0 {
        info!("Killing child process");
        if let Err(e) = child.kill() {
            warn!("Failed to kill child process: {e}");
        }
    }

    // Wait for process to die
    let _ = child.wait();
    stop_ctrlc.store(false, Ordering::SeqCst);
}

fn handle_stdout(stdout: std::process::ChildStdout, output_path: Option<std::path::PathBuf>) {
    let reader = BufReader::new(stdout);

    if let Some(path) = output_path {
        // Redirect to file
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(mut file) => {
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            info!("stdout: {line}");
                            if writeln!(file, "{line}").is_err() {
                                error!("Failed to write to stdout file");
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            Err(e) => {
                error!("Failed to open stdout file {path:?}: {e}");
            }
        }
    } else {
        // Just log to service log
        for line in reader.lines() {
            match line {
                Ok(line) => info!("stdout: {line}"),
                Err(_) => break,
            }
        }
    }
}

fn handle_stderr(stderr: std::process::ChildStderr, output_path: Option<std::path::PathBuf>) {
    let reader = BufReader::new(stderr);

    if let Some(path) = output_path {
        // Redirect to file
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(mut file) => {
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            warn!("stderr: {line}");
                            if writeln!(file, "{line}").is_err() {
                                error!("Failed to write to stderr file");
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            Err(e) => {
                error!("Failed to open stderr file {path:?}: {e}");
            }
        }
    } else {
        // Just log to service log
        for line in reader.lines() {
            match line {
                Ok(line) => warn!("stderr: {line}"),
                Err(_) => break,
            }
        }
    }
}

fn parse_command_line(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current_arg = String::new();
    let mut in_quotes = false;
    let chars = input.chars().peekable();

    for ch in chars {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' | '\t' => {
                if in_quotes {
                    current_arg.push(ch);
                } else if !current_arg.is_empty() {
                    args.push(current_arg.clone());
                    current_arg.clear();
                }
            }
            _ => {
                current_arg.push(ch);
            }
        }
    }

    if !current_arg.is_empty() {
        args.push(current_arg);
    }

    args
}
