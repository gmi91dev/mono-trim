//! `config.toml` 的数据结构、模板与读取。

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::cli::COMMAND;
use crate::git::project_root;

/// 工具自身与配置所在的封闭目录（严禁被裁剪）
pub const TOOL_DIR: &str = ".mono-trim";
pub const CONFIG_NAME: &str = "config.toml";
pub const DEFAULT_DOC_TARGET: &str = "../README.md";

pub const TEMPLATE: &str = r#"# =====================================================================
#                 mono-trim 工作空间裁剪引擎配置文件
# =====================================================================

# ✅ 重要：每次修改本文件后，请务必先运行一次 `{command} doctor` 校验配置，
#    确认无❌错误后再执行自定义裁剪命令，避免非法配置破坏工作空间。

doc_target = "../README.md"

# ⛔ 系统保留命令（内置命令，禁止用作 profiles 名称，否则永远无法触发）：
#   {reserved}

# 🛠️ 语法结构模板：
#   [profiles.你的自定义子命令名称]
#   mode = "black" 或 "white"
#   desc = "关于这个自定义命令的作用描述"
#   targets = [ "apps/ios/", "assets/large_video.mp4" ]

# ⚠️ 保护规则：`/`（项目根）、`.git/`、`.mono-trim/` 永远不可被裁剪。
"#;

/// 初始化时写入的完整配置模板（保留命令列表由 `cli` 提供，避免两处漂移）
pub fn template() -> String {
    let reserved = crate::cli::reserved_commands()
        .collect::<Vec<_>>()
        .join(" / ");
    TEMPLATE
        .replace("{reserved}", &reserved)
        .replace("{command}", COMMAND)
}

#[derive(Deserialize, Debug, Default)]
pub struct Config {
    #[serde(default)]
    pub doc_target: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

#[derive(Deserialize, Debug)]
pub struct Profile {
    pub mode: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub targets: Vec<String>,
}

/// 配置文件路径：以 Git 顶层目录为基准，支持在子目录中调用
pub fn path() -> PathBuf {
    project_root().join(TOOL_DIR).join(CONFIG_NAME)
}

pub fn read_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path).map_err(|e| {
        format!(
            "未发现或无法读取配置文件 ./{TOOL_DIR}/{CONFIG_NAME}（{e}），请先运行 {COMMAND} init。"
        )
    })?;
    toml::from_str(&content)
        .map_err(|e| format!("配置文件 ./{TOOL_DIR}/{CONFIG_NAME} 语法错误：{e}"))
}

/// 配置中的 profile 名称按字典序排列，保证输出稳定
pub fn sorted_profiles(config: &Config) -> Vec<&String> {
    let mut names: Vec<&String> = config.profiles.keys().collect();
    names.sort();
    names
}
