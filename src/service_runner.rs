use std::ffi::OsString;
use std::io::Write;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use log::{debug, error, info, warn};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Console::{AllocConsole, CTRL_C_EVENT};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows::Win32::System::Threading::{PROCESS_CREATION_FLAGS, SetPriorityClass};
use windows::core::PCWSTR;
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

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

define_windows_service!(ffi_service_main, service_main);

#[derive(Debug)]
enum ProcessStatus {
    Running,
    Exited(i32),
    Unknown(std::io::Error),
}

enum LoopControl {
    Restart(Option<Instant>, ServiceExitCode),
    Exit(ServiceExitCode),
    /// The application exited but the service keeps running without it
    /// (AppExitAction=Ignore) until a stop is requested.
    Idle(ServiceExitCode),
}

struct RunningChild {
    child: Child,
    _job: ChildJob,
    stdout_thread: Option<thread::JoinHandle<()>>,
    stderr_thread: Option<thread::JoinHandle<()>>,
}

struct ChildJob {
    handle: HANDLE,
}

impl Drop for ChildJob {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
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

    if let Err(error) = set_start_pending_status(&status_handle) {
        warn!("Failed to report START_PENDING to the SCM: {error}");
    }

    // The SCM must always be told the service stopped, even when the loop
    // bails out with an error, otherwise the service hangs in its last
    // reported state.
    let exit_code = match service_loop(&status_handle, &shutdown_rx, &service_name) {
        Ok(exit_code) => exit_code,
        Err(error) => {
            error!("Service '{service_name}' failed: {error}");
            ServiceExitCode::ServiceSpecific(1)
        }
    };

    set_stopped_status(&status_handle, exit_code)?;
    Ok(())
}

fn service_loop(
    status_handle: &ServiceStatusHandle,
    shutdown_rx: &mpsc::Receiver<()>,
    service_name: &str,
) -> AppResult<ServiceExitCode> {
    let config = crate::service_manager::load_service_config(service_name)?;
    install_ctrlc_guard()?;

    let mut restart_after: Option<Instant> = None;
    let mut consecutive_failures = 0u32;
    let mut reported_running = false;

    loop {
        if wait_for_restart_delay(shutdown_rx, restart_after)? {
            return Ok(ServiceExitCode::NO_ERROR);
        }

        let mut running_child = match launch_child(&config) {
            Ok(child) => child,
            Err(error) => {
                error!("Failed to launch application: {error}");
                return Ok(ServiceExitCode::ServiceSpecific(1));
            }
        };

        // Only report RUNNING once the application has actually been
        // launched; a broken configuration fails the start instead of
        // flapping RUNNING -> STOPPED.
        if !reported_running {
            set_running_status(status_handle)?;
            reported_running = true;
        }

        let control = monitor_child(
            status_handle,
            shutdown_rx,
            &config,
            &mut running_child.child,
            &mut consecutive_failures,
        );
        finalize_child_threads(running_child);

        match control {
            LoopControl::Restart(next_restart, _exit_code) => {
                restart_after = next_restart;
            }
            LoopControl::Exit(exit_code) => return Ok(exit_code),
            LoopControl::Idle(exit_code) => {
                info!("AppExitAction=Ignore: service stays running until stopped");
                let _ = shutdown_rx.recv();
                return Ok(exit_code);
            }
        }
    }
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

fn set_start_pending_status(status_handle: &ServiceStatusHandle) -> AppResult<()> {
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::NO_ERROR,
        checkpoint: 0,
        wait_hint: Duration::from_secs(10),
        process_id: None,
    })?;
    Ok(())
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

fn set_stop_pending_status(
    status_handle: &ServiceStatusHandle,
    checkpoint: u32,
    wait_hint: Duration,
) -> AppResult<()> {
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::StopPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::NO_ERROR,
        checkpoint,
        wait_hint,
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
    let mut command = build_command(config)?;
    let mut child = command.spawn()?;
    let child_id = child.id();
    info!("Application launched with PID: {child_id}");

    let job = match create_child_job().and_then(|job| {
        assign_child_to_job(&job, &child)?;
        Ok(job)
    }) {
        Ok(job) => job,
        Err(error) => {
            error!("Failed to attach child process {child_id} to cleanup job: {error}");
            kill_child_after_launch_failure(&mut child);
            return Err(error);
        }
    };

    set_child_priority(child_id, config)?;

    let stdout_thread = spawn_output_thread(child.stdout.take(), config.app_stdout.clone(), false);
    let stderr_thread = spawn_output_thread(child.stderr.take(), config.app_stderr.clone(), true);

    Ok(RunningChild {
        child,
        _job: job,
        stdout_thread,
        stderr_thread,
    })
}

fn create_child_job() -> AppResult<ChildJob> {
    unsafe {
        let job = ChildJob {
            handle: CreateJobObjectW(None, PCWSTR::null())?,
        };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        SetInformationJobObject(
            job.handle,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )?;

        Ok(job)
    }
}

fn assign_child_to_job(job: &ChildJob, child: &Child) -> AppResult<()> {
    unsafe {
        AssignProcessToJobObject(job.handle, HANDLE(child.as_raw_handle()))?;
    }

    Ok(())
}

fn kill_child_after_launch_failure(child: &mut Child) {
    if let Err(error) = child.kill() {
        warn!(
            "Failed to kill child process {} after launch setup failed: {error}",
            child.id()
        );
    }
    let _ = child.wait();
}

fn build_command(config: &ServiceConfig) -> AppResult<Command> {
    let mut command = Command::new(&config.application);
    command.current_dir(resolve_working_dir(config));

    if let Some(parameters) = &config.app_parameters {
        command.args(crate::cmdline::parse_command_line(parameters));
    }

    configure_stdio(&mut command, config)?;

    for env_var in &config.app_environment_extra {
        if let Some((key, value)) = env_var.split_once('=') {
            command.env(key, value);
        }
    }

    Ok(command)
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

fn configure_stdio(command: &mut Command, config: &ServiceConfig) -> AppResult<()> {
    if let Some(path) = &config.app_stdin {
        let file = std::fs::File::open(path).map_err(|error| {
            AppError::Message(format!(
                "Failed to open AppStdin file '{}': {error}",
                path.display()
            ))
        })?;
        command.stdin(Stdio::from(file));
    }

    if config.app_stdout.is_some() || config.app_stderr.is_some() {
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
    } else if config.app_no_console {
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
    }

    Ok(())
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
    stream.map(|stream| thread::spawn(move || pump_output(stream, output_path, is_stderr)))
}

/// Drain a child output pipe for the lifetime of the process.
///
/// Copies raw bytes: output must not be assumed to be UTF-8, and the pipe
/// must be drained even when the redirection file cannot be written,
/// otherwise the pipe fills up and blocks the child.
fn pump_output<T: std::io::Read>(mut stream: T, output_path: Option<PathBuf>, is_stderr: bool) {
    let stream_name = if is_stderr { "stderr" } else { "stdout" };
    let mut file = output_path.as_ref().and_then(|path| {
        match std::fs::OpenOptions::new().create(true).append(true).open(path) {
            Ok(file) => Some(file),
            Err(error) => {
                error!(
                    "Failed to open {stream_name} redirection file {path:?}: {error}; output will be discarded"
                );
                None
            }
        }
    });

    let mut buffer = [0u8; 8192];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                let chunk = &buffer[..count];
                if let Some(open_file) = file.as_mut() {
                    if let Err(error) = open_file.write_all(chunk) {
                        error!(
                            "Failed to write {stream_name} redirection output: {error}; output will be discarded"
                        );
                        file = None;
                    }
                } else if output_path.is_none() {
                    let text = String::from_utf8_lossy(chunk);
                    let text = text.trim_end_matches(['\r', '\n']);
                    if !text.is_empty() {
                        if is_stderr {
                            warn!("stderr: {text}");
                        } else {
                            info!("stdout: {text}");
                        }
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

fn monitor_child(
    status_handle: &ServiceStatusHandle,
    shutdown_rx: &mpsc::Receiver<()>,
    config: &ServiceConfig,
    child: &mut Child,
    consecutive_failures: &mut u32,
) -> LoopControl {
    let start_time = Instant::now();

    loop {
        match shutdown_rx.recv_timeout(Duration::from_secs(1)) {
            Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                info!("Shutting down service");
                report_stop_progress(status_handle, &mut 0, stop_wait_hint(config));
                stop_child_process(status_handle, child, config);
                return LoopControl::Exit(ServiceExitCode::NO_ERROR);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        match check_process_status(child) {
            ProcessStatus::Running => continue,
            ProcessStatus::Exited(exit_code) => {
                let runtime = start_time.elapsed();
                info!("Application exited with code {exit_code} after {runtime:?}");
                let service_exit_code = exit_code_to_service_code(exit_code);

                return match config.app_exit_default {
                    ExitAction::Restart => LoopControl::Restart(
                        calculate_restart_delay(config, runtime, consecutive_failures),
                        service_exit_code,
                    ),
                    ExitAction::Ignore => LoopControl::Idle(service_exit_code),
                    ExitAction::Exit => LoopControl::Exit(service_exit_code),
                };
            }
            ProcessStatus::Unknown(error) => {
                error!("Failed to query child process status: {error}");
                report_stop_progress(status_handle, &mut 0, stop_wait_hint(config));
                stop_child_process(status_handle, child, config);
                return LoopControl::Exit(ServiceExitCode::ServiceSpecific(1));
            }
        }
    }
}

fn calculate_restart_delay(
    config: &ServiceConfig,
    runtime: Duration,
    consecutive_failures: &mut u32,
) -> Option<Instant> {
    let delay = restart_delay_duration(config, runtime, consecutive_failures);
    (delay.as_millis() > 0).then(|| Instant::now() + delay)
}

/// Exponential backoff for rapidly failing applications: runtimes below
/// AppThrottle count as failures and back off 2^n seconds (capped at
/// 256s); a healthy runtime resets the counter and uses AppRestartDelay.
fn restart_delay_duration(
    config: &ServiceConfig,
    runtime: Duration,
    consecutive_failures: &mut u32,
) -> Duration {
    if runtime.as_millis() < config.app_throttle as u128 {
        *consecutive_failures += 1;
        let millis = (2u64.pow((*consecutive_failures).min(8))) * 1000;
        Duration::from_millis(millis.min(256000))
    } else {
        *consecutive_failures = 0;
        Duration::from_millis(config.app_restart_delay as u64)
    }
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

fn check_process_status(child: &mut Child) -> ProcessStatus {
    match child.try_wait() {
        Ok(None) => ProcessStatus::Running,
        // On Windows ExitStatus::code() is always Some; a process killed via
        // TerminateProcess reports the exit code passed to that call.
        Ok(Some(status)) => ProcessStatus::Exited(status.code().unwrap_or(1)),
        Err(error) => ProcessStatus::Unknown(error),
    }
}

/// Grace period after TerminateProcess before giving up waiting.
const KILL_WAIT_MS: u32 = 5000;

/// Total stop budget reported to the SCM, covering every enabled stop
/// method plus the kill grace period.
fn stop_wait_hint(config: &ServiceConfig) -> Duration {
    let mut total_ms = u64::from(KILL_WAIT_MS) + 2000;
    if !config.app_no_console && (config.app_stop_method_skip & 1) == 0 {
        total_ms += u64::from(config.app_stop_method_console);
    }
    if (config.app_stop_method_skip & 2) == 0 {
        total_ms += u64::from(config.app_stop_method_window);
    }
    if (config.app_stop_method_skip & 4) == 0 {
        total_ms += u64::from(config.app_stop_method_threads);
    }
    Duration::from_millis(total_ms)
}

fn report_stop_progress(
    status_handle: &ServiceStatusHandle,
    checkpoint: &mut u32,
    wait_hint: Duration,
) {
    if let Err(error) = set_stop_pending_status(status_handle, *checkpoint, wait_hint) {
        warn!("Failed to report stop progress to the SCM: {error}");
    }
    *checkpoint += 1;
}

fn process_running(child: &mut Child) -> bool {
    matches!(check_process_status(child), ProcessStatus::Running)
}

/// Escalating stop sequence: Ctrl-C, WM_CLOSE, WM_QUIT, TerminateProcess.
///
/// Each step is skipped if the corresponding AppStopMethodSkip bit is set
/// or the process has already exited. All signalling is done while the
/// child handle is held, so the PID cannot be recycled by another process
/// mid-sequence.
fn stop_child_process(
    status_handle: &ServiceStatusHandle,
    child: &mut Child,
    config: &ServiceConfig,
) {
    let child_id = child.id();
    info!("Stopping child process with PID: {child_id}");
    let wait_hint = stop_wait_hint(config);
    let mut checkpoint = 1u32;

    if !process_running(child) {
        info!("Child process already exited");
        return;
    }

    if !config.app_no_console && (config.app_stop_method_skip & 1) == 0 {
        report_stop_progress(status_handle, &mut checkpoint, wait_hint);
        send_console_ctrl_c(child_id);
        if wait_for_process_exit(child, config.app_stop_method_console) {
            info!("Child process stopped after Ctrl-C");
            return;
        }
    }

    if (config.app_stop_method_skip & 2) == 0 && process_running(child) {
        report_stop_progress(status_handle, &mut checkpoint, wait_hint);
        post_close_to_windows(child_id);
        if wait_for_process_exit(child, config.app_stop_method_window) {
            info!("Child process stopped after WM_CLOSE");
            return;
        }
    }

    if (config.app_stop_method_skip & 4) == 0 && process_running(child) {
        report_stop_progress(status_handle, &mut checkpoint, wait_hint);
        post_quit_to_threads(child_id);
        if wait_for_process_exit(child, config.app_stop_method_threads) {
            info!("Child process stopped after WM_QUIT");
            return;
        }
    }

    if (config.app_stop_method_skip & 8) == 0 && process_running(child) {
        report_stop_progress(status_handle, &mut checkpoint, wait_hint);
        info!("Terminating child process");
        if let Err(error) = child.kill() {
            warn!("Failed to kill child process: {error}");
        }
        if wait_for_process_exit(child, KILL_WAIT_MS) {
            info!("Child process terminated");
            return;
        }
    }

    if process_running(child) {
        warn!(
            "Child process {child_id} is still running after the stop sequence; \
             the job object will terminate it when the service exits"
        );
    }
}

fn send_console_ctrl_c(child_id: u32) {
    info!("Sending Ctrl-C to child process");
    unsafe {
        use windows::Win32::System::Console::{
            AllocConsole, AttachConsole, FreeConsole, GenerateConsoleCtrlEvent,
        };

        // A process can only be attached to one console at a time and the
        // service allocates its own at startup, so detach first. If the
        // child shares our console this re-attaches to the same one.
        let _ = FreeConsole();
        if AttachConsole(child_id).is_ok() {
            // Process group 0 signals every process attached to the console;
            // our own empty ctrl-c handler ignores it for this process.
            if GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0).is_err() {
                warn!("Failed to send Ctrl-C event");
            }
            let _ = FreeConsole();
        } else {
            warn!("Failed to attach to child process console; skipping Ctrl-C");
        }
        // Restore a console for subsequent children and stop attempts.
        let _ = AllocConsole();
    }
}

fn post_close_to_windows(child_id: u32) {
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
}

fn post_quit_to_threads(child_id: u32) {
    info!("Posting WM_QUIT to child process threads");
    unsafe {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        };
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};

        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                warn!("Failed to snapshot threads for WM_QUIT: {error}");
                return;
            }
        };

        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        if Thread32First(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32OwnerProcessID == child_id {
                    let _ = PostThreadMessageW(entry.th32ThreadID, WM_QUIT, WPARAM(0), LPARAM(0));
                }
                entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
                if Thread32Next(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
}

fn wait_for_process_exit(child: &mut Child, timeout_ms: u32) -> bool {
    let start = Instant::now();
    while start.elapsed().as_millis() < timeout_ms as u128 {
        match check_process_status(child) {
            ProcessStatus::Running => thread::sleep(Duration::from_millis(50)),
            _ => return true,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ServiceConfig {
        ServiceConfig {
            app_throttle: 1500,
            app_restart_delay: 0,
            ..Default::default()
        }
    }

    #[test]
    fn fast_failures_back_off_exponentially() {
        let config = test_config();
        let mut failures = 0;
        let fast_exit = Duration::from_millis(100);

        assert_eq!(
            restart_delay_duration(&config, fast_exit, &mut failures),
            Duration::from_secs(2)
        );
        assert_eq!(
            restart_delay_duration(&config, fast_exit, &mut failures),
            Duration::from_secs(4)
        );
        assert_eq!(
            restart_delay_duration(&config, fast_exit, &mut failures),
            Duration::from_secs(8)
        );
        assert_eq!(failures, 3);
    }

    #[test]
    fn backoff_is_capped_at_256_seconds() {
        let config = test_config();
        let mut failures = 20;
        assert_eq!(
            restart_delay_duration(&config, Duration::from_millis(1), &mut failures),
            Duration::from_secs(256)
        );
    }

    #[test]
    fn healthy_runtime_resets_failure_counter() {
        let mut config = test_config();
        config.app_restart_delay = 500;
        let mut failures = 5;

        let delay = restart_delay_duration(&config, Duration::from_secs(60), &mut failures);
        assert_eq!(delay, Duration::from_millis(500));
        assert_eq!(failures, 0);
    }

    #[test]
    fn zero_restart_delay_means_immediate_restart() {
        let config = test_config();
        let mut failures = 0;
        assert!(calculate_restart_delay(&config, Duration::from_secs(60), &mut failures).is_none());
    }
}
