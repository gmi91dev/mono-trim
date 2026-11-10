//! Git 调用封装：仓库判定、路径定位、稀疏检出状态查询。

use std::path::PathBuf;
use std::process::{Command, Output};

use crate::Result;

fn git(args: &[&str]) -> Result<Output> {
    Command::new("git")
        .args(args)
        .output()
        .map_err(|e| format!("无法执行 git（{e}），请确认已安装并加入 PATH。"))
}

/// 执行 git 并要求成功，失败时透传 stderr
pub fn git_ok(args: &[&str]) -> Result<()> {
    let output = git(args)?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    Err(if detail.is_empty() {
        format!("git {} 执行失败。", args.join(" "))
    } else {
        format!("git {} 执行失败：{detail}", args.join(" "))
    })
}

fn git_stdout(args: &[&str]) -> Result<String> {
    let output = git(args)?;
    if !output.status.success() {
        return Err(format!("git {} 执行失败。", args.join(" ")));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// 项目根目录：优先取 Git 顶层目录，兼容从子目录调用
pub fn project_root() -> PathBuf {
    match git_stdout(&["rev-parse", "--show-toplevel"]) {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => PathBuf::from("."),
    }
}

pub fn ensure_repo() -> Result<()> {
    match git_stdout(&["rev-parse", "--is-inside-work-tree"]) {
        Ok(flag) if flag == "true" => Ok(()),
        _ => Err("当前目录不是一个 Git 仓库，请前往项目根目录运行。".into()),
    }
}

/// 兼容 worktree / 子目录调用，由 Git 自己给出 sparse-checkout 文件位置
pub fn sparse_checkout_path() -> Result<PathBuf> {
    let path = git_stdout(&["rev-parse", "--git-path", "info/sparse-checkout"])?;
    if path.is_empty() {
        return Err("无法定位 Git 稀疏检出配置文件。".into());
    }
    Ok(PathBuf::from(path))
}

pub fn sparse_enabled() -> Result<bool> {
    Ok(git_stdout(&["config", "--bool", "core.sparseCheckout"])? == "true")
}
