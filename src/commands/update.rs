//! `update`：升级系统 CLI，并把项目记录的版本同步到当前 CLI 版本。
//!
//! - 没有新版 CLI、项目版本也已同步时，不做任何写入
//! - 项目系统配置由内置默认值整体重写，用户配置永不改动

use crate::Result;
use crate::config::{Project, current_version};
use crate::update::{Fetcher, Replacer};
use crate::util::{is_interactive, step};
use crate::version;

pub fn run(
    project: &Project,
    fetcher: &dyn Fetcher,
    replacer: &dyn Replacer,
    target: Option<&str>,
) -> Result<()> {
    let started = std::time::Instant::now();
    let mut effective = current_version().to_string();
    let mut acted = false;
    let mut cli_checked = false;

    match target {
        // 指定版本：只允许升级
        Some(requested) => {
            let requested = requested.trim().trim_start_matches('v');
            if !version::is_valid(requested) {
                return Err(format!(
                    "版本号格式不正确：'{requested}'（期望 x.y.z，例如 0.0.9）"
                ));
            }

            println!(
                "⏳ 正在检查目标版本 {requested}（当前 {}）...",
                current_version()
            );

            if requested == current_version() {
                cli_checked = true;
                step("已是该版本，无需下载");
            } else if !version::is_newer(requested, current_version()) {
                return Err(format!(
                    "不支持降级：当前 {}，目标 {requested}",
                    current_version()
                ));
            } else {
                cli_checked = true;
                step(&format!("开始下载 {requested}"));
                let fetched = fetcher.download_version(requested)?;
                if fetched.version != requested {
                    return Err(format!(
                        "下载的版本 {} 与请求的版本 {requested} 不一致",
                        fetched.version
                    ));
                }
                replacer.replace(&fetched.binary)?;
                println!(
                    "✅ 已升级系统 CLI：{} → {}",
                    current_version(),
                    fetched.version
                );
                effective = fetched.version;
                acted = true;
            }
        }
        // 默认：只更新到最新版
        None => {
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
        }
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
        requested_versions: RefCell<Vec<String>>,
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
                requested_versions: RefCell::new(Vec::new()),
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
                requested_versions: RefCell::new(Vec::new()),
            }
        }

        fn requested(&self) -> Vec<String> {
            self.requested_versions.borrow().clone()
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
            self.requested_versions
                .borrow_mut()
                .push("latest".to_string());
            match &self.download {
                Ok(fetched) => Ok(Fetched {
                    version: fetched.version.clone(),
                    binary: fetched.binary.clone(),
                }),
                Err(e) => Err(e.clone()),
            }
        }

        fn download_version(&self, version: &str) -> Result<Fetched> {
            self.downloads.set(self.downloads.get() + 1);
            self.requested_versions
                .borrow_mut()
                .push(version.to_string());
            match &self.download {
                Ok(_) => Ok(Fetched {
                    version: version.to_string(),
                    binary: PathBuf::from("/tmp/mtrim-fake"),
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
        dir.write(".mono-trim/config.toml", "version = \"0.0.1\"\n");
        dir.write(
            "mono-trim.toml",
            "[profiles.keep]\nmode = \"black\"\ntargets = []\n",
        );
        let replacer = FakeReplacer::default();
        let fetcher = FakeFetcher::new("99.0.0");

        run(&project(&dir), &fetcher, &replacer, None).unwrap();

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
            &format!("version = \"{}\"\n", current_version()),
        );
        let replacer = FakeReplacer::default();
        let fetcher = FakeFetcher::new(current_version());

        run(&project(&dir), &fetcher, &replacer, None).unwrap();

        assert_eq!(fetcher.downloads(), 0, "已是最新时不应下载产物");
        assert!(replacer.replaced.borrow().is_empty());
    }

    #[test]
    fn skips_download_when_remote_is_older() {
        let dir = TempDir::new("update-older");
        let replacer = FakeReplacer::default();
        let fetcher = FakeFetcher::new("0.0.1");

        run(&project(&dir), &fetcher, &replacer, None).unwrap();

        assert_eq!(fetcher.downloads(), 0, "远端更旧时不应下载");
        assert!(replacer.replaced.borrow().is_empty());
    }

    #[test]
    fn syncs_project_config_even_if_version_check_fails() {
        let dir = TempDir::new("update-offline");
        dir.write(".mono-trim/config.toml", "version = \"0.0.1\"\n");
        let fetcher = FakeFetcher::failing_latest("离线");

        run(&project(&dir), &fetcher, &FakeReplacer::default(), None).unwrap();

        assert_eq!(fetcher.downloads(), 0);
        assert_eq!(
            project(&dir).load_system().unwrap().version.as_deref(),
            Some(current_version())
        );
    }

    #[test]
    fn updates_to_specific_version() {
        let dir = TempDir::new("update-target");
        dir.write(".mono-trim/config.toml", "version = \"0.0.1\"\n");
        let replacer = FakeReplacer::default();
        let fetcher = FakeFetcher::new("99.0.0");

        run(&project(&dir), &fetcher, &replacer, Some("99.0.0")).unwrap();

        assert_eq!(fetcher.requested(), vec!["99.0.0".to_string()]);
        assert_eq!(replacer.replaced.borrow().len(), 1);
        assert_eq!(
            project(&dir).load_system().unwrap().version.as_deref(),
            Some("99.0.0")
        );
    }

    #[test]
    fn rejects_downgrade_to_specific_version() {
        let dir = TempDir::new("update-downgrade");
        let fetcher = FakeFetcher::new("0.0.1");

        let err = run(
            &project(&dir),
            &fetcher,
            &FakeReplacer::default(),
            Some("0.0.1"),
        )
        .unwrap_err();

        assert!(err.contains("不支持降级"), "{err}");
        assert_eq!(fetcher.downloads(), 0);
    }

    #[test]
    fn skips_download_for_current_version() {
        let dir = TempDir::new("update-same-version");
        let fetcher = FakeFetcher::new(current_version());

        run(
            &project(&dir),
            &fetcher,
            &FakeReplacer::default(),
            Some(current_version()),
        )
        .unwrap();

        assert_eq!(fetcher.downloads(), 0);
    }

    #[test]
    fn rejects_invalid_version_argument() {
        let dir = TempDir::new("update-invalid-version");
        let fetcher = FakeFetcher::new("0.0.1");

        let err = run(
            &project(&dir),
            &fetcher,
            &FakeReplacer::default(),
            Some("latest"),
        )
        .unwrap_err();

        assert!(err.contains("版本号格式不正确"), "{err}");
        assert_eq!(fetcher.downloads(), 0);
    }

    #[test]
    fn refuses_to_downgrade_project_config() {
        let dir = TempDir::new("update-newer-project");
        dir.write(".mono-trim/config.toml", "version = \"99.0.0\"\n");

        run(
            &project(&dir),
            &FakeFetcher::new(current_version()),
            &FakeReplacer::default(),
            None,
        )
        .unwrap();

        assert_eq!(
            project(&dir).load_system().unwrap().version.as_deref(),
            Some("99.0.0")
        );
    }
}
