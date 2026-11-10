# mono-trim 项目技术文档

**mono-trim** 是一款专门为多平台 Monorepo（单一仓库）项目打造的**工作空间拓扑裁剪引擎**。它通过高内聚的本地封闭环境，完美解决了个人开发者或多端团队在克隆项目时“必须拉取异构平台多余代码”的痛点。

---

## 📌 核心设计哲学
1. **高内聚，零污染**：工具本身（三端二进制产物）与项目专属配置文件（`config.toml`）完全内聚在项目根目录的 `.mono-trim/` 隐藏文件夹下，不污染项目根目录。
2. **工具跟随项目**：配置与工具直接提交至 Git 仓库，实现一次配置，跨设备、多端全自动开箱即用。
3. **黑白双煞裁剪**：完美支持黑名单（排除指定目录/单文件）与白名单（仅保留指定目录/单文件）两种拓扑裁剪模式。
4. **命令驱动 + 配套自动化**：日常裁剪行为完全由用户在配置中自定义的子命令驱动，同时内置 `doctor` 自检与 `doc` 文档自动注入功能。

---

## 📁 完美的项目拓扑结构

初始化完成后，您的多平台项目将呈现如下结构：

```text
my-multiplatform-project/
├── .git/
├── .mono-trim/            # 👈 一切关于裁剪能力的黑盒目录
│   ├── config.toml        # 👈 唯一的、高可读性的本地配置文件
│   ├── mtrim.exe          # Windows 平台专用的裁剪二进制文件
│   ├── mtrim-mac          # macOS 平台专用的裁剪二进制文件
│   └── mtrim-linux        # Linux 平台专用的裁剪二进制文件
├── rust/                  # 跨平台通用核心逻辑层 (例如 Rust 交叉编译模块)
└── apps/                  # 异构平台 UI 壳子 (如 apps/android/, apps/ios/)
```

---

## ⚙️ 配置文件规范说明 (`.mono-trim/config.toml`)

`init` 命令会为您生成一份纯净的空白考卷。**每次编辑完该文件后，都建议立刻运行一次 `mtrim doctor` 做自检**，确认无误再执行裁剪命令；`doctor` 会拦截语法错误、非法 `mode`、危险路径与无法被调用的 `profiles` 名称。您可以根据项目结构在其中声明具体的裁剪行为：

```toml
# =====================================================================
#                 mono-trim 工作空间裁剪引擎配置文件
# =====================================================================

# ✅ 重要：每次修改本文件后，请务必先运行一次 `mtrim doctor` 校验配置，
#    确认无 ❌ 错误后再执行自定义裁剪命令，避免非法配置破坏工作空间。

# 1. 自动生成说明文档的目标文件路径
# 说明：必须使用相对路径 "../" 回退到项目根目录。
doc_target = "../README.md"

# 2. 系统保留命令（内置命令，禁止用作 profiles 名称，否则永远不会被执行）
#   init / reapply / doc / doctor / reset / help

# 3. 用户自定义命令与黑白名单配置说明 (profiles)
# 
# 🛠️ 语法结构模板：
#   [profiles.你的自定义子命令名称]
#   mode = "black" 或 "white"
#   desc = "关于这个自定义命令的作用描述（运行 doc 命令时会自动导出到文档中）"
#   targets = [ "需要裁剪的路径A", "需要裁剪的路径B" ]
#
# ⚙️ 参数详解：
#   - mode = "black" ：黑名单模式。默认保留所有内容，仅物理排除 targets 里的项。
#   - mode = "white" ：白名单模式。默认隐藏所有内容，【仅保留】targets 里的项。
#   - targets 路径规则：
#     * 目录：必须以 "/" 结尾（例如 "apps/ios/"）。
#     * 单文件：绝对不能以 "/" 结尾（例如 "assets/large_video.mp4"）。

# 💡 用户配置示例：
[profiles.win-dev]
mode = "black"
desc = "Windows 端 Android 开发环境：隐藏 iOS 原生壳和部分大资产文件"
targets = [
    "apps/ios/",
    "assets/large_video.mp4"
]

[profiles.rust-only]
mode = "white"
desc = "纯底层纯净模式：【仅白名单】检出 Rust 核心层和公共配置，不加载任何 UI 壳"
targets = [
    "rust/",
    "Makefile",
    "Cargo.toml"
]
```

---

## 🚀 命令行工具完整指南

### 1. 内置刚需命令

#### `mtrim init`
在当前项目根目录下初始化工具封闭环境。
* **参数选项**：
  * `-p, --plat <PLATFORMS>...`：指定需要初始化的平台产物（可选值：`win`, `mac`, `linux`）。不传默认全部拷贝。
  * `-e, --exc <EXCLUDES>...`：指定需要排除不拷贝的平台产物。
* **调用示例**：
  ```bash
  # 默认全部拷贝
  mtrim init
  # 仅拷贝并配置 Windows 和 Linux 端二进制文件
  mtrim init -p win linux
  # 拷贝全部，但排除 mac 端
  mtrim init --exc mac
  ```
* **控制台反馈**：执行成功后将输出：
  ```text
  ✨ 初始化成功！配置文件已生成，具体配置方法请直接查看该文件：
  👉 ./.mono-trim/config.toml
  ⚠️  修改配置后请务必运行一次 `mtrim doctor` 校验，再执行自定义裁剪命令。
  ```

#### `mtrim reset`
一键清理当前所有的稀疏裁剪规则，恢复项目 100% 全量的目录与物理文件。

#### `mtrim doctor`
对当前环境进行健康检查。它会校验 `config.toml` 的语法合法性，逐行比对 `targets` 中的物理路径在当前硬盘中是否存在，精准抛出拼写警告；同时校验 `profiles` 名称是否能真正作为子命令被调用——与内置命令（`init` / `reapply` / `doc` / `doctor` / `reset` / `help`）重名、含空白字符、以 `-` 开头或为空的名称都会直接报错，避免配置写完却永远无法触发。

#### `mtrim doc`
文档自动化引擎。自动扫描 `config.toml` 中用户编写的所有自定义命令和 `desc` 描述，在 `doc_target` 指定的文件（如根目录 `README.md`）末尾自动喷出一段完美的 Markdown 格式化命令速查表格。

#### `mtrim reapply`
根据当前本地的 Git 稀疏规则设置，强行重新刷新并同步本地物理硬盘上的文件状态。

---

### 2. 动态自定义命令 (日常盲跑)

当用户在 `config.toml` 中配置了特定的 `profiles` 节点后（例如 `win-dev`），该节点名称将直接被 CLI 转化为对应的子命令。

* **黑名单模式执行逻辑**：
  自动向 Git 稀疏配置文件中写入 `/*` 允许全部通过，随后对 `targets` 中配置的目录或单文件追加感叹号 `!/path` 进行物理挖除。
* **白名单模式执行逻辑**：
  自动向 Git 稀疏配置文件中写入 `/*` 允许根目录基础文件通过（确保 `Makefile`、`Cargo.toml` 等关键引导文件不丢失），并对非白名单目录进行精准封锁，仅向物理硬盘释放白名单内的资产。
* **调用示例**：
  ```bash
  # 执行您自定义的 Windows 开发环境裁剪
  .\.mono-trim\mtrim win-dev
  
  # 执行您自定义的纯 Rust 底层白名单保留
  .\.mono-trim\mtrim rust-only
  ```

---

## 🛠️ 多平台交叉编译部署指南

作为全局发布或团队内部分发前，推荐在开发机上通过以下命令一键交叉编译出三端产物，并统一放置在您的全局 `PATH` 路径或归档目录下：

```bash
# 1. 编译 Windows 端
cargo build --release --target x86_64-pc-windows-msvc
mv target/x86_64-pc-windows-msvc/release/mtrim.exe ./mtrim-win.exe

# 2. 编译 macOS 端
cargo build --release --target x86_64-apple-darwin
mv target/x86_64-apple-darwin/release/mtrim ./mtrim-mac

# 3. 编译 Linux 端
cargo build --release --target x86_64-unknown-linux-gnu
mv target/x86_64-unknown-linux-gnu/release/mtrim ./mtrim-linux
```
当全局工具运行 `init` 时，它会智能感知自身所在的二进制目录，并根据用户的包含/排除参数，将对应的 `mtrim-mac` 或 `mtrim.exe` 优雅地克隆进目标项目的 `.mono-trim/` 隐藏资产库中。
