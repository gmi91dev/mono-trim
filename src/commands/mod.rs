//! 子命令实现与统一调度。

pub mod apply;
pub mod clone;
pub mod doc;
pub mod doctor;
pub mod init;
pub mod ls;
pub mod show;
pub mod update;

use clap::ArgMatches;

use crate::Result;
use crate::config::Project;
use crate::git::SystemGit;
use crate::update::{GithubFetcher, SelfReplacer};

pub fn dispatch(matches: ArgMatches) -> Result<()> {
    let project = Project::detect(&SystemGit);

    match matches.subcommand() {
        Some(("init", _)) => init::run(&project),
        Some(("clone", sub)) => clone::run(
            sub.get_one::<String>("repository")
                .map(String::as_str)
                .unwrap_or_default(),
            sub.get_one::<String>("directory").map(String::as_str),
            sub.get_one::<String>("command").map(String::as_str),
        ),
        Some(("update", sub)) => update::run(
            &project,
            &GithubFetcher,
            &SelfReplacer,
            sub.get_one::<String>("version").map(String::as_str),
        ),
        Some(("doctor", _)) => doctor::run(&project),
        Some(("ls", sub_matches)) => ls::run(&project, sub_matches.get_flag("all")),
        Some(("show", _)) => show::run(&project),
        Some(("doc", _)) => doc::run(&project),
        Some(("applyall", _)) => apply::applyall(&SystemGit),
        Some((name, _)) => apply::profile(&project, name, &SystemGit),
        None => Err("未指定具体操作！输入 --help 查看可用命令。".into()),
    }
}
