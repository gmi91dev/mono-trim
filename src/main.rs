//! Mono-Trim：高内聚、零污染的项目级多平台工作空间裁剪引擎。
//!
//! 模块职责划分：
//! - [`cli`]        命令与参数定义
//! - [`commands`]   各子命令的实现与调度
//! - [`config`]     系统配置 / 用户配置与合并
//! - [`git`]        Git 调用、仓库与稀疏检出配置定位
//! - [`sparse`]     稀疏规则的生成与目标路径校验
//! - [`update`]     系统 CLI 自更新的下载与替换
//! - [`util`]       文件与版本号等通用工具
//! - [`version`]    版本号解析与比较

mod cli;
mod commands;
mod config;
mod git;
mod sparse;
#[cfg(test)]
mod test_support;
mod update;
mod util;
mod version;

/// 全项目统一错误类型：错误信息即面向用户的最终提示。
pub type Result<T> = std::result::Result<T, String>;

fn main() {
    if let Err(message) = commands::dispatch(cli::build().get_matches()) {
        eprintln!("❌ {message}");
        std::process::exit(1);
    }
}
