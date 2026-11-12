//! 系统 CLI 自更新：从 GitHub Release 下载与当前平台匹配的产物。
//!
//! 下载与替换都通过 trait 抽象，便于在没有网络（或没有真实 Release）时测试决策逻辑。

use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;

use crate::Result;
use crate::util::make_executable;

/// 发布仓库
pub const REPO: &str = "gmi91dev/mono-trim";

/// 已下载的最新版产物
pub struct Fetched {
    pub version: String,
    pub binary: PathBuf,
}

/// 获取最新版（网络实现或测试替身）
pub trait Fetcher {
    /// 只查询最新版本号（下载 `version.txt`，几百字节），不下载二进制
    fn latest_version(&self) -> Result<String>;

    /// 下载最新版二进制并校验（仅在确实需要升级时调用）
    fn download_latest(&self) -> Result<Fetched>;
}

/// 用新产物替换当前可执行文件
pub trait Replacer {
    fn replace(&self, new_binary: &Path) -> Result<()>;
}

/// 当前平台对应的产物名，例如 `mtrim-win-x86_64.exe`
pub fn asset_name() -> String {
    let os = match std::env::consts::OS {
        "windows" => "win",
        "macos" => "macos",
        other => other,
    };
    let ext = if cfg!(windows) { ".exe" } else { "" };
    format!("mtrim-{os}-{}{ext}", std::env::consts::ARCH)
}

/// 下载源：默认 GitHub，可用 `MONO_TRIM_MIRROR` 指向内网镜像
pub fn release_base() -> String {
    std::env::var("MONO_TRIM_MIRROR")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("https://github.com/{REPO}"))
}

pub fn latest_url(asset: &str) -> String {
    format!("{}/releases/latest/download/{asset}", release_base())
}

/// 从 `sha256sums.txt` 中解析指定文件的校验和
/// 兼容 `hash  file`、`hash *file`、`hash ./file` 三种格式
pub fn parse_checksum(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let file = parts
            .next()?
            .trim_start_matches('*')
            .trim_start_matches("./");
        (file == name).then(|| hash.to_lowercase())
    })
}

/// 下载后的临时文件名
///
/// - Windows 必须保留 `.exe` 才能执行；
/// - 带上进程号，避免多个 `mtrim update` 并发时互相删除临时文件。
fn download_file_name(asset: &str) -> String {
    let pid = std::process::id();
    if cfg!(windows) {
        format!("mtrim-{pid}-{asset}.download.exe")
    } else {
        format!("mtrim-{pid}-{asset}.download")
    }
}

/// 通过 GitHub Release 的 `latest/download` 直链下载（无需解析 API）
pub struct GithubFetcher;

impl Fetcher for GithubFetcher {
    fn latest_version(&self) -> Result<String> {
        let dest = std::env::temp_dir().join(format!("mtrim-version-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&dest);
        download(&latest_url("version.txt"), &dest, false)?;
        let text = std::fs::read_to_string(&dest).map_err(|e| format!("读取版本文件失败：{e}"))?;
        let _ = std::fs::remove_file(&dest);

        let version = text.trim();
        if crate::version::is_valid(version) {
            Ok(version.to_string())
        } else {
            Err(format!("无法识别的版本文件内容：{version}"))
        }
    }

    fn download_latest(&self) -> Result<Fetched> {
        let asset = asset_name();
        println!("→ 正在下载 {asset}");
        let temp = std::env::temp_dir();
        let sums = temp.join(format!("mtrim-sums-{}.txt", std::process::id()));
        let dest = temp.join(download_file_name(&asset));
        let _ = std::fs::remove_file(&sums);
        let _ = std::fs::remove_file(&dest);

        // 先取校验和，再下载产物，避免下载后才发现校验失败
        download(&latest_url("sha256sums.txt"), &sums, false)?;
        let text =
            std::fs::read_to_string(&sums).map_err(|e| format!("读取校验和文件失败：{e}"))?;
        let expected = parse_checksum(&text, &asset)
            .ok_or_else(|| format!("校验文件中没有 {asset} 的记录"))?;
        let _ = std::fs::remove_file(&sums);

        download(&latest_url(&asset), &dest, true)?;

        println!("→ 正在校验 sha256");
        let actual = crate::util::sha256_file(&dest)?;
        if actual != expected {
            let _ = std::fs::remove_file(&dest);
            return Err(format!("sha256 校验失败（期望 {expected}，实际 {actual}）"));
        }

        make_executable(&dest);
        let version = read_version(&dest)?;
        Ok(Fetched {
            version,
            binary: dest,
        })
    }
}

fn download(url: &str, dest: &Path, show_progress: bool) -> Result<()> {
    // 失败时统一给出简洁提示，避免把各平台本地化的命令输出直接抛给用户
    let hint = || format!("下载 {url} 失败，请检查网络或手动从 Release 安装");

    // 优先 curl（Win10+ / macOS / Linux 通常自带）
    if want_progress() && show_progress {
        return curl_with_progress(url, dest);
    }

    let ok = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "--retry-all-errors", "-o"])
        .arg(dest)
        .arg(url)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if ok {
        return Ok(());
    }

    #[cfg(windows)]
    {
        let script = format!(
            "Invoke-WebRequest -UseBasicParsing -Uri '{url}' -OutFile '{}'",
            dest.display()
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .map_err(|e| format!("无法执行下载命令：{e}"))?;
        if output.status.success() {
            return Ok(());
        }
        Err(hint())
    }

    #[cfg(not(windows))]
    Err(hint())
}

/// 交互式终端：curl 静默下载，主线程轮询文件大小绘制自绘进度条
fn curl_with_progress(url: &str, dest: &Path) -> Result<()> {
    let total = remote_size(url);
    let mut child = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "--retry-all-errors", "-o"])
        .arg(dest)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法执行 curl：{e}"))?;

    let mut bar = crate::progress::ProgressBar::new("下载", total);
    let mut stderr = child.stderr.take();
    let mut last_size = 0;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    bar.finish();
                    return Ok(());
                }
                let mut message = String::new();
                if let Some(pipe) = stderr.as_mut() {
                    let _ = pipe.read_to_string(&mut message);
                }
                bar.finish();
                let detail = message.trim();
                return Err(if detail.is_empty() {
                    format!("下载 {url} 失败，请检查网络或手动从 Release 安装")
                } else {
                    format!("下载 {url} 失败：{detail}")
                });
            }
            Ok(None) => {
                if let Ok(metadata) = std::fs::metadata(dest) {
                    last_size = metadata.len();
                }
                bar.advance(last_size);
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                bar.finish();
                return Err(format!("等待下载进程失败：{e}"));
            }
        }
    }
}

/// 通过 HEAD 请求获取文件大小（失败时返回 None，进度条退化为已下载量）
fn remote_size(url: &str) -> Option<u64> {
    let output = Command::new("curl").args(["-fsSIL", url]).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let headers = String::from_utf8_lossy(&output.stdout);
    let mut size = None;
    for line in headers.lines() {
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            size = value.trim().parse::<u64>().ok().or(size);
        }
    }
    size.filter(|value| *value > 0)
}

/// 终端下才显示下载进度；重定向（CI / 管道）时保持安静
fn want_progress() -> bool {
    std::io::stderr().is_terminal()
}

fn read_version(binary: &Path) -> Result<String> {
    let output = Command::new(binary)
        .arg("--version")
        .output()
        .map_err(|e| format!("无法执行下载的产物：{e}"))?;
    if !output.status.success() {
        return Err("下载的产物无法运行（--version 失败）".into());
    }

    // 期望输出：`mtrim 0.2.0`
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.split_whitespace()
        .nth(1)
        .map(str::to_string)
        .filter(|version| crate::version::is_valid(version))
        .ok_or_else(|| format!("无法识别的版本输出：{}", text.trim()))
}

/// 替换当前正在运行的可执行文件
pub struct SelfReplacer;

impl Replacer for SelfReplacer {
    fn replace(&self, new_binary: &Path) -> Result<()> {
        let current = std::env::current_exe().map_err(|e| format!("无法定位当前程序：{e}"))?;
        make_executable(new_binary);

        // Windows 上运行中的 exe 不能被覆盖：先改名，再写入，失败则回滚
        let file_name = current
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "mtrim".to_string());
        let backup = current.with_file_name(format!("{file_name}.old-{}", std::process::id()));

        if cfg!(windows) {
            std::fs::rename(&current, &backup).map_err(|e| format!("无法替换当前程序（{e}）"))?;
            if let Err(e) = std::fs::copy(new_binary, &current) {
                let _ = std::fs::rename(&backup, &current);
                return Err(format!("写入新版本失败（{e}），已回滚"));
            }
            let _ = std::fs::remove_file(&backup);
            return Ok(());
        }

        let staged = current.with_file_name(format!("{file_name}.new-{}", std::process::id()));
        std::fs::copy(new_binary, &staged).map_err(|e| format!("无法写入新版本（{e}）"))?;
        make_executable(&staged);
        std::fs::rename(&staged, &current).map_err(|e| format!("无法替换当前程序（{e}）"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_name_uses_current_platform() {
        let asset = asset_name();
        assert!(asset.starts_with("mtrim-"), "{asset}");
        assert!(asset.contains(std::env::consts::ARCH), "{asset}");
        assert_eq!(asset.ends_with(".exe"), cfg!(windows), "{asset}");
    }

    #[test]
    fn latest_url_points_to_release_download() {
        // 注意：存在 MONO_TRIM_MIRROR 时以镜像为准，这里只校验默认拼接规则
        let url = latest_url("mtrim-win-x86_64.exe");
        assert!(
            url.ends_with("/releases/latest/download/mtrim-win-x86_64.exe"),
            "{url}"
        );
        assert!(url.starts_with("http"), "{url}");
    }

    #[test]
    fn curl_with_progress_copies_local_file() {
        let dir = crate::test_support::TempDir::new("dl-progress");
        let source = dir.write("source.bin", "hello mtrim");
        let dest = dir.path().join("dest.bin");

        let text = source.display().to_string().replace('\\', "/");
        let url = if text.starts_with('/') {
            format!("file://{text}")
        } else {
            format!("file:///{text}")
        };

        curl_with_progress(&url, &dest).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "hello mtrim");
    }

    #[test]
    fn progress_is_disabled_when_not_a_terminal() {
        // 测试环境 stderr 不是终端，不应输出进度条
        assert!(!want_progress());
    }

    #[test]
    fn parses_checksums_in_all_formats() {
        let text = "aaa  *mtrim-win-x86_64.exe
bbb  ./mtrim-macos-aarch64
ccc  mtrim-linux-x86_64
";

        assert_eq!(
            parse_checksum(text, "mtrim-win-x86_64.exe").as_deref(),
            Some("aaa")
        );
        assert_eq!(
            parse_checksum(text, "mtrim-macos-aarch64").as_deref(),
            Some("bbb")
        );
        assert_eq!(
            parse_checksum(text, "mtrim-linux-x86_64").as_deref(),
            Some("ccc")
        );
        assert_eq!(parse_checksum(text, "missing"), None);
    }

    #[test]
    fn download_name_is_process_scoped_and_keeps_extension() {
        let name = download_file_name("mtrim-win-x86_64.exe");
        assert!(name.contains(&std::process::id().to_string()), "{name}");
        assert!(name.contains("mtrim-win-x86_64.exe"), "{name}");
        if cfg!(windows) {
            assert!(name.ends_with(".exe"), "{name}");
        } else {
            assert!(name.ends_with(".download"), "{name}");
        }
    }
}
