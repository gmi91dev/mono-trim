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

    let git = git::SystemGit;
    git::ensure_repo(&git)?;
    let config_path = config::path(&git);

    match matches.subcommand() {
        Some(("reapply", _)) => apply::reapply(&git),
        Some(("doc", _)) => doc::run(&config_path),
        Some(("doctor", _)) => doctor::run(&config_path),
        Some(("reset", _)) => apply::reset(&git),
        Some((name, _)) => apply::profile(&config_path, name, &git),
        None => Err("未指定具体操作！输入 --help 查看可用命令。".into()),
    }
}
