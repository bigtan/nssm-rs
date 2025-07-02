# GitHub Actions 工作流说明

本项目包含以下 GitHub Actions 工作流，用于自动化 CI/CD 流程：

## 工作流文件

### 1. CI 工作流 (`.github/workflows/ci.yml`)

**触发条件：**
- 推送到 `main`、`master` 或 `develop` 分支
- 对这些分支的 Pull Request

**功能：**
- 代码格式检查 (`cargo fmt`)
- 代码质量检查 (`cargo clippy`)
- 运行测试 (`cargo test`)
- 编译 debug 和 release 版本

### 2. Release 工作流 (`.github/workflows/release.yml`)

**触发条件：**
- 推送以 `v` 开头的标签 (如 `v1.0.0`)
- 手动触发 (workflow_dispatch)

**功能：**
- 编译 release 版本
- 运行测试
- 创建发布包 (ZIP 文件)
- 创建 GitHub Release
- 上传发布文件

### 3. 创建标签工作流 (`.github/workflows/create-tag.yml`)

**触发条件：**
- 手动触发 (workflow_dispatch)

**功能：**
- 更新 `Cargo.toml` 中的版本号
- 创建并推送 git 标签
- 自动触发 release 工作流

### 4. 生成变更日志工作流 (`.github/workflows/changelog.yml`)

**触发条件：**
- 推送以 `v` 开头的标签

**功能：**
- 自动生成 CHANGELOG.md
- 基于 conventional commits 格式

## 使用方法

### 发布新版本

1. **方法一：手动创建标签**
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

2. **方法二：使用 GitHub Actions (推荐)**
   - 在 GitHub 仓库页面，点击 "Actions"
   - 选择 "Create Tag and Release" 工作流
   - 点击 "Run workflow"
   - 输入版本号 (如 `1.0.0`)
   - 点击 "Run workflow"

### 版本号规范

- 使用语义化版本控制 (Semantic Versioning)
- 格式：`MAJOR.MINOR.PATCH` (如 `1.0.0`)
- 标签格式：`v1.0.0`

### 提交信息规范

为了自动生成变更日志，建议使用 [Conventional Commits](https://www.conventionalcommits.org/) 格式：

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

**类型 (type):**
- `feat`: 新功能
- `fix`: 修复 bug
- `docs`: 文档更新
- `style`: 代码格式调整
- `refactor`: 代码重构
- `perf`: 性能优化
- `test`: 测试相关
- `chore`: 构建过程或辅助工具的变动

**示例:**
```
feat: 添加服务安装功能
fix: 修复服务启动时的内存泄漏问题
docs: 更新 README 文件
```

## 配置说明

### 必需的仓库设置

1. **Actions 权限**
   - 确保 Actions 有读写权限 (Settings → Actions → General → Workflow permissions)

2. **GITHUB_TOKEN**
   - GitHub 会自动提供，无需手动配置

### 可选配置

1. **保护分支**
   - 建议为 `main` 分支启用保护规则
   - 要求 PR 合并前通过 CI 检查

2. **自动合并**
   - 可以配置 dependabot 自动更新依赖

## 发布文件

每次发布会包含以下文件：

1. `nssm-rs.exe` - 单独的可执行文件
2. `nssm-rs-windows-x64.zip` - 包含可执行文件和文档的完整包

发布的包会包含：
- `nssm-rs.exe` - 主程序
- `README.md` - 项目说明
- `USAGE.md` - 使用说明
- `LOGGING.md` - 日志说明

## 故障排除

### 常见问题

1. **工作流失败**
   - 检查 Actions 页面的错误日志
   - 确认代码通过所有测试
   - 检查 Rust 工具链版本

2. **发布失败**
   - 确认标签格式正确 (`vX.Y.Z`)
   - 检查是否有权限创建 Release

3. **版本更新失败**
   - 确认版本号格式为 `X.Y.Z`
   - 检查 `Cargo.toml` 文件权限

如有问题，请查看 Actions 运行日志或创建 Issue。
