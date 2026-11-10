//! 裁剪相关的动态命令：自定义 profile、reset、reapply。

use std::path::Path;

use crate::Result;
use crate::config::{CONFIG_NAME, read_config};
use crate::git::{GitRunner, git_ok, sparse_checkout_path, sparse_enabled};
use crate::sparse::{Action, INCLUDE_ALL, build_plan};
use crate::util::write_file;

/// 执行 `config.toml` 中声明的自定义 profile（由命令名动态触发）
pub fn profile(config_path: &Path, name: &str, git: &dyn GitRunner) -> Result<()> {
    let config = read_config(config_path)?;
    let profile = config.profiles.get(name).ok_or_else(|| {
        format!(
            "未知命令 '{name}'。输入 --help 查看内置命令，或检查 {CONFIG_NAME} 中的 profiles 节点。"
        )
    })?;

    println!("⏳ 正在执行自定义命令 [{name}] ({}) ...", profile.desc);
    // 先完整校验并生成计划，任何非法配置都在触碰工作空间之前拦截
    let plan = build_plan(profile)?;
    for action in &plan.actions {
        match action {
            Action::Exclude(target) => println!("   🚫 [黑名单排除] -> {target}"),
            Action::Keep(target) => println!("   ✅ [白名单仅保留] -> {target}"),
        }
    }

    git_ok(git, &["sparse-checkout", "init", "--no-cone"])?;
    write_file(&sparse_checkout_path(git)?, &plan.patterns)?;
    git_ok(git, &["sparse-checkout", "reapply"])?;

    println!("✨ 工作空间拓扑裁剪成功！");
    Ok(())
}

pub fn reset(git: &dyn GitRunner) -> Result<()> {
    println!("⏳ 正在恢复全量目录与文件...");
    if !sparse_enabled(git)? {
        println!("✨ 当前工作空间本就是全量状态，无需恢复。");
        return Ok(());
    }

    // 优先使用官方 disable 彻底关闭稀疏检出，失败则退回全量规则
    if git_ok(git, &["sparse-checkout", "disable"]).is_err() {
        write_file(&sparse_checkout_path(git)?, &format!("{INCLUDE_ALL}\n"))?;
        git_ok(git, &["sparse-checkout", "reapply"])?;
    }

    println!("✨ 已成功恢复所有平台的完整工作空间！");
    Ok(())
}

pub fn reapply(git: &dyn GitRunner) -> Result<()> {
    if !sparse_enabled(git)? {
        return Err("当前工作空间尚未启用稀疏检出，请先执行自定义裁剪命令。".into());
    }
    println!("⏳ 正在重新刷新本地物理硬盘目录...");
    git_ok(git, &["sparse-checkout", "reapply"])?;
    println!("✨ 已按现有稀疏规则刷新完成！");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::GitOutput;
    use crate::test_support::{FakeGit, TempDir, write_config};

    const CONFIG: &str = r#"[profiles.win-dev]
mode = "black"
desc = "隐藏 iOS 壳"
targets = ["apps/ios/"]
"#;

    fn config(dir: &TempDir) -> std::path::PathBuf {
        write_config(dir, CONFIG)
    }

    #[test]
    fn unknown_profile_is_rejected() {
        let dir = TempDir::new("apply-unknown");
        let path = config(&dir);
        let git = FakeGit::new();

        let err = profile(&path, "nope", &git).unwrap_err();

        assert!(err.contains("未知命令 'nope'"), "{err}");
        assert!(git.calls().is_empty(), "不应调用 git");
    }

    #[test]
    fn invalid_mode_fails_before_touching_workspace() {
        let dir = TempDir::new("apply-mode");
        let path = write_config(
            &dir,
            r#"[profiles.bad]
mode = "gray"
desc = "示例"
targets = []
"#,
        );
        let git = FakeGit::new();

        let err = profile(&path, "bad", &git).unwrap_err();

        assert!(err.contains("未知的 mode"), "{err}");
        assert!(git.calls().is_empty(), "不应调用 git");
    }

    #[test]
    fn writes_plan_then_reapplies() {
        let dir = TempDir::new("apply-write");
        let path = config(&dir);
        let sparse_path = dir.write(".git/info/sparse-checkout", "");
        let git = FakeGit::new()
            .on("sparse-checkout init --no-cone", GitOutput::ok(""))
            .on(
                "rev-parse --git-path info/sparse-checkout",
                GitOutput::ok(sparse_path.to_string_lossy()),
            )
            .on("sparse-checkout reapply", GitOutput::ok(""));

        profile(&path, "win-dev", &git).unwrap();

        assert_eq!(
            git.calls(),
            [
                "sparse-checkout init --no-cone",
                "rev-parse --git-path info/sparse-checkout",
                "sparse-checkout reapply"
            ]
        );
        assert_eq!(
            std::fs::read_to_string(&sparse_path).unwrap(),
            "/*\n!/apps/ios/\n"
        );
    }

    #[test]
    fn stops_when_sparse_init_fails() {
        let dir = TempDir::new("apply-init-fail");
        let path = config(&dir);
        let git = FakeGit::new().on(
            "sparse-checkout init --no-cone",
            GitOutput::fail("fatal: boom"),
        );

        let err = profile(&path, "win-dev", &git).unwrap_err();

        assert!(err.contains("boom"), "{err}");
        assert_eq!(git.calls(), ["sparse-checkout init --no-cone"]);
    }

    #[test]
    fn reset_is_noop_when_sparse_disabled() {
        let git = FakeGit::new().on("config --bool core.sparseCheckout", GitOutput::ok("false"));

        reset(&git).unwrap();

        assert_eq!(git.calls(), ["config --bool core.sparseCheckout"]);
    }

    #[test]
    fn reset_disables_sparse_checkout() {
        let git = FakeGit::new()
            .on("config --bool core.sparseCheckout", GitOutput::ok("true"))
            .on("sparse-checkout disable", GitOutput::ok(""));

        reset(&git).unwrap();

        assert_eq!(
            git.calls(),
            [
                "config --bool core.sparseCheckout",
                "sparse-checkout disable"
            ]
        );
    }

    #[test]
    fn reset_falls_back_to_full_patterns() {
        let dir = TempDir::new("apply-reset-fallback");
        let sparse_path = dir.write(".git/info/sparse-checkout", "old");
        let git = FakeGit::new()
            .on("config --bool core.sparseCheckout", GitOutput::ok("true"))
            .on(
                "sparse-checkout disable",
                GitOutput::fail("fatal: cannot disable"),
            )
            .on(
                "rev-parse --git-path info/sparse-checkout",
                GitOutput::ok(sparse_path.to_string_lossy()),
            )
            .on("sparse-checkout reapply", GitOutput::ok(""));

        reset(&git).unwrap();

        assert_eq!(std::fs::read_to_string(&sparse_path).unwrap(), "/*\n");
        assert_eq!(git.calls().last().unwrap(), "sparse-checkout reapply");
    }

    #[test]
    fn reapply_requires_sparse_checkout() {
        let git = FakeGit::new().on("config --bool core.sparseCheckout", GitOutput::ok("false"));

        let err = reapply(&git).unwrap_err();

        assert!(err.contains("尚未启用稀疏检出"), "{err}");
    }

    #[test]
    fn reapply_refreshes_when_enabled() {
        let git = FakeGit::new()
            .on("config --bool core.sparseCheckout", GitOutput::ok("true"))
            .on("sparse-checkout reapply", GitOutput::ok(""));

        reapply(&git).unwrap();

        assert_eq!(
            git.calls(),
            [
                "config --bool core.sparseCheckout",
                "sparse-checkout reapply"
            ]
        );
    }
}
