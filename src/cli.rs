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
    #[arg(short, long)]
    pub verbose: bool,

    /// Enable debug output
    #[arg(short, long)]
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
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
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
        /// Parameter value. AppEnvironmentExtra accepts multiple KEY=VALUE
        /// entries; AppParameters accepts multiple arguments which are
        /// quoted and joined.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        value: Vec<String>,
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

#[cfg(test)]
mod tests {
    use super::{Cli, Commands};
    use clap::Parser;
    use std::path::PathBuf;

    #[test]
    fn install_keeps_hyphenated_application_arguments() {
        let cli = Cli::parse_from([
            "nssm-rs",
            "install",
            "Clash",
            r"D:\Tools\Clash\clash.exe",
            "-d",
            r"D:\Tools\Clash",
        ]);

        match cli.command {
            Commands::Install {
                service_name,
                application,
                arguments,
            } => {
                assert_eq!(service_name, "Clash");
                assert_eq!(application, PathBuf::from(r"D:\Tools\Clash\clash.exe"));
                assert_eq!(arguments, vec!["-d", r"D:\Tools\Clash"]);
            }
            _ => panic!("expected install command"),
        }
    }

    #[test]
    fn debug_option_still_works_before_subcommand() {
        let cli = Cli::parse_from([
            "nssm-rs",
            "--debug",
            "install",
            "Clash",
            r"D:\Tools\Clash\clash.exe",
            "-d",
            r"D:\Tools\Clash",
        ]);

        assert!(cli.debug);
        match cli.command {
            Commands::Install { arguments, .. } => {
                assert_eq!(arguments, vec!["-d", r"D:\Tools\Clash"]);
            }
            _ => panic!("expected install command"),
        }
    }
}
