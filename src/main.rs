mod cli;
mod cmdline;
mod config;
mod error;
mod parameters;
mod registry;
mod service_manager;
mod service_runner;

use clap::Parser;
use cli::{Cli, Commands};
use error::AppResult;
use log::{debug, error, info};
use parameters::ServiceParameter;
use service_manager::ServiceManager;
use service_runner::run_service;

#[cfg(windows)]
fn main() {
    let cli = Cli::parse();
    init_logging(&cli);

    info!("NSSM-RS starting up...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    debug!("Command parsed: {:?}", std::mem::discriminant(&cli.command));

    let result = run(cli);

    match &result {
        Ok(()) => info!("Operation completed successfully"),
        Err(error) => {
            error!("Operation failed: {error}");
            eprintln!("Error: {error}");
        }
    }

    if result.is_err() {
        std::process::exit(1);
    }

    info!("NSSM-RS shutting down normally");
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This application is only supported on Windows");
    std::process::exit(1);
}

#[cfg(windows)]
fn run(cli: Cli) -> AppResult<()> {
    match cli.command {
        Commands::Run { name } => run_service(name),
        command => {
            let service_manager = match command {
                Commands::Install { .. } => ServiceManager::new_for_install()?,
                _ => ServiceManager::new()?,
            };
            execute_command(&service_manager, command)
        }
    }
}

#[cfg(windows)]
fn execute_command(service_manager: &ServiceManager, command: Commands) -> AppResult<()> {
    match command {
        Commands::Install {
            service_name,
            application,
            arguments,
        } => {
            info!("Installing service '{service_name}' with application: {application:?}");
            if !arguments.is_empty() {
                info!("Application arguments: {arguments:?}");
            }
            service_manager.install_service(&service_name, &application, &arguments)
        }
        Commands::Remove {
            service_name,
            confirm,
        } => {
            info!("Removing service '{service_name}'");
            service_manager.remove_service(&service_name, confirm)
        }
        Commands::Start { service_name } => {
            info!("Starting service '{service_name}'");
            service_manager.start_service(&service_name)
        }
        Commands::Stop { service_name } => {
            info!("Stopping service '{service_name}'");
            service_manager.stop_service(&service_name)
        }
        Commands::Restart { service_name } => {
            info!("Restarting service '{service_name}'");
            service_manager.restart_service(&service_name)
        }
        Commands::Set {
            service_name,
            parameter,
            value,
        } => {
            info!(
                "Setting parameter '{parameter}' = '{}' for service '{service_name}'",
                value.join(" ")
            );
            service_manager.set_service_parameter(&service_name, &parameter, &value)
        }
        Commands::Get {
            service_name,
            parameter,
        } => {
            info!("Getting parameter '{parameter}' for service '{service_name}'");
            service_manager
                .get_service_parameter(&service_name, &parameter)
                .map(|_| ())
        }
        Commands::Reset {
            service_name,
            parameter,
        } => {
            let parameter = ServiceParameter::parse(&parameter)?;
            if parameter == ServiceParameter::Application {
                return Err(error::AppError::Message(
                    "APPLICATION cannot be reset to an empty value; set a new path instead"
                        .to_string(),
                ));
            }
            let default_value = parameter.default_value();
            info!(
                "Resetting parameter '{}' for service '{}' to '{}'",
                parameter.as_str(),
                service_name,
                default_value
            );
            service_manager.set_service_parameter(
                &service_name,
                parameter.as_str(),
                &[default_value],
            )
        }
        Commands::Status { service_name } => {
            info!("Querying status for service '{service_name}'");
            service_manager.query_service_status(&service_name)
        }
        Commands::List => {
            info!("Listing all NSSM-RS managed services");
            service_manager.list_nssm_services()
        }
        Commands::Run { .. } => unreachable!(),
    }
}

#[cfg(windows)]
fn init_logging(cli: &Cli) {
    let log_level = if cli.debug {
        log::LevelFilter::Debug
    } else if cli.verbose {
        log::LevelFilter::Info
    } else {
        log::LevelFilter::Warn
    };

    let mut builder = env_logger::Builder::from_default_env();
    builder.format(|buf, record| {
        use std::io::Write;

        let local_time = chrono::Local::now();
        writeln!(
            buf,
            "[{} {} {}:{}] {}",
            local_time.format("%Y-%m-%d %H:%M:%S"),
            record.level(),
            record.file().unwrap_or("unknown"),
            record.line().unwrap_or(0),
            record.args()
        )
    });

    if let Commands::Run { name } = &cli.command {
        // When running as a service, stdout only reaches a hidden console;
        // log to a per-service file so launches, exits and stop sequences
        // can actually be diagnosed.
        builder.filter_level(if cli.debug {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        });
        match open_service_log_file(name) {
            Some(file) => {
                builder.target(env_logger::Target::Pipe(Box::new(file)));
            }
            None => {
                builder.target(env_logger::Target::Stdout);
            }
        }
    } else {
        builder.filter_level(log_level);
        builder.target(env_logger::Target::Stdout);
    }

    builder.init();

    if cli.debug {
        debug!("Debug mode enabled");
    }
    if cli.verbose {
        info!("Verbose mode enabled");
    }
}

/// Opens %ProgramData%\nssm-rs\logs\<service>.log for appending.
#[cfg(windows)]
fn open_service_log_file(service_name: &str) -> Option<std::fs::File> {
    use std::path::PathBuf;

    let base = std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    let dir = base.join("nssm-rs").join("logs");
    std::fs::create_dir_all(&dir).ok()?;

    let sanitized: String = service_name
        .chars()
        .map(|ch| {
            if matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                ch
            }
        })
        .collect();

    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{sanitized}.log")))
        .ok()
}
