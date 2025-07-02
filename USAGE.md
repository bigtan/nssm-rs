# NSSM-RS 使用示例

## 基本用法

### 1. 安装服务

```powershell
# 安装一个简单的测试服务
nssm-rs install TestService "D:\WorkPlace\Rust\nssm-rs\examples\test-app\target\release\test-app.exe"

# 安装带参数的服务
nssm-rs install TestServiceError "D:\WorkPlace\Rust\nssm-rs\examples\test-app\target\release\test-app.exe" error

# 安装 Python 脚本服务
nssm-rs install PythonTestService "python" "D:\WorkPlace\Rust\nssm-rs\examples\test_service.py"
```

### 2. 配置服务

```powershell
# 设置服务显示名称
nssm-rs set TestService DisplayName "Test Application Service"

# 设置服务描述
nssm-rs set TestService Description "A test service managed by nssm-rs"

# 设置启动类型
nssm-rs set TestService Start SERVICE_AUTO_START

# 设置工作目录
nssm-rs set TestService AppDirectory "D:\WorkPlace\Rust\nssm-rs\examples\test-app"

# 设置输出重定向
nssm-rs set TestService AppStdout "D:\Logs\TestService_stdout.log"
nssm-rs set TestService AppStderr "D:\Logs\TestService_stderr.log"

# 设置进程优先级
nssm-rs set TestService AppPriority HIGH_PRIORITY_CLASS

# 设置重启延迟
nssm-rs set TestService AppRestartDelay 5000

# 设置节流时间
nssm-rs set TestService AppThrottle 2000

# 设置退出动作
nssm-rs set TestService AppExitAction Restart
```

### 3. 服务管理

```powershell
# 启动服务
nssm-rs start TestService

# 停止服务
nssm-rs stop TestService

# 重启服务
nssm-rs restart TestService

# 查询服务状态
nssm-rs status TestService

# 列出所有由 nssm-rs 管理的服务
nssm-rs list
```

### 4. 查看配置

```powershell
# 查看所有参数
nssm-rs get TestService Application
nssm-rs get TestService AppDirectory
nssm-rs get TestService AppParameters
nssm-rs get TestService DisplayName
nssm-rs get TestService Description
nssm-rs get TestService Start
nssm-rs get TestService AppPriority
nssm-rs get TestService AppStdout
nssm-rs get TestService AppStderr
nssm-rs get TestService AppThrottle
nssm-rs get TestService AppRestartDelay
nssm-rs get TestService AppExitAction
```

### 5. 重置配置

```powershell
# 重置单个参数到默认值
nssm-rs reset TestService AppThrottle
nssm-rs reset TestService AppPriority
```

### 6. 删除服务

```powershell
# 删除服务（会要求确认）
nssm-rs remove TestService

# 强制删除服务（跳过确认）
nssm-rs remove TestService --confirm
```

## 高级配置

### 停止方法配置

```powershell
# 配置停止方法：0=所有方法，1=跳过Ctrl+C，2=跳过WM_CLOSE，4=跳过线程终止，8=跳过进程终止
nssm-rs set TestService AppStopMethod 0

# 配置各个停止方法的超时时间（毫秒）
nssm-rs set TestService AppStopMethod_Console 3000  # Ctrl+C 超时
nssm-rs set TestService AppStopMethod_Window 3000   # WM_CLOSE 超时  
nssm-rs set TestService AppStopMethod_Threads 3000  # 线程终止超时
```

### 控制台设置

```powershell
# 禁用控制台（对 GUI 应用有用）
nssm-rs set TestService AppNoConsole 1
```

## 测试场景

### 场景1：测试正常服务

```powershell
# 安装并启动一个正常的测试服务
nssm-rs install NormalTest "D:\WorkPlace\Rust\nssm-rs\examples\test-app\target\release\test-app.exe"
nssm-rs set NormalTest DisplayName "Normal Test Service"
nssm-rs set NormalTest AppStdout "D:\Logs\normal_test.log"
nssm-rs start NormalTest
nssm-rs status NormalTest
```

### 场景2：测试错误退出和重启

```powershell
# 安装一个会出错的服务来测试重启功能
nssm-rs install ErrorTest "D:\WorkPlace\Rust\nssm-rs\examples\test-app\target\release\test-app.exe" error
nssm-rs set ErrorTest AppExitAction Restart
nssm-rs set ErrorTest AppRestartDelay 2000
nssm-rs set ErrorTest AppStdout "D:\Logs\error_test.log"
nssm-rs start ErrorTest
# 观察日志，应该看到服务会在错误后重启
```

### 场景3：测试节流机制

```powershell
# 安装一个快速退出的服务来测试节流
nssm-rs install ThrottleTest "D:\WorkPlace\Rust\nssm-rs\examples\test-app\target\release\test-app.exe" quick
nssm-rs set ThrottleTest AppThrottle 5000  # 5秒节流时间
nssm-rs set ThrottleTest AppStdout "D:\Logs\throttle_test.log"
nssm-rs start ThrottleTest
# 观察日志，应该看到快速重启之间有延迟
```

## 故障排除

### 1. 权限问题
- 确保以管理员身份运行 nssm-rs
- 检查服务账户权限

### 2. 路径问题
- 使用绝对路径
- 确保路径中的文件存在
- 检查工作目录设置

### 3. 日志检查
- 查看 Windows 事件查看器中的服务日志
- 检查设置的 stdout/stderr 重定向文件
- 使用 `nssm-rs status` 查看服务状态

### 4. 测试应用
- 先在命令行下测试应用程序是否正常运行
- 确认应用程序能正确处理信号（Ctrl+C等）

## 与原版 NSSM 的兼容性

nssm-rs 保持了与原版 NSSM 的主要接口兼容性：

- 支持相同的命令行参数格式
- 支持相同的注册表配置结构
- 支持相同的服务管理操作
- 支持相同的参数名称和值

主要差异：
- 基于 Rust 实现，性能更好
- 错误处理更清晰
- 代码更易维护和扩展
