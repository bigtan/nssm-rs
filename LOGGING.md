# NSSM-RS 日志功能说明

## 概述

NSSM-RS 现在包含了完善的日志功能，可以帮助用户了解程序运行状态和调试问题。

## 日志级别

NSSM-RS 支持以下日志级别：

1. **ERROR** - 错误信息（始终显示）
2. **WARN** - 警告信息（默认显示）
3. **INFO** - 信息性消息（使用 `-v` 或 `--verbose` 启用）
4. **DEBUG** - 调试信息（使用 `-d` 或 `--debug` 启用）

## 命令行选项

- `-v, --verbose`: 启用详细输出（INFO 级别）
- `-d, --debug`: 启用调试输出（DEBUG 级别，包含所有级别）

## 环境变量

您也可以通过设置环境变量来控制日志级别：

```powershell
# 设置日志级别为 DEBUG
$env:RUST_LOG = "debug"
nssm-rs.exe list

# 设置日志级别为 INFO
$env:RUST_LOG = "info"
nssm-rs.exe list

# 只显示错误
$env:RUST_LOG = "error"
nssm-rs.exe list
```

## 使用示例

### 基本使用（只显示警告和错误）
```cmd
nssm-rs.exe list
```

### 详细模式（显示信息、警告和错误）
```cmd
nssm-rs.exe -v list
```

### 调试模式（显示所有级别的信息）
```cmd
nssm-rs.exe -d list
```

## 日志输出示例

### 详细模式输出示例
```
[2025-07-02T04:39:06Z INFO  nssm_rs] NSSM-RS starting up...
[2025-07-02T04:39:06Z INFO  nssm_rs] Version: 0.1.0
[2025-07-02T04:39:06Z INFO  nssm_rs] Verbose mode enabled
[2025-07-02T04:39:06Z INFO  nssm_rs] Listing all NSSM-RS managed services
[2025-07-02T04:39:06Z INFO  nssm_rs::service_manager] ServiceManager created successfully
Services managed by nssm-rs:
  Clash
  Clash2
[2025-07-02T04:39:06Z INFO  nssm_rs] Operation completed successfully
[2025-07-02T04:39:06Z INFO  nssm_rs] NSSM-RS shutting down normally
```

### 调试模式输出示例
```
[2025-07-02T04:39:44Z INFO  nssm_rs] NSSM-RS starting up...
[2025-07-02T04:39:44Z INFO  nssm_rs] Version: 0.1.0
[2025-07-02T04:39:44Z DEBUG nssm_rs] Debug mode enabled
[2025-07-02T04:39:44Z DEBUG nssm_rs] Command parsed: Discriminant(9)
[2025-07-02T04:39:44Z INFO  nssm_rs] Listing all NSSM-RS managed services
[2025-07-02T04:39:44Z DEBUG nssm_rs::service_manager] Creating new ServiceManager instance
[2025-07-02T04:39:44Z INFO  nssm_rs::service_manager] ServiceManager created successfully
Services managed by nssm-rs:
  Clash
  Clash2
[2025-07-02T04:39:44Z INFO  nssm_rs] Operation completed successfully
[2025-07-02T04:39:44Z INFO  nssm_rs] NSSM-RS shutting down normally
```

## 日志输出位置

- 日志信息输出到标准输出（stdout）
- 错误信息同时输出到标准错误（stderr）
- 作为 Windows 服务运行时，日志会输出到 Windows 事件日志

## 故障排除

当遇到问题时，建议使用调试模式运行命令：

```cmd
nssm-rs.exe -d install MyService "C:\Path\To\MyApp.exe"
```

这将提供详细的调试信息，帮助诊断问题。
