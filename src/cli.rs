use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "nssm-rs")]
#[command(about = "A Rust implementation of NSSM - the Non-Sucking Service Manager")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
    
    /// Enable debug output
    #[arg(short, long, global = true)]
    pub debug: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Install a new service
    Install {
        /// Service name
        service_name: String,
        /// Application path
        application: PathBuf,
        /// Application arguments
        #[arg(trailing_var_arg = true)]
        arguments: Vec<String>,
    },
    /// Remove a service
    Remove {
        /// Service name
        service_name: String,
        /// Skip confirmation
        #[arg(short, long, action)]
        confirm: bool,
    },
    /// Start a service
    Start {
        /// Service name
        service_name: String,
    },
    /// Stop a service
    Stop {
        /// Service name
        service_name: String,
    },
    /// Restart a service
    Restart {
        /// Service name
        service_name: String,
    },
    /// Set service parameters
    Set {
        /// Service name
        service_name: String,
        /// Parameter name
        parameter: String,
        /// Parameter value
        value: String,
    },
    /// Get service parameters
    Get {
        /// Service name
        service_name: String,
        /// Parameter name
        parameter: String,
    },
    /// Reset service parameters to default
    Reset {
        /// Service name
        service_name: String,
        /// Parameter name
        parameter: String,
    },
    /// Query service status
    Status {
        /// Service name
        service_name: String,
    },
    /// List installed services (created by nssm-rs)
    List,
    /// Run as a service (internal command)
    #[command(hide = true)]
    Run {
        /// Service name
        name: String,
    },
}

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub application: PathBuf,
    pub app_directory: Option<PathBuf>,
    pub app_parameters: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub start_type: ServiceStartType,
    pub object_name: Option<String>,
    pub dependencies: Vec<String>,
    pub app_priority: ProcessPriority,
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
    pub app_rotate_files: bool,
    pub app_rotate_online: bool,
    pub app_rotate_seconds: u32,
    pub app_rotate_bytes: u64,
    pub app_environment: Vec<String>,
    pub app_environment_extra: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ServiceStartType {
    Auto,
    Manual,
    Disabled,
}

#[derive(Debug, Clone)]
pub enum ProcessPriority {
    Realtime,
    High,
    AboveNormal,
    Normal,
    BelowNormal,
    Idle,
}

#[derive(Debug, Clone)]
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
            start_type: ServiceStartType::Manual,
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
    pub fn to_windows_value(&self) -> u32 {
        match self {
            Self::Auto => 2,        // SERVICE_AUTO_START
            Self::Manual => 3,      // SERVICE_DEMAND_START
            Self::Disabled => 4,    // SERVICE_DISABLED
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
}

impl ProcessPriority {
    pub fn to_windows_value(&self) -> u32 {
        match self {
            Self::Realtime => 0x00000100,      // REALTIME_PRIORITY_CLASS
            Self::High => 0x00000080,          // HIGH_PRIORITY_CLASS
            Self::AboveNormal => 0x00008000,   // ABOVE_NORMAL_PRIORITY_CLASS
            Self::Normal => 0x00000020,        // NORMAL_PRIORITY_CLASS
            Self::BelowNormal => 0x00004000,   // BELOW_NORMAL_PRIORITY_CLASS
            Self::Idle => 0x00000040,          // IDLE_PRIORITY_CLASS
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

    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Restart => "Restart",
            Self::Ignore => "Ignore",
            Self::Exit => "Exit",
        }
    }
}
