//! mono-trim：高内聚、零污染的项目级多平台工作空间裁剪引擎。
//!
//! 模块职责划分：
//! - [`cli`]        命令与参数定义
//! - [`commands`]   各子命令的实现与调度
//! - [`config`]     `config.toml` 的数据结构与读取
//! - [`git`]        Git 调用、仓库与稀疏检出配置定位
//! - [`sparse`]     稀疏规则的生成与目标路径校验
//! - [`util`]       文件读写等通用工具

mod cli;
mod commands;
mod config;
mod git;
mod sparse;
mod util;

/// 全项目统一错误类型：错误信息即面向用户的最终提示。
pub type Result<T> = std::result::Result<T, String>;

fn main() {
    if let Err(message) = commands::dispatch(cli::build().get_matches()) {
        eprintln!("❌ {message}");
        std::process::exit(1);
    }
}
