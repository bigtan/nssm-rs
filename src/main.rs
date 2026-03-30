mod cli;
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
            let service_manager = ServiceManager::new()?;
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
            info!("Setting parameter '{parameter}' = '{value}' for service '{service_name}'");
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
            let default_value = parameter.default_value();
            info!(
                "Resetting parameter '{}' for service '{}' to '{}'",
                parameter.as_str(),
                service_name,
                default_value
            );
            service_manager.set_service_parameter(&service_name, parameter.as_str(), &default_value)
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

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .target(env_logger::Target::Stdout)
        .format(|buf, record| {
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
        })
        .init();

    if cli.debug {
        debug!("Debug mode enabled");
    }
    if cli.verbose {
        info!("Verbose mode enabled");
    }
}
