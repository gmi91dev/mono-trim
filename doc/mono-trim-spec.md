# mono-trim 项目技术文档

**mono-trim** 是一款面向多平台 Monorepo 的**工作空间拓扑裁剪引擎**。它用 Git 稀疏检出（sparse-checkout）把不属于当前平台的文件从工作区物理移除，只保留你要的那部分；裁剪规则写在**项目内的配置**里，随仓库同步，而工具本身只装在开发者的机器上。

---

## 📌 设计原则

1. **本机驱动**：项目里不存放任何二进制，命令由本机安装的 `mtrim` 执行。
2. **配置即项目状态**：Git 里只有两份配置文件，克隆下来就知道该怎么裁剪。
3. **系统配置 / 用户配置分离**：工具维护的默认值与版本放在系统配置，用户只在自己的配置里写覆盖项与自定义命令。
4. **只增不删**：配置字段只新增、不删除不重命名，旧配置永远可用，升级不需要迁移脚本。
5. **黑白双煞**：`black` 默认保留全部、只排除指定项；`white` 默认隐藏全部、只保留指定项。

---

## 📁 项目拓扑

```text
my-multiplatform-project/
├── .git/
├── .mono-trim/
│   └── config.toml      # 系统配置（工具生成，随仓库提交）
├── mono-trim.toml       # 用户配置（profiles 写在这里，随仓库提交）
├── rust/                # 跨平台核心逻辑
└── apps/                # 异构平台壳子（apps/android/、apps/ios/）
```

`.mono-trim/config.toml` 由工具维护，`mono-trim.toml` 由你维护；除此之外项目里不会再产生任何文件。

---

## ⚙️ 配置文件规范

### 1. 系统配置 `.mono-trim/config.toml`

由 `mtrim init` / `mtrim update` 用**内置默认值整体重写**，请勿手工修改：

```toml
# 系统配置：由 mono-trim 生成，请勿手工修改（mtrim init / update 会整体重写本文件）
version = "0.1.0"
doc_target = "README.md"
```

| 字段 | 说明 |
| :--- | :--- |
| `version` | 生成该配置的工具版本，用于提示"项目配置落后/过高" |
| `doc_target` | `doc` 命令的默认输出位置（相对项目根目录），可被用户配置覆盖 |

### 2. 用户配置 `mono-trim.toml`

只写你想覆盖或新增的内容，工具**永不改写**此文件：

```toml
# 覆盖系统默认的文档输出位置
doc_target = "docs/commands.md"

[profiles.win-dev]
mode = "black"
desc = "Windows 端开发：隐藏 iOS 壳与大资产"
targets = ["apps/ios/", "assets/large_video.mp4"]
```

| 字段 | 说明 |
| :--- | :--- |
| `doc_target` | 覆盖系统默认的文档输出位置，相对项目根目录 |
| `[profiles.<名称>]` | 自定义子命令，之后可用 `mtrim <名称>` 调用 |
| `mode` | `black`：保留全部，仅排除 `targets`；`white`：隐藏全部，仅保留 `targets` |
| `desc` | 命令说明，`mtrim ls` 与 `mtrim doc` 会展示 |
| `targets` | 相对项目根目录的路径；目录以 `/` 结尾（`apps/ios/`），单文件不要以 `/` 结尾 |

### 3. 合并规则

```text
内置默认值  ←  .mono-trim/config.toml  ←  mono-trim.toml
```

- 字段级覆盖：用户配置有值就用用户的，没有就用系统配置的，再没有就用内置默认值。
- `profiles` 按命令名合并，同名时用户配置胜出。
- `version` 只认系统配置。

### 4. 保护规则

`/`（项目根）、`.git/`、`.mono-trim/` 永远不可被裁剪；`profiles` 名称不能与内置命令同名，也不能含空白字符。

---

## 🚀 命令指南

| 命令 | 作用 |
| :--- | :--- |
| `mtrim init` | 用内置默认值重新初始化系统配置；用户配置不存在时创建，存在则不动 |
| `mtrim clone <仓库> [目录] [-c <命令>]` | 克隆仓库；识别 mono-trim 配置，可立即执行某条命令 |
| `mtrim update` | 升级系统 CLI；把项目记录的 `version` 同步到当前 CLI 版本（无变化则不写） |
| `mtrim doctor` | 校验系统配置与用户配置的合法性（语法、字段、profile 命名、目标路径、是否被 `.gitignore` 忽略） |
| `mtrim ls` | 列出用户自定义命令；`ls -a` 同时列出内置命令 |
| `mtrim show` | 打印两份配置文件的位置与是否存在 |
| `mtrim doc` | 把命令速查表写入 `doc_target`（可重复执行，原地更新） |
| `mtrim applyall` | 恢复项目为全量工作空间（不需要配置文件） |
| `mtrim <名称>` | 执行用户配置中的 profile |

### `mtrim clone`

```bash
mtrim clone <仓库> [目录] [-c <命令>]
```

1. 执行标准 `git clone`（工作空间为全量）；
2. 如果仓库里有 mono-trim 配置：打印可用命令；带 `-c/--command` 时立即在该目录执行这条命令（按 profile 裁剪）；
3. 如果没有配置：提示 `该仓库尚未使用 mono-trim 管理`，并建议运行 `mtrim init`；此时带 `-c` 会直接报错退出。

`-c` 的取值是 `[profiles.*]` 下的**命令名**，不是配置文件路径；命令不存在时提示 `配置中没有 '<名称>' 命令`。
（旧参数 `-p/--profile` 作为隐藏别名保留兼容。）

### 典型输出

```text
$ mtrim ls
rust-only  用户 white  仅保留 Rust 核心层
win-dev    用户 black  隐藏 iOS 壳与大资产

$ mtrim show
系统配置  ./.mono-trim/config.toml  （存在）
用户配置  ./mono-trim.toml          （存在）
```

---

## 🔄 版本与升级

- `mtrim --version` 输出当前 CLI 版本。
- 系统配置里的 `version` 记录"项目由哪个版本的工具维护"。
- 各命令发现版本不一致时只给提示，不阻断：
  - 项目版本更高 → `建议升级系统 CLI`
  - 项目版本更低 → `可运行 mtrim update 同步`
- `mtrim update` 的行为：
  1. 先下载几百字节的 `version.txt` 查询最新版本；**只有确实存在新版本时**才下载二进制，
     并用 `sha256sums.txt` 校验后再替换自身（校验失败不会改动现有文件）；
  2. 若当前目录已初始化且项目版本落后，用内置默认值重写系统配置并写入新版本；
  3. 两者都不需要 → 输出"已是最新"，不写任何文件。
- 配置字段只增不删，所以从旧版本升级到新版本**不需要迁移、备份或重置用户配置**。
- 内网/镜像环境：安装脚本与 `mtrim update` 都支持 `MONO_TRIM_MIRROR` 指定下载源。

---

## 📦 安装

安装只需要一行命令：Windows 用 PowerShell，macOS / Linux 用 sh。脚本会从 GitHub Release 下载当前平台产物、校验 sha256、安装到用户目录并提示 PATH。

### Windows（PowerShell 5.1 / 7）

```powershell
irm https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/install.ps1 | iex

# 指定版本 / 目录（iex 无法传参，用脚本块）
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/install.ps1))) -Version 0.0.9 -Dir D:\tools\mono-trim
```

### macOS / Linux

默认安装最新版：

```bash
curl -fsSL https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/install.sh | bash
```

指定版本：

```bash
curl -fsSL https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/install.sh | bash -s -- --version 0.0.9
```

### 参数

| 参数 | 说明 |
| :--- | :--- |
| `--version` / `-Version` | 指定版本，默认 `latest` |
| `--dir` / `-Dir` | 安装目录（默认 `~/.local/bin` 或 `%LOCALAPPDATA%\Programs\mono-trim`） |
| `--force` / `-Force` | 同版本也重新安装 |
| `--dry-run` / `-DryRun` | 只打印动作，不写文件 |

环境变量 `MONO_TRIM_MIRROR` 可替换下载源（内网镜像，安装脚本与 `mtrim update` 都支持）。

### 网络受限 / 先审查脚本

```powershell
# Windows：先下载再执行
curl.exe -fsSL -o "$env:TEMP\mtrim-install.ps1" https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/install.ps1
& "$env:TEMP\mtrim-install.ps1"
```

```bash
# macOS / Linux：clone 后运行仓库内脚本
git clone https://github.com/gmi91dev/mono-trim.git
./mono-trim/scripts/install.sh --version 0.0.9
```

### 从源码

```bash
cargo install --path .
cargo install --git https://github.com/gmi91dev/mono-trim.git
```

### 卸载

```powershell
# Windows
irm https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/uninstall.ps1 | iex
```

```bash
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/gmi91dev/mono-trim/main/scripts/uninstall.sh | bash
```

脚本会删除可执行文件并（Windows 上）把安装目录从用户 PATH 移除；目录为空时一并删除。
项目配置文件 `.mono-trim/config.toml`、`mono-trim.toml` 不会被删除。

### 新设备上手

```bash
git clone <repo> && cd <repo>
mtrim doctor
mtrim ls
mtrim win-dev
```

### 升级

- 系统 CLI：`mtrim update`（下载 `sha256sums.txt` 校验后替换自身），或重跑安装脚本；
- 项目配置版本：`mtrim update` 会同步 `.mono-trim/config.toml`（无变化不写文件）。

### 发布（维护者）

1. 修改 `Cargo.toml` 的 `version`；
2. `git tag v0.0.9 && git push origin v0.0.9`；
3. `.github/workflows/release.yml` 构建四个平台产物，并附 `sha256sums.txt` 与 `version.txt`（供 `update` 免下载查询版本）。

## ⏱️ 进度与耗时

- `clone`、`mtrim <命令>`、`applyall`：直接继承终端输出，git 的 `Receiving objects` / `Updating files` 进度条可见，并在结束时打印耗时；
- `update`：显示当前版本、下载产物名、**自绘 ASCII 进度条**（`[#####-----] 45% 700 KiB/1.5 MiB`）、sha256 校验与总耗时；无 `Content-Length` 时退化为"已下载量"；
- `doctor`：在交互式终端下逐项显示检查步骤与总耗时；
- 输出被重定向（CI、管道）时这些进度行自动静默，保持日志干净。

## 🧪 开发与测试

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

测试分为四层：

| 层 | 位置 | 覆盖内容 |
| :--- | :--- | :--- |
| 单元测试 | `src/**` | 配置合并、规则生成、路径校验、版本比较、doctor 结论 |
| 契约测试 | `tests/cli.rs` | 帮助、版本、退出码、各命令端到端行为 |
| 端到端测试 | `tests/git_sparse.rs` | 真实 Git 下黑白名单与 `applyall` 的文件结果 |
| 自更新测试 | `tests/update.rs` | 本地 HTTP 服务器模拟 Release：下载、sha256 校验、校验失败与下载失败 |
| 快照测试 | `tests/golden.rs` | 帮助输出、生成的配置、`doc` 表格、`ls` 输出 |

更新快照：`INSTA_UPDATE=always cargo test --test golden`。
