//! `update`：升级系统 CLI，并把项目记录的版本同步到当前 CLI 版本。
//!
//! - 没有新版 CLI、项目版本也已同步时，不做任何写入
//! - 项目系统配置由内置默认值整体重写，用户配置永不改动

use crate::Result;
use crate::config::{Project, current_version};
use crate::update::{Fetcher, Replacer};
use crate::util::{is_interactive, step};
use crate::version;
use std::fs;

pub fn run(project: &Project, fetcher: &dyn Fetcher, replacer: &dyn Replacer) -> Result<()> {
    let started = std::time::Instant::now();
    let mut effective = current_version().to_string();
    let mut acted = false;
    let mut cli_checked = false;

    // 1) 系统 CLI
    println!("⏳ 正在检查系统 CLI 更新（当前 {}）...", current_version());
    match fetcher.fetch_latest() {
        Ok(fetched) if version::is_newer(&fetched.version, current_version()) => {
            cli_checked = true;
            replacer.replace(&fetched.binary)?;
            println!(
                "✅ 已升级系统 CLI：{} → {}",
                current_version(),
                fetched.version
            );
            effective = fetched.version;
            acted = true;
        }
        Ok(fetched) => {
            // 已是最新版：丢弃下载的临时产物
            let _ = fs::remove_file(&fetched.binary);
            cli_checked = true;
        }
        Err(e) => println!("⚠️  检查系统 CLI 更新失败：{e}"),
    }

    // 2) 项目系统配置（仅在已初始化时）
    if project.is_initialized() {
        step("检查项目配置版本");
        let recorded = project
            .load_system()?
            .version
            .unwrap_or_else(|| current_version().to_string());

        if version::is_newer(&effective, &recorded) {
            project.write_system_config(&effective)?;
            println!("✅ 项目配置版本已同步：{recorded} → {effective}");
            println!("ℹ️ 请提交 {}", project.system_config_display());
            acted = true;
        } else if version::is_newer(&recorded, &effective) {
            println!("⚠️  项目配置版本（{recorded}）高于当前 CLI（{effective}），请升级系统 CLI");
        }
    }

    if !acted {
        if cli_checked {
            println!("✅ 已是最新（{effective}）");
        } else {
            println!("ℹ️ 项目配置已是最新（{effective}）；系统 CLI 更新未检查");
        }
    }

    if is_interactive() {
        println!("   （耗时 {:.1}s）", started.elapsed().as_secs_f64());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use crate::update::Fetched;
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};

    struct FakeFetcher {
        result: Result<Fetched>,
    }

    impl Fetcher for FakeFetcher {
        fn fetch_latest(&self) -> Result<Fetched> {
            match &self.result {
                Ok(fetched) => Ok(Fetched {
                    version: fetched.version.clone(),
                    binary: fetched.binary.clone(),
                }),
                Err(e) => Err(e.clone()),
            }
        }
    }

    #[derive(Default)]
    struct FakeReplacer {
        replaced: RefCell<Vec<PathBuf>>,
    }

    impl Replacer for FakeReplacer {
        fn replace(&self, new_binary: &Path) -> Result<()> {
            self.replaced.borrow_mut().push(new_binary.to_path_buf());
            Ok(())
        }
    }

    fn project(dir: &TempDir) -> Project {
        Project::from_root(dir.path())
    }

    fn fake(version: &str) -> FakeFetcher {
        FakeFetcher {
            result: Ok(Fetched {
                version: version.to_string(),
                binary: PathBuf::from("/tmp/mtrim-fake"),
            }),
        }
    }

    #[test]
    fn updates_cli_and_project_config_when_both_outdated() {
        let dir = TempDir::new("update-both");
        dir.write(".mono-trim/config.toml", "version = \"0.0.1\"\n");
        dir.write(
            "mono-trim.toml",
            "[profiles.keep]\nmode = \"black\"\ntargets = []\n",
        );
        let replacer = FakeReplacer::default();

        run(&project(&dir), &fake("99.0.0"), &replacer).unwrap();

        assert_eq!(replacer.replaced.borrow().len(), 1);
        assert_eq!(
            project(&dir).load_system().unwrap().version.as_deref(),
            Some("99.0.0")
        );
        assert!(dir.read("mono-trim.toml").contains("profiles.keep"));
    }

    #[test]
    fn does_nothing_when_already_latest() {
        let dir = TempDir::new("update-noop");
        dir.write(
            ".mono-trim/config.toml",
            &format!("version = \"{}\"\n", current_version()),
        );
        let replacer = FakeReplacer::default();

        run(&project(&dir), &fake(current_version()), &replacer).unwrap();

        assert!(replacer.replaced.borrow().is_empty());
        assert_eq!(
            project(&dir).load_system().unwrap().version.as_deref(),
            Some(current_version())
        );
    }

    #[test]
    fn syncs_project_config_even_if_cli_check_fails() {
        let dir = TempDir::new("update-offline");
        dir.write(".mono-trim/config.toml", "version = \"0.0.1\"\n");
        let fetcher = FakeFetcher {
            result: Err("离线".into()),
        };

        run(&project(&dir), &fetcher, &FakeReplacer::default()).unwrap();

        assert_eq!(
            project(&dir).load_system().unwrap().version.as_deref(),
            Some(current_version())
        );
    }

    #[test]
    fn refuses_to_downgrade_project_config() {
        let dir = TempDir::new("update-newer-project");
        dir.write(".mono-trim/config.toml", "version = \"99.0.0\"\n");

        run(
            &project(&dir),
            &fake(current_version()),
            &FakeReplacer::default(),
        )
        .unwrap();

        assert_eq!(
            project(&dir).load_system().unwrap().version.as_deref(),
            Some("99.0.0")
        );
    }
}
