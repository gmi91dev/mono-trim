# mono-trim

**按平台裁剪工作空间：只保留你要的那部分代码。**

[![CI](https://github.com/gmi91dev/mono-trim/actions/workflows/ci.yml/badge.svg)](https://github.com/gmi91dev/mono-trim/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/gmi91dev/mono-trim)](https://github.com/gmi91dev/mono-trim/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`mtrim` 面向多平台 Monorepo：用 Git 稀疏检出（sparse-checkout）把不属于当前平台的文件**从工作区物理移除**，只留下你要的部分。裁剪规则写在仓库内的配置文件里，随 Git 同步；工具本身装在开发者机器上，项目里不留任何二进制。

```text
apps/ios/  apps/android/  rust/  assets/
     └── 在 Windows 上跑 mtrim win-dev → 只剩 apps/android/ + rust/ + 根目录配置
```

## 特性

- **黑白双煞**：`black` 默认保留全部、只排除指定项；`white` 默认隐藏全部、只保留指定项。
- **配置随仓库走**：系统配置（工具维护的版本与默认值）+ 用户配置（你自己的 profiles）分离，字段只增不删，升级无需迁移。
- **本机驱动**：命令由本机 `mtrim` 执行，项目里不提交二进制，不会出现"三端产物版本不一致"。
- **可自更新**：`mtrim update` 校验 sha256 后升级自身，并同步项目配置版本。
- **零依赖运行**：单文件二进制，Windows / macOS / Linux 均可。

## 安装

安装只需要一行命令：Windows 用 PowerShell，macOS / Linux 用 sh。脚本会校验 `sha256`、安装到用户目录并提示 PATH。

### Windows（PowerShell 5.1 / 7）

默认安装最新版：

```powershell
irm https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/install.ps1 | iex
```

指定版本或目录（`iex` 无法传参，改用脚本块）：

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/install.ps1))) -Version 0.0.6 -Dir D:\tools\mono-trim
```

### macOS / Linux

默认安装最新版：

```bash
curl -fsSL https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/install.sh | bash
```

指定版本：

```bash
curl -fsSL https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/install.sh | bash -s -- --version 0.0.6
```

### 安装脚本参数

| 参数 | 说明 |
| :--- | :--- |
| `--version` / `-Version` | 指定版本，默认 `latest` |
| `--dir` / `-Dir` | 安装目录（默认 `~/.local/bin` 或 `%LOCALAPPDATA%\Programs\mono-trim`） |
| `--force` / `-Force` | 同版本也重新安装 |
| `--dry-run` / `-DryRun` | 只打印将要执行的动作 |

环境变量 `MONO_TRIM_MIRROR` 可替换下载源（内网镜像）。

### 网络受限 / 想先审查脚本

```powershell
# PowerShell：先下载再执行
curl.exe -fsSL -o "$env:TEMP\mtrim-install.ps1" https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/install.ps1
& "$env:TEMP\mtrim-install.ps1"
```

```bash
# macOS / Linux：clone 后运行仓库内脚本
git clone https://github.com/gmi91dev/mono-trim.git
./mono-trim/scripts/install.sh --version 0.0.6
```

### 从源码

```bash
cargo install --path .
# 或
cargo install --git https://github.com/gmi91dev/mono-trim.git
```

### 卸载

```powershell
# Windows：删除程序，并把安装目录从用户 PATH 移除
irm https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/uninstall.ps1 | iex
```

```bash
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/uninstall.sh | bash
```

手动方式：

- Windows：删除 `%LOCALAPPDATA%\Programs\mono-trim`，再从用户 PATH 中移除该目录
- macOS / Linux：删除 `~/.local/bin/mtrim`，并删掉 shell 配置里对应的 PATH 行

卸载**不会**动项目里的 `.mono-trim/config.toml` 与 `mono-trim.toml`（它们是项目数据）。

## 快速开始

```bash
cd <你的仓库>

mtrim init          # 生成 .mono-trim/config.toml 与 mono-trim.toml
$EDITOR mono-trim.toml

mtrim doctor        # 校验两份配置是否合法
mtrim ls            # 查看可用命令
mtrim win-dev       # 执行自定义裁剪
mtrim applyall      # 恢复全量工作空间
```

`mono-trim.toml` 示例：

```toml
[profiles.win-dev]
mode = "black"
desc = "Windows 端开发：隐藏 iOS 壳与大资产"
targets = ["apps/ios/", "assets/large_video.mp4"]

[profiles.rust-only]
mode = "white"
desc = "只保留 Rust 核心层"
targets = ["rust/", "Makefile", "Cargo.toml"]
```

## 命令

| 命令 | 作用 |
| :--- | :--- |
| `mtrim init` | 用内置默认值初始化项目配置（系统配置会重写，用户配置只在缺失时创建） |
| `mtrim clone <仓库> [目录] [-c <命令>]` | 克隆仓库；识别其中的 mono-trim 配置，可立即执行某条命令 |
| `mtrim update` | 升级系统 CLI；并把项目配置版本同步到当前 CLI（无变化不写文件） |
| `mtrim doctor` | 校验系统配置与用户配置的合法性（含是否被 `.gitignore` 忽略） |
| `mtrim ls` | 列出用户自定义命令；`ls -a` 同时列出内置命令 |
| `mtrim show` | 打印两份配置文件的位置与是否存在 |
| `mtrim doc` | 把命令速查表写入 `doc_target`（可重复执行，原地更新） |
| `mtrim applyall` | 恢复项目为全量工作空间（不需要配置文件） |
| `mtrim <名称>` | 执行用户配置中的 profile |

## 克隆后直接裁剪（可选）

```bash
# 标准克隆；若仓库已用 mono-trim 管理，会列出可用命令
mtrim clone https://github.com/<owner>/<repo>.git

# 克隆完成后立即执行配置里的某条命令（-c / --command）
mtrim clone https://github.com/<owner>/<repo>.git -c win-dev

# 指定目录
mtrim clone <url> work/mono -c win-dev
```

- 仓库**没有**使用 mono-trim 时，会提示 `该仓库尚未使用 mono-trim 管理`，并建议进入目录执行 `mtrim init`；
- `-c` 的取值是**配置文件中 `[profiles.*]` 的命令名**，不是配置文件路径；命令不存在会报 `配置中没有 '<名称>' 命令`；
- 默认目录名取仓库名，目标目录已存在且非空会拒绝。

## 配置

| 文件 | 谁维护 | 内容 |
| :--- | :--- | :--- |
| `.mono-trim/config.toml` | 工具（`init` / `update` 重写） | `version`、默认值（如 `doc_target`） |
| `mono-trim.toml` | 你 | `profiles`、需要覆盖的字段 |

运行时按 `内置默认值 ← 系统配置 ← 用户配置` 合并，用户字段优先。`/`、`.git/`、`.mono-trim/` 永远不可裁剪。

## 升级

```bash
mtrim update        # 有新版本则升级 CLI，并同步项目配置版本
mtrim --version     # 查看当前版本
```

配置字段只增不删，所以从旧版本升级到新版本不需要迁移脚本，也不会动你的用户配置。

## 常见问题

- **Windows 首次运行提示 SmartScreen / macOS 提示来源未知**：二进制未做代码签名，选择"仍要运行"，或在 macOS 上执行 `xattr -d com.apple.quarantine $(which mtrim)`。
- **公司内网无法访问 GitHub**：设置 `MONO_TRIM_MIRROR`（安装脚本与 `mtrim update` 都支持），指向内部静态目录（结构与本仓库 Release 一致）。
- **路径规则**：目录以 `/` 结尾（`apps/ios/`），单文件不要以 `/` 结尾；不支持绝对路径与 `..`。
- **配置文件被 `.gitignore` 忽略**：`mtrim init` 会提示、`mtrim doctor` 会报错。修复方式是在 `.gitignore` 中加例外：
  ```gitignore
  !.mono-trim/
  !mono-trim.toml
  ```
  或临时 `git add -f .mono-trim/config.toml mono-trim.toml`。
- **为什么要两份配置**：工具需要记录的版本与默认值和你的业务配置分开，`update` 重写系统配置时不会碰你的内容。

更详细的规范见 [doc/mono-trim-spec.md](doc/mono-trim-spec.md)。

## 开发

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

测试分四层：单元（`src/**`）、CLI 契约（`tests/cli.rs`）、真实 Git 端到端（`tests/git_sparse.rs`）、文本快照（`tests/golden.rs`），另有 `tests/update.rs` 用本地 HTTP 服务器验证自更新。

更新快照：`INSTA_UPDATE=always cargo test --test golden`。

发布：修改 `Cargo.toml` 版本 → `git tag v0.0.6 && git push origin v0.0.6`，GitHub Actions 会构建四平台产物并生成 `sha256sums.txt`。

## License

[MIT](LICENSE)
