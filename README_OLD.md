# NSSM-RS

A Rust implementation of NSSM (Non-Sucking Service Manager) for Windows. This tool allows you to run any console application as a Windows service.

## Features

- **Install services**: Convert any console application into a Windows service
- **Service management**: Start, stop, restart, and remove services
- **Configuration**: Set and get service parameters like nssm
- **Process management**: Proper handling of child processes with graceful shutdown
- **I/O redirection**: Redirect application output to files
- **Registry integration**: Store service configuration in Windows registry
- **Compatible interface**: Command-line interface similar to original NSSM

## Installation

1. Build from source:
```bash
cargo build --release
```

2. Copy the executable to a permanent location (e.g., `C:\Program Files\nssm-rs\`)

## Usage

### Install a Service

Install a Python script as a service:
```cmd
nssm-rs install my-python-app "C:\Python39\python.exe" "C:\path\to\script.py" --arg1 value1
```

Install a simple executable:
```cmd
nssm-rs install my-service "C:\path\to\app.exe"
```

### Manage Services

Start a service:
```cmd
nssm-rs start my-service
```

Stop a service:
```cmd
nssm-rs stop my-service
```

Restart a service:
```cmd
nssm-rs restart my-service
```

Remove a service:
```cmd
nssm-rs remove my-service
```

Remove without confirmation:
```cmd
nssm-rs remove my-service --confirm
```

### Configure Services

Set application parameters:
```cmd
nssm-rs set my-service AppParameters "--config /path/to/config.json"
nssm-rs set my-service AppDirectory "C:\app\workdir"
nssm-rs set my-service AppStdout "C:\logs\app.log"
nssm-rs set my-service AppStderr "C:\logs\app_error.log"
```

Get service configuration:
```cmd
nssm-rs get my-service Application
nssm-rs get my-service AppParameters
nssm-rs get my-service AppDirectory
```

Reset parameter to default:
```cmd
nssm-rs reset my-service AppThrottle
```

## Supported Parameters

### Basic Configuration
- `Application`: Path to the executable
- `AppDirectory`: Working directory (defaults to application directory)
- `AppParameters`: Command line arguments
- `DisplayName`: Service display name
- `Description`: Service description
- `Start`: Service start type (AUTO, MANUAL, DISABLED)

### Process Management
- `AppPriority`: Process priority (NORMAL, HIGH, LOW, etc.)
- `AppNoConsole`: Disable console window (0/1)
- `AppThrottle`: Restart throttling delay in milliseconds

### I/O Redirection
- `AppStdout`: Redirect stdout to file
- `AppStderr`: Redirect stderr to file
- `AppStdin`: Redirect stdin from file

## How It Works

1. **Service Installation**: When you install a service, nssm-rs registers itself as the service executable with Windows Service Manager
2. **Service Execution**: When the service starts, Windows runs nssm-rs with the `run` command
3. **Process Management**: nssm-rs loads the configuration from registry and launches your application as a child process
4. **Lifecycle Management**: nssm-rs handles service start/stop events and manages the child process accordingly
5. **Graceful Shutdown**: On service stop, nssm-rs sends Ctrl+C to the child process and waits before force terminating

## Working Directory

By default, the working directory is set to the directory containing the application executable. This ensures that relative paths in your application work correctly.

## Examples

### Python Web Server
```cmd
# Install a Flask app as a service
nssm-rs install flask-app "C:\Python39\python.exe" "C:\myapp\app.py"
nssm-rs set flask-app AppDirectory "C:\myapp"
nssm-rs set flask-app AppStdout "C:\logs\flask.log"
nssm-rs start flask-app
```

### Node.js Application
```cmd
# Install a Node.js app as a service
nssm-rs install node-app "C:\Program Files\nodejs\node.exe" "C:\myapp\server.js"
nssm-rs set node-app AppDirectory "C:\myapp"
nssm-rs set node-app AppEnvironment "NODE_ENV=production"
nssm-rs start node-app
```

### Batch Script
```cmd
# Install a batch script as a service
nssm-rs install batch-service "C:\scripts\monitor.bat"
nssm-rs set batch-service AppStdout "C:\logs\monitor.log"
nssm-rs start batch-service
```

## Differences from Original NSSM

- Written in Rust for better memory safety and performance
- Simplified codebase focused on core functionality
- Modern Windows API usage
- No GUI interface (command-line only)
- Some advanced features may not be implemented yet

## Requirements

- Windows 7 or later
- Administrator privileges for service installation/removal

## Building from Source

Requirements:
- Rust 1.70 or later
- Windows SDK

```bash
git clone <repository>
cd nssm-rs
cargo build --release
```

## License

This project is open source. Please check the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit pull requests or open issues.
