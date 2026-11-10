//! 子命令实现与统一调度。

pub mod apply;
pub mod doc;
pub mod doctor;
pub mod init;

use clap::ArgMatches;

use crate::{Result, config, git};

pub fn dispatch(matches: ArgMatches) -> Result<()> {
    // init 不需要已有的 Git 仓库与配置文件
    if let Some(("init", sub_matches)) = matches.subcommand() {
        return init::run(sub_matches);
    }

    git::ensure_repo()?;
    let config_path = config::path();

    match matches.subcommand() {
        Some(("reapply", _)) => apply::reapply(),
        Some(("doc", _)) => doc::run(&config_path),
        Some(("doctor", _)) => doctor::run(&config_path),
        Some(("reset", _)) => apply::reset(),
        Some((name, _)) => apply::profile(&config_path, name),
        None => Err("未指定具体操作！输入 --help 查看可用命令。".into()),
    }
}
