# NSSM-RS

A Rust implementation of NSSM (Non-Sucking Service Manager) - a tool to convert any console application into a Windows service.

## Features

- **Full Service Management**: Install, remove, start, stop, restart Windows services
- **Parameter Configuration**: Set and get service parameters like original NSSM
- **I/O Redirection**: Redirect stdout, stderr to files
- **Process Management**: Graceful shutdown with multiple stop methods
- **Restart Control**: Configurable restart behavior and throttling
- **Registry Integration**: Store configuration in Windows registry
- **CLI Compatibility**: Command-line interface compatible with original NSSM

## Quick Start

1. **Build the project**:
   ```powershell
   cargo build --release
   ```

2. **Install a service** (requires Administrator privileges):
   ```powershell
   nssm-rs install MyService "C:\Path\To\Your\Application.exe"
   ```

3. **Start the service**:
   ```powershell
   nssm-rs start MyService
   ```

4. **Check service status**:
   ```powershell
   nssm-rs status MyService
   ```

## Commands

### Service Management
- `install <service_name> <application> [arguments...]` - Install a new service
- `remove <service_name> [--confirm]` - Remove a service
- `start <service_name>` - Start a service
- `stop <service_name>` - Stop a service
- `restart <service_name>` - Restart a service
- `status <service_name>` - Query service status
- `list` - List all services managed by nssm-rs

### Configuration
- `set <service_name> <parameter> <value>` - Set service parameter
- `get <service_name> <parameter>` - Get service parameter
- `reset <service_name> <parameter>` - Reset parameter to default

## Supported Parameters

### Application Settings
- `Application` - Path to the application executable
- `AppDirectory` - Working directory (defaults to application directory)
- `AppParameters` - Command line arguments
- `DisplayName` - Service display name
- `Description` - Service description

### Process Control
- `AppPriority` - Process priority (NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS, etc.)
- `AppNoConsole` - Disable console allocation (0/1)
- `Start` - Service start type (SERVICE_AUTO_START, SERVICE_DEMAND_START, SERVICE_DISABLED)

### I/O Redirection
- `AppStdout` - Redirect stdout to file
- `AppStderr` - Redirect stderr to file
- `AppStdin` - Redirect stdin from file

### Restart Behavior
- `AppExitAction` - Action on exit (Restart, Ignore, Exit)
- `AppRestartDelay` - Delay before restart (milliseconds)
- `AppThrottle` - Minimum runtime before fast restart (milliseconds)

### Stop Methods
- `AppStopMethod` - Stop method flags (bitwise combination)
- `AppStopMethod_Console` - Console Ctrl+C timeout (milliseconds)
- `AppStopMethod_Window` - Window WM_CLOSE timeout (milliseconds)
- `AppStopMethod_Threads` - Thread termination timeout (milliseconds)

## Examples

### Basic Service Installation
```powershell
# Install a simple service
nssm-rs install WebServer "C:\MyApp\server.exe"

# Set working directory
nssm-rs set WebServer AppDirectory "C:\MyApp"

# Set up logging
nssm-rs set WebServer AppStdout "C:\Logs\server_out.log"
nssm-rs set WebServer AppStderr "C:\Logs\server_err.log"

# Start the service
nssm-rs start WebServer
```

### Advanced Configuration
```powershell
# Install with parameters
nssm-rs install APIService "C:\MyAPI\api.exe" --port 8080 --config production.json

# Configure auto-start
nssm-rs set APIService Start SERVICE_AUTO_START

# Set high priority
nssm-rs set APIService AppPriority HIGH_PRIORITY_CLASS

# Configure restart behavior
nssm-rs set APIService AppExitAction Restart
nssm-rs set APIService AppRestartDelay 5000
nssm-rs set APIService AppThrottle 2000

# Start the service
nssm-rs start APIService
```

### Python Script as Service
```powershell
nssm-rs install PythonService "python.exe" "C:\Scripts\my_service.py"
nssm-rs set PythonService AppDirectory "C:\Scripts"
nssm-rs start PythonService
```

## Architecture

NSSM-RS works in two modes:

1. **Management Mode**: When you run commands like `install`, `start`, etc., nssm-rs acts as a service management tool that interacts with Windows Service Control Manager.

2. **Service Mode**: When Windows starts a service created by nssm-rs, it actually starts nssm-rs itself with the `run` command, which then launches and manages your actual application as a child process.

## Key Features

### Graceful Shutdown
NSSM-RS attempts multiple methods to stop applications gracefully:
1. Send Ctrl+C signal to console applications
2. Send WM_CLOSE message to GUI windows
3. Terminate threads
4. Force process termination (as last resort)

### Restart Management
- Automatic restart on application exit
- Configurable restart delays
- Throttling to prevent rapid restart loops
- Different behaviors based on exit codes

### I/O Management
- Redirect application output to log files
- Handle both stdout and stderr
- Optional stdin redirection

## Requirements

- Windows 10/11 or Windows Server 2016+
- Administrator privileges for service management
- Rust 1.70+ for building from source

## Building

```powershell
# Clone and build
git clone <repository-url>
cd nssm-rs
cargo build --release
```

The executable will be created at `target/release/nssm-rs.exe`.

## Testing

See [USAGE.md](USAGE.md) for comprehensive testing examples and scenarios.

The project includes test applications:
- `examples/test-app/` - Rust test application
- `examples/test_service.py` - Python test script
- `examples/test_service.bat` - Batch test script

## Compatibility

NSSM-RS maintains command-line compatibility with the original NSSM while providing:
- Better error handling and reporting
- Improved performance through Rust
- Modern, maintainable codebase
- Enhanced logging and debugging capabilities

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
