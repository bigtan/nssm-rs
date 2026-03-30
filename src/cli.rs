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
