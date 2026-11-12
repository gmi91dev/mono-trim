//! `update`：升级系统 CLI，并把项目记录的版本同步到当前 CLI 版本。
//!
//! - 没有新版 CLI、项目版本也已同步时，不做任何写入
//! - 项目系统配置由内置默认值整体重写，用户配置永不改动

use crate::Result;
use crate::config::{Project, current_version};
use crate::update::{Fetcher, Replacer};
use crate::util::{is_interactive, step};
use crate::version;

pub fn run(project: &Project, fetcher: &dyn Fetcher, replacer: &dyn Replacer) -> Result<()> {
    let started = std::time::Instant::now();
    let mut effective = current_version().to_string();
    let mut acted = false;
    let mut cli_checked = false;

    // 1) 系统 CLI：先查版本（不下载产物），只有更新才下载
    println!("⏳ 正在检查系统 CLI 更新（当前 {}）...", current_version());
    match fetcher.latest_version() {
        Ok(latest) if version::is_newer(&latest, current_version()) => {
            cli_checked = true;
            step(&format!("发现新版本 {latest}，开始下载"));
            match fetcher.download_latest() {
                Ok(fetched) => {
                    replacer.replace(&fetched.binary)?;
                    println!(
                        "✅ 已升级系统 CLI：{} → {}",
                        current_version(),
                        fetched.version
                    );
                    effective = fetched.version;
                    acted = true;
                }
                Err(e) => println!("⚠️  下载新版本失败：{e}"),
            }
        }
        Ok(latest) => {
            cli_checked = true;
            step(&format!("最新版本为 {latest}，无需下载"));
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
    use std::cell::{Cell, RefCell};
    use std::path::{Path, PathBuf};

    struct FakeFetcher {
        latest: Result<String>,
        download: Result<Fetched>,
        downloads: Cell<usize>,
    }

    impl FakeFetcher {
        fn new(latest: &str) -> Self {
            Self {
                latest: Ok(latest.to_string()),
                download: Ok(Fetched {
                    version: latest.to_string(),
                    binary: PathBuf::from("/tmp/mtrim-fake"),
                }),
                downloads: Cell::new(0),
            }
        }

        fn failing_latest(message: &str) -> Self {
            Self {
                latest: Err(message.to_string()),
                download: Ok(Fetched {
                    version: current_version().to_string(),
                    binary: PathBuf::from("/tmp/mtrim-fake"),
                }),
                downloads: Cell::new(0),
            }
        }

        fn downloads(&self) -> usize {
            self.downloads.get()
        }
    }

    impl Fetcher for FakeFetcher {
        fn latest_version(&self) -> Result<String> {
            self.latest.clone()
        }

        fn download_latest(&self) -> Result<Fetched> {
            self.downloads.set(self.downloads.get() + 1);
            match &self.download {
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

    #[test]
    fn updates_cli_and_project_config_when_both_outdated() {
        let dir = TempDir::new("update-both");
        dir.write(
            ".mono-trim/config.toml",
            "version = \"0.0.1\"
",
        );
        dir.write(
            "mono-trim.toml",
            "[profiles.keep]
mode = \"black\"
targets = []
",
        );
        let replacer = FakeReplacer::default();
        let fetcher = FakeFetcher::new("99.0.0");

        run(&project(&dir), &fetcher, &replacer).unwrap();

        assert_eq!(fetcher.downloads(), 1, "有新版本时应下载一次");
        assert_eq!(replacer.replaced.borrow().len(), 1);
        assert_eq!(
            project(&dir).load_system().unwrap().version.as_deref(),
            Some("99.0.0")
        );
        assert!(dir.read("mono-trim.toml").contains("profiles.keep"));
    }

    #[test]
    fn skips_download_when_already_latest() {
        let dir = TempDir::new("update-noop");
        dir.write(
            ".mono-trim/config.toml",
            &format!(
                "version = \"{}\"
",
                current_version()
            ),
        );
        let replacer = FakeReplacer::default();
        let fetcher = FakeFetcher::new(current_version());

        run(&project(&dir), &fetcher, &replacer).unwrap();

        assert_eq!(fetcher.downloads(), 0, "已是最新时不应下载产物");
        assert!(replacer.replaced.borrow().is_empty());
    }

    #[test]
    fn skips_download_when_remote_is_older() {
        let dir = TempDir::new("update-older");
        let replacer = FakeReplacer::default();
        let fetcher = FakeFetcher::new("0.0.1");

        run(&project(&dir), &fetcher, &replacer).unwrap();

        assert_eq!(fetcher.downloads(), 0, "远端更旧时不应下载");
        assert!(replacer.replaced.borrow().is_empty());
    }

    #[test]
    fn syncs_project_config_even_if_version_check_fails() {
        let dir = TempDir::new("update-offline");
        dir.write(
            ".mono-trim/config.toml",
            "version = \"0.0.1\"
",
        );
        let fetcher = FakeFetcher::failing_latest("离线");

        run(&project(&dir), &fetcher, &FakeReplacer::default()).unwrap();

        assert_eq!(fetcher.downloads(), 0);
        assert_eq!(
            project(&dir).load_system().unwrap().version.as_deref(),
            Some(current_version())
        );
    }

    #[test]
    fn refuses_to_downgrade_project_config() {
        let dir = TempDir::new("update-newer-project");
        dir.write(
            ".mono-trim/config.toml",
            "version = \"99.0.0\"
",
        );

        run(
            &project(&dir),
            &FakeFetcher::new(current_version()),
            &FakeReplacer::default(),
        )
        .unwrap();

        assert_eq!(
            project(&dir).load_system().unwrap().version.as_deref(),
            Some("99.0.0")
        );
    }
}
