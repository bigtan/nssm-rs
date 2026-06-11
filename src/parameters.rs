use std::path::PathBuf;

use crate::config::{ExitAction, ProcessPriority, ServiceConfig, ServiceStartType};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceParameter {
    Application,
    AppDirectory,
    AppParameters,
    DisplayName,
    Description,
    Start,
    AppPriority,
    AppNoConsole,
    AppThrottle,
    AppStdout,
    AppStderr,
    AppStdin,
    AppStopMethod,
    AppStopMethodConsole,
    AppStopMethodWindow,
    AppStopMethodThreads,
    AppRestartDelay,
    AppExitAction,
    AppEnvironmentExtra,
}

impl ServiceParameter {
    pub fn parse(parameter: &str) -> AppResult<Self> {
        match parameter.to_uppercase().as_str() {
            "APPLICATION" => Ok(Self::Application),
            "APPDIRECTORY" => Ok(Self::AppDirectory),
            "APPPARAMETERS" => Ok(Self::AppParameters),
            "DISPLAYNAME" => Ok(Self::DisplayName),
            "DESCRIPTION" => Ok(Self::Description),
            "START" => Ok(Self::Start),
            "APPPRIORITY" => Ok(Self::AppPriority),
            "APPNOCONSOLE" => Ok(Self::AppNoConsole),
            "APPTHROTTLE" => Ok(Self::AppThrottle),
            "APPSTDOUT" => Ok(Self::AppStdout),
            "APPSTDERR" => Ok(Self::AppStderr),
            "APPSTDIN" => Ok(Self::AppStdin),
            "APPSTOPMETHOD" => Ok(Self::AppStopMethod),
            "APPSTOPMETHOD_CONSOLE" => Ok(Self::AppStopMethodConsole),
            "APPSTOPMETHOD_WINDOW" => Ok(Self::AppStopMethodWindow),
            "APPSTOPMETHOD_THREADS" => Ok(Self::AppStopMethodThreads),
            "APPRESTARTDELAY" => Ok(Self::AppRestartDelay),
            "APPEXITACTION" => Ok(Self::AppExitAction),
            "APPENVIRONMENTEXTRA" => Ok(Self::AppEnvironmentExtra),
            _ => Err(AppError::UnknownParameter(parameter.to_string())),
        }
    }

    pub fn default_value(self) -> String {
        match self {
            Self::Application
            | Self::AppDirectory
            | Self::AppParameters
            | Self::DisplayName
            | Self::Description
            | Self::AppStdout
            | Self::AppStderr
            | Self::AppStdin
            | Self::AppEnvironmentExtra => String::new(),
            Self::Start => "SERVICE_AUTO_START".to_string(),
            Self::AppPriority => "NORMAL_PRIORITY_CLASS".to_string(),
            Self::AppNoConsole => "0".to_string(),
            Self::AppThrottle => "1500".to_string(),
            Self::AppStopMethod => "0".to_string(),
            Self::AppStopMethodConsole => "1500".to_string(),
            Self::AppStopMethodWindow => "1500".to_string(),
            Self::AppStopMethodThreads => "1500".to_string(),
            Self::AppRestartDelay => "0".to_string(),
            Self::AppExitAction => "Restart".to_string(),
        }
    }

    pub fn apply(self, config: &mut ServiceConfig, value: &str) -> AppResult<()> {
        match self {
            Self::Application => {
                if value.is_empty() {
                    return Err(AppError::InvalidParameterValue {
                        parameter: self.as_str().to_string(),
                        value: value.to_string(),
                    });
                }
                config.application = PathBuf::from(value);
            }
            Self::AppDirectory => {
                config.app_directory = empty_to_none_path(value);
            }
            Self::AppParameters => {
                config.app_parameters = empty_to_none_string(value);
            }
            Self::DisplayName => {
                config.display_name = empty_to_none_string(value);
            }
            Self::Description => {
                config.description = empty_to_none_string(value);
            }
            Self::Start => {
                config.start_type = ServiceStartType::from_str(value).ok_or_else(|| {
                    AppError::InvalidParameterValue {
                        parameter: self.as_str().to_string(),
                        value: value.to_string(),
                    }
                })?;
            }
            Self::AppPriority => {
                config.app_priority = ProcessPriority::from_str(value).ok_or_else(|| {
                    AppError::InvalidParameterValue {
                        parameter: self.as_str().to_string(),
                        value: value.to_string(),
                    }
                })?;
            }
            Self::AppNoConsole => {
                config.app_no_console = value != "0";
            }
            Self::AppThrottle => {
                config.app_throttle = parse_u32(self, value)?;
            }
            Self::AppStdout => {
                config.app_stdout = empty_to_none_path(value);
            }
            Self::AppStderr => {
                config.app_stderr = empty_to_none_path(value);
            }
            Self::AppStdin => {
                config.app_stdin = empty_to_none_path(value);
            }
            Self::AppStopMethod => {
                config.app_stop_method_skip = parse_u32(self, value)?;
            }
            Self::AppStopMethodConsole => {
                config.app_stop_method_console = parse_u32(self, value)?;
            }
            Self::AppStopMethodWindow => {
                config.app_stop_method_window = parse_u32(self, value)?;
            }
            Self::AppStopMethodThreads => {
                config.app_stop_method_threads = parse_u32(self, value)?;
            }
            Self::AppRestartDelay => {
                config.app_restart_delay = parse_u32(self, value)?;
            }
            Self::AppExitAction => {
                config.app_exit_default =
                    ExitAction::from_str(value).ok_or_else(|| AppError::InvalidParameterValue {
                        parameter: self.as_str().to_string(),
                        value: value.to_string(),
                    })?;
            }
            Self::AppEnvironmentExtra => {
                config.app_environment_extra = value
                    .lines()
                    .filter(|line| !line.is_empty())
                    .map(str::to_string)
                    .collect();
            }
        }

        Ok(())
    }

    pub fn read(self, config: &ServiceConfig) -> String {
        match self {
            Self::Application => config.application.to_string_lossy().into_owned(),
            Self::AppDirectory => config
                .app_directory
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Self::AppParameters => config.app_parameters.clone().unwrap_or_default(),
            Self::DisplayName => config.display_name.clone().unwrap_or_default(),
            Self::Description => config.description.clone().unwrap_or_default(),
            Self::Start => config.start_type.as_cli_value().to_string(),
            Self::AppPriority => config.app_priority.as_cli_value().to_string(),
            Self::AppNoConsole => bool_to_flag(config.app_no_console),
            Self::AppThrottle => config.app_throttle.to_string(),
            Self::AppStdout => config
                .app_stdout
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Self::AppStderr => config
                .app_stderr
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Self::AppStdin => config
                .app_stdin
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Self::AppStopMethod => config.app_stop_method_skip.to_string(),
            Self::AppStopMethodConsole => config.app_stop_method_console.to_string(),
            Self::AppStopMethodWindow => config.app_stop_method_window.to_string(),
            Self::AppStopMethodThreads => config.app_stop_method_threads.to_string(),
            Self::AppRestartDelay => config.app_restart_delay.to_string(),
            Self::AppExitAction => config.app_exit_default.as_registry_value().to_string(),
            Self::AppEnvironmentExtra => config.app_environment_extra.join("\n"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Application => "APPLICATION",
            Self::AppDirectory => "APPDIRECTORY",
            Self::AppParameters => "APPPARAMETERS",
            Self::DisplayName => "DISPLAYNAME",
            Self::Description => "DESCRIPTION",
            Self::Start => "START",
            Self::AppPriority => "APPPRIORITY",
            Self::AppNoConsole => "APPNOCONSOLE",
            Self::AppThrottle => "APPTHROTTLE",
            Self::AppStdout => "APPSTDOUT",
            Self::AppStderr => "APPSTDERR",
            Self::AppStdin => "APPSTDIN",
            Self::AppStopMethod => "APPSTOPMETHOD",
            Self::AppStopMethodConsole => "APPSTOPMETHOD_CONSOLE",
            Self::AppStopMethodWindow => "APPSTOPMETHOD_WINDOW",
            Self::AppStopMethodThreads => "APPSTOPMETHOD_THREADS",
            Self::AppRestartDelay => "APPRESTARTDELAY",
            Self::AppExitAction => "APPEXITACTION",
            Self::AppEnvironmentExtra => "APPENVIRONMENTEXTRA",
        }
    }
}

fn parse_u32(parameter: ServiceParameter, value: &str) -> AppResult<u32> {
    value.parse().map_err(|_| AppError::InvalidParameterValue {
        parameter: parameter.as_str().to_string(),
        value: value.to_string(),
    })
}

fn empty_to_none_string(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn empty_to_none_path(value: &str) -> Option<PathBuf> {
    if value.is_empty() {
        None
    } else {
        Some(PathBuf::from(value))
    }
}

fn bool_to_flag(value: bool) -> String {
    if value { "1" } else { "0" }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_is_case_insensitive() {
        assert_eq!(
            ServiceParameter::parse("application").unwrap(),
            ServiceParameter::Application
        );
        assert_eq!(
            ServiceParameter::parse("AppExitAction").unwrap(),
            ServiceParameter::AppExitAction
        );
    }

    #[test]
    fn parse_rejects_unknown_parameter() {
        assert!(matches!(
            ServiceParameter::parse("NoSuchParameter"),
            Err(AppError::UnknownParameter(_))
        ));
    }

    #[test]
    fn apply_then_read_round_trips() {
        let mut config = ServiceConfig::default();

        for (parameter, value) in [
            (ServiceParameter::Application, r"C:\app.exe"),
            (ServiceParameter::AppParameters, "--port 80"),
            (ServiceParameter::AppThrottle, "3000"),
            (ServiceParameter::AppExitAction, "Ignore"),
            (ServiceParameter::AppNoConsole, "1"),
            (ServiceParameter::AppStdout, r"C:\logs\out.log"),
        ] {
            parameter.apply(&mut config, value).unwrap();
            assert_eq!(parameter.read(&config), value, "{}", parameter.as_str());
        }
    }

    #[test]
    fn apply_rejects_empty_application() {
        let mut config = ServiceConfig::default();
        assert!(matches!(
            ServiceParameter::Application.apply(&mut config, ""),
            Err(AppError::InvalidParameterValue { .. })
        ));
    }

    #[test]
    fn apply_rejects_invalid_values() {
        let mut config = ServiceConfig::default();
        assert!(
            ServiceParameter::Start
                .apply(&mut config, "SOMETIMES")
                .is_err()
        );
        assert!(
            ServiceParameter::AppThrottle
                .apply(&mut config, "abc")
                .is_err()
        );
        assert!(
            ServiceParameter::AppExitAction
                .apply(&mut config, "Reboot")
                .is_err()
        );
        assert!(
            ServiceParameter::AppPriority
                .apply(&mut config, "TURBO")
                .is_err()
        );
    }

    #[test]
    fn environment_extra_round_trips_through_lines() {
        let mut config = ServiceConfig::default();
        ServiceParameter::AppEnvironmentExtra
            .apply(&mut config, "A=1\nB=two words")
            .unwrap();
        assert_eq!(config.app_environment_extra, vec!["A=1", "B=two words"]);
        assert_eq!(
            ServiceParameter::AppEnvironmentExtra.read(&config),
            "A=1\nB=two words"
        );
    }

    #[test]
    fn start_default_matches_install_default() {
        use crate::config::ServiceStartType;

        let default = ServiceParameter::Start.default_value();
        assert_eq!(
            ServiceStartType::from_str(&default),
            Some(ServiceStartType::Auto)
        );
    }
}
