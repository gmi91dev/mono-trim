//! Git 调用封装：仓库判定、路径定位、稀疏检出状态查询。
//!
//! 具体执行通过 [`GitRunner`] 抽象，生产环境用 [`SystemGit`]，
//! 测试可注入假实现，从而无需真实仓库也能验证命令编排。

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Result;

/// git 命令输出（简化自 `std::process::Output`，便于测试替身构造）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

#[cfg(test)]
impl GitOutput {
    pub fn ok(stdout: impl Into<String>) -> Self {
        Self {
            success: true,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    pub fn fail(stderr: impl Into<String>) -> Self {
        Self {
            success: false,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }
}

/// 执行 git 的能力抽象
pub trait GitRunner {
    fn run(&self, args: &[&str]) -> Result<GitOutput>;

    /// 让 git 直接把输出/进度打到终端（用于 clone、reapply 等耗时操作）
    ///
    /// 默认实现回退到 `run`，便于测试替身复用。
    fn run_inherit(&self, args: &[&str]) -> Result<()> {
        let output = self.run(args)?;
        if output.success {
            Ok(())
        } else {
            let detail = output.stderr.trim();
            Err(if detail.is_empty() {
                format!("git {} 执行失败。", args.join(" "))
            } else {
                format!("git {} 执行失败：{detail}", args.join(" "))
            })
        }
    }
}

/// 真实调用系统 git
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemGit;

impl GitRunner for SystemGit {
    fn run_inherit(&self, args: &[&str]) -> Result<()> {
        let status = Command::new("git")
            .args(args)
            .status()
            .map_err(|e| format!("无法执行 git（{e}），请确认已安装并加入 PATH。"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "git {} 执行失败（退出码 {}）",
                args.join(" "),
                status.code().unwrap_or(-1)
            ))
        }
    }

    fn run(&self, args: &[&str]) -> Result<GitOutput> {
        let output = Command::new("git")
            .args(args)
            .output()
            .map_err(|e| format!("无法执行 git（{e}），请确认已安装并加入 PATH。"))?;
        Ok(GitOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// 执行 git 并要求成功，失败时透传 stderr
pub fn git_ok(git: &dyn GitRunner, args: &[&str]) -> Result<()> {
    let output = git.run(args)?;
    if output.success {
        return Ok(());
    }
    let detail = output.stderr.trim();
    Err(if detail.is_empty() {
        format!("git {} 执行失败。", args.join(" "))
    } else {
        format!("git {} 执行失败：{detail}", args.join(" "))
    })
}

fn stdout(git: &dyn GitRunner, args: &[&str]) -> Result<String> {
    let output = git.run(args)?;
    if !output.success {
        return Err(format!("git {} 执行失败。", args.join(" ")));
    }
    Ok(output.stdout.trim().to_string())
}

/// 项目根目录：优先取 Git 顶层目录，兼容从子目录调用
pub fn project_root(git: &dyn GitRunner) -> PathBuf {
    match stdout(git, &["rev-parse", "--show-toplevel"]) {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => PathBuf::from("."),
    }
}

pub fn ensure_repo(git: &dyn GitRunner) -> Result<()> {
    match stdout(git, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(flag) if flag == "true" => Ok(()),
        _ => Err("当前目录不是一个 Git 仓库，请前往项目根目录运行。".into()),
    }
}

/// 查询某个仓库相对路径是否被 .gitignore 忽略
///
/// 返回匹配的规则描述（例如 `.gitignore:3:mono-*.toml`）；未忽略或不是仓库时返回 None。
pub fn ignore_reason(git: &dyn GitRunner, root: &Path, relative: &str) -> Result<Option<String>> {
    let root = root.to_string_lossy().into_owned();

    // 先用 -q 判定：`check-ignore -v` 命中否定规则（!pattern）时退出码也是 0，
    // 只有 -q 能正确区分“真的被忽略”。
    let quiet = git.run(&["-C", &root, "check-ignore", "-q", "--", relative])?;
    if !quiet.success {
        return Ok(None);
    }

    let verbose = git.run(&["-C", &root, "check-ignore", "-v", "--", relative])?;
    let reason = verbose.stdout.trim();
    Ok(Some(if reason.is_empty() {
        "未知规则".to_string()
    } else {
        reason.to_string()
    }))
}

/// 兼容 worktree / 子目录调用，由 Git 自己给出 sparse-checkout 文件位置
pub fn sparse_checkout_path(git: &dyn GitRunner) -> Result<PathBuf> {
    let path = stdout(git, &["rev-parse", "--git-path", "info/sparse-checkout"])?;
    if path.is_empty() {
        return Err("无法定位 Git 稀疏检出配置文件。".into());
    }
    Ok(PathBuf::from(path))
}

pub fn sparse_enabled(git: &dyn GitRunner) -> Result<bool> {
    // 配置项未设置时 `git config` 返回非零，此时应视为「未启用」而非错误
    let output = git.run(&["config", "--bool", "core.sparseCheckout"])?;
    Ok(output.stdout.trim() == "true")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeGit;

    #[test]
    fn git_ok_propagates_failure() {
        let git = SystemGit;
        let err = git_ok(&git, &["--definitely-not-a-real-option"]).unwrap_err();
        assert!(
            err.starts_with("git ") || err.contains("无法执行 git"),
            "失败应带上 git 命令与错误原因：{err}"
        );
    }

    #[test]
    fn project_root_is_never_empty() {
        assert!(!project_root(&SystemGit).as_os_str().is_empty());
    }

    #[test]
    fn ensure_repo_accepts_only_true() {
        let git = FakeGit::new()
            .on("rev-parse --is-inside-work-tree", GitOutput::ok("true\n"))
            .on("rev-parse --is-inside-work-tree", GitOutput::ok("false\n"));
        assert!(ensure_repo(&git).is_ok());
        assert!(ensure_repo(&git).is_err());
        assert_eq!(git.calls().len(), 2);
    }

    #[test]
    fn sparse_path_and_state_come_from_git() {
        let git = FakeGit::new()
            .on(
                "rev-parse --git-path info/sparse-checkout",
                GitOutput::ok(".git/info/sparse-checkout"),
            )
            .on("config --bool core.sparseCheckout", GitOutput::ok("true"));
        assert_eq!(
            sparse_checkout_path(&git).unwrap(),
            PathBuf::from(".git/info/sparse-checkout")
        );
        assert!(sparse_enabled(&git).unwrap());
    }

    #[test]
    fn unset_sparse_config_means_disabled() {
        let git = FakeGit::new().on("config --bool core.sparseCheckout", GitOutput::fail(""));

        assert!(!sparse_enabled(&git).unwrap());
    }

    #[test]
    fn run_inherit_forwards_failure_from_fake() {
        let git = FakeGit::new().on("sparse-checkout reapply", GitOutput::fail("fatal: boom"));
        let err = git
            .run_inherit(&["sparse-checkout", "reapply"])
            .unwrap_err();
        assert!(err.contains("boom"), "{err}");
    }

    #[test]
    fn ignore_reason_returns_matching_rule() {
        let git = FakeGit::new()
            .on(
                "-C /repo check-ignore -q -- mono-trim.toml",
                GitOutput::ok(""),
            )
            .on(
                "-C /repo check-ignore -v -- mono-trim.toml",
                GitOutput::ok(".gitignore:3:mono-*.toml\tmono-trim.toml"),
            );
        let reason = ignore_reason(&git, Path::new("/repo"), "mono-trim.toml").unwrap();
        assert_eq!(
            reason.as_deref(),
            Some(".gitignore:3:mono-*.toml\tmono-trim.toml")
        );
    }

    #[test]
    fn ignore_reason_returns_none_when_not_ignored() {
        // 命中否定规则（!pattern）时 -q 的退出码是 1
        let git = FakeGit::new().on(
            "-C /repo check-ignore -q -- mono-trim.toml",
            GitOutput::fail(""),
        );
        assert!(
            ignore_reason(&git, Path::new("/repo"), "mono-trim.toml")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn missing_sparse_path_is_reported() {
        let git = FakeGit::new().on(
            "rev-parse --git-path info/sparse-checkout",
            GitOutput::ok(""),
        );
        assert!(sparse_checkout_path(&git).is_err());
    }
}
