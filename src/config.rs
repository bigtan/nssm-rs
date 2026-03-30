use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub application: PathBuf,
    pub app_directory: Option<PathBuf>,
    pub app_parameters: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub start_type: ServiceStartType,
    #[allow(dead_code)]
    pub object_name: Option<String>,
    #[allow(dead_code)]
    pub dependencies: Vec<String>,
    pub app_priority: ProcessPriority,
    #[allow(dead_code)]
    pub app_affinity: Option<String>,
    pub app_no_console: bool,
    pub app_stop_method_skip: u32,
    pub app_stop_method_console: u32,
    pub app_stop_method_window: u32,
    pub app_stop_method_threads: u32,
    pub app_throttle: u32,
    pub app_exit_default: ExitAction,
    pub app_restart_delay: u32,
    pub app_stdout: Option<PathBuf>,
    pub app_stderr: Option<PathBuf>,
    pub app_stdin: Option<PathBuf>,
    #[allow(dead_code)]
    pub app_rotate_files: bool,
    #[allow(dead_code)]
    pub app_rotate_online: bool,
    #[allow(dead_code)]
    pub app_rotate_seconds: u32,
    #[allow(dead_code)]
    pub app_rotate_bytes: u64,
    #[allow(dead_code)]
    pub app_environment: Vec<String>,
    pub app_environment_extra: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStartType {
    Auto,
    Manual,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessPriority {
    Realtime,
    High,
    AboveNormal,
    Normal,
    BelowNormal,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitAction {
    Restart,
    Ignore,
    Exit,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            application: PathBuf::new(),
            app_directory: None,
            app_parameters: None,
            display_name: None,
            description: None,
            start_type: ServiceStartType::Auto,
            object_name: None,
            dependencies: Vec::new(),
            app_priority: ProcessPriority::Normal,
            app_affinity: None,
            app_no_console: false,
            app_stop_method_skip: 0,
            app_stop_method_console: 1500,
            app_stop_method_window: 1500,
            app_stop_method_threads: 1500,
            app_throttle: 1500,
            app_exit_default: ExitAction::Restart,
            app_restart_delay: 0,
            app_stdout: None,
            app_stderr: None,
            app_stdin: None,
            app_rotate_files: false,
            app_rotate_online: false,
            app_rotate_seconds: 86400,
            app_rotate_bytes: 1048576,
            app_environment: Vec::new(),
            app_environment_extra: Vec::new(),
        }
    }
}

impl ServiceStartType {
    pub fn to_windows_value(self) -> u32 {
        match self {
            Self::Auto => 2,
            Self::Manual => 3,
            Self::Disabled => 4,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "AUTO" | "SERVICE_AUTO_START" => Some(Self::Auto),
            "MANUAL" | "DEMAND" | "SERVICE_DEMAND_START" => Some(Self::Manual),
            "DISABLED" | "SERVICE_DISABLED" => Some(Self::Disabled),
            _ => None,
        }
    }

    pub fn as_cli_value(self) -> &'static str {
        match self {
            Self::Auto => "SERVICE_AUTO_START",
            Self::Manual => "SERVICE_DEMAND_START",
            Self::Disabled => "SERVICE_DISABLED",
        }
    }
}

impl ProcessPriority {
    pub fn to_windows_value(self) -> u32 {
        match self {
            Self::Realtime => 0x00000100,
            Self::High => 0x00000080,
            Self::AboveNormal => 0x00008000,
            Self::Normal => 0x00000020,
            Self::BelowNormal => 0x00004000,
            Self::Idle => 0x00000040,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "REALTIME" | "REALTIME_PRIORITY_CLASS" => Some(Self::Realtime),
            "HIGH" | "HIGH_PRIORITY_CLASS" => Some(Self::High),
            "ABOVENORMAL" | "ABOVE_NORMAL_PRIORITY_CLASS" => Some(Self::AboveNormal),
            "NORMAL" | "NORMAL_PRIORITY_CLASS" => Some(Self::Normal),
            "BELOWNORMAL" | "BELOW_NORMAL_PRIORITY_CLASS" => Some(Self::BelowNormal),
            "IDLE" | "IDLE_PRIORITY_CLASS" => Some(Self::Idle),
            _ => None,
        }
    }

    pub fn from_windows_value(value: u32) -> Self {
        match value {
            0x00000100 => Self::Realtime,
            0x00000080 => Self::High,
            0x00008000 => Self::AboveNormal,
            0x00004000 => Self::BelowNormal,
            0x00000040 => Self::Idle,
            _ => Self::Normal,
        }
    }

    pub fn as_cli_value(self) -> &'static str {
        match self {
            Self::Realtime => "REALTIME_PRIORITY_CLASS",
            Self::High => "HIGH_PRIORITY_CLASS",
            Self::AboveNormal => "ABOVE_NORMAL_PRIORITY_CLASS",
            Self::Normal => "NORMAL_PRIORITY_CLASS",
            Self::BelowNormal => "BELOW_NORMAL_PRIORITY_CLASS",
            Self::Idle => "IDLE_PRIORITY_CLASS",
        }
    }
}

impl ExitAction {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "RESTART" => Some(Self::Restart),
            "IGNORE" => Some(Self::Ignore),
            "EXIT" => Some(Self::Exit),
            _ => None,
        }
    }

    pub fn as_registry_value(self) -> &'static str {
        match self {
            Self::Restart => "Restart",
            Self::Ignore => "Ignore",
            Self::Exit => "Exit",
        }
    }
}
