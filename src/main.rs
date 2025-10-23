mod cli;
mod service_manager;
mod service_runner;

use clap::Parser;
use cli::{Cli, Commands};
use log::{debug, error, info};
use service_manager::ServiceManager;
use service_runner::run_service;

#[cfg(windows)]
fn main() {
    let cli = Cli::parse();

    // 根据命令行参数设置日志级别
    let log_level = if cli.debug {
        log::LevelFilter::Debug
    } else if cli.verbose {
        log::LevelFilter::Info
    } else {
        log::LevelFilter::Warn
    };

    // 初始化日志记录器，支持环境变量配置日志级别
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

    info!("NSSM-RS starting up...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    if cli.debug {
        debug!("Debug mode enabled");
    }
    if cli.verbose {
        info!("Verbose mode enabled");
    }

    debug!("Command parsed: {:?}", std::mem::discriminant(&cli.command));

    let result = match cli.command {
        Commands::Install {
            service_name,
            application,
            arguments,
        } => {
            info!("Installing service '{service_name}' with application: {application:?}");
            if !arguments.is_empty() {
                info!("Application arguments: {arguments:?}");
            }
            let service_manager = ServiceManager::new().expect("Failed to create service manager");
            service_manager.install_service(&service_name, &application, &arguments)
        }
        Commands::Remove {
            service_name,
            confirm,
        } => {
            info!("Removing service '{service_name}'");
            if !confirm {
                info!("Confirmation will be required");
            }
            let service_manager = ServiceManager::new().expect("Failed to create service manager");
            service_manager.remove_service(&service_name, confirm)
        }
        Commands::Start { service_name } => {
            info!("Starting service '{service_name}'");
            let service_manager = ServiceManager::new().expect("Failed to create service manager");
            service_manager.start_service(&service_name)
        }
        Commands::Stop { service_name } => {
            info!("Stopping service '{service_name}'");
            let service_manager = ServiceManager::new().expect("Failed to create service manager");
            service_manager.stop_service(&service_name)
        }
        Commands::Restart { service_name } => {
            info!("Restarting service '{service_name}'");
            let service_manager = ServiceManager::new().expect("Failed to create service manager");
            info!("Stopping service first...");
            
            // Stop the service first, return error if it fails
            match service_manager.stop_service(&service_name) {
                Ok(_) => {
                    info!("Service stopped successfully");
                    info!("Waiting 2 seconds before starting...");
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    info!("Starting service...");
                    service_manager.start_service(&service_name)
                }
                Err(e) => {
                    error!("Failed to stop service: {e}");
                    Err(e)
                }
            }
        }
        Commands::Set {
            service_name,
            parameter,
            value,
        } => {
            info!("Setting parameter '{parameter}' = '{value}' for service '{service_name}'");
            let service_manager = ServiceManager::new().expect("Failed to create service manager");
            service_manager.set_service_parameter(&service_name, &parameter, &value)
        }
        Commands::Get {
            service_name,
            parameter,
        } => {
            info!("Getting parameter '{parameter}' for service '{service_name}'");
            let service_manager = ServiceManager::new().expect("Failed to create service manager");
            service_manager
                .get_service_parameter(&service_name, &parameter)
                .map(|_| ())
        }
        Commands::Reset {
            service_name,
            parameter,
        } => {
            info!("Resetting parameter '{parameter}' for service '{service_name}'");
            // Reset to default value
            let service_manager = ServiceManager::new().expect("Failed to create service manager");
            let default_value = get_default_parameter_value(&parameter);
            info!("Default value for '{parameter}': '{default_value}'");
            service_manager.set_service_parameter(&service_name, &parameter, &default_value)
        }
        Commands::Status { service_name } => {
            info!("Querying status for service '{service_name}'");
            let service_manager = ServiceManager::new().expect("Failed to create service manager");
            service_manager.query_service_status(&service_name)
        }
        Commands::List => {
            info!("Listing all NSSM-RS managed services");
            let service_manager = ServiceManager::new().expect("Failed to create service manager");
            service_manager.list_nssm_services()
        }
        Commands::Run { name } => {
            info!("Running as service: '{name}'");
            let service_name = name.clone();
            match run_service(name) {
                Ok(()) => {
                    info!("Service '{service_name}' completed successfully");
                    Ok(())
                }
                Err(e) => {
                    error!("Service '{service_name}' failed: {e:?}");
                    // Try to print to console in case this was run directly
                    eprintln!("Service failed: {e:?}");
                    std::process::exit(1);
                }
            }
        }
    };

    match &result {
        Ok(()) => {
            info!("Operation completed successfully");
        }
        Err(e) => {
            error!("Operation failed: {e}");
            eprintln!("Error: {e}");
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

fn get_default_parameter_value(parameter: &str) -> String {
    match parameter.to_uppercase().as_str() {
        "APPTHROTTLE" => "1500".to_string(),
        "APPSTOPMETHOD" => "0".to_string(),
        "APPSTOPMETHOD_CONSOLE" => "1500".to_string(),
        "APPSTOPMETHOD_WINDOW" => "1500".to_string(),
        "APPSTOPMETHOD_THREADS" => "1500".to_string(),
        "APPRESTARTDELAY" => "0".to_string(),
        "APPNOCONSOLE" => "0".to_string(),
        "APPPRIORITY" => "NORMAL_PRIORITY_CLASS".to_string(),
        "START" => "SERVICE_DEMAND_START".to_string(),
        "APPEXITACTION" => "Restart".to_string(),
        "DISPLAYNAME" => String::new(),
        "DESCRIPTION" => String::new(),
        "APPDIRECTORY" => String::new(),
        "APPPARAMETERS" => String::new(),
        "APPSTDOUT" => String::new(),
        "APPSTDERR" => String::new(),
        "APPSTDIN" => String::new(),
        _ => String::new(),
    }
}
