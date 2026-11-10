//! 稀疏检出规则生成：把 profile 翻译成 sparse-checkout 非 cone 模式规则，
//! 并对用户填写的目标路径做安全校验。
//!
//! 这里只做纯计算，不写文件、不打印日志，便于单元测试。

use crate::Result;
use crate::config::{Profile, TOOL_DIR};

/// 全量放行规则（黑名单首行 / reset 回退共用）
pub const INCLUDE_ALL: &str = "/*";

/// 一次裁剪的完整执行计划
#[derive(Debug, PartialEq, Eq)]
pub struct SparsePlan {
    /// 待写入 sparse-checkout 文件的规则文本
    pub patterns: String,
    /// 面向用户的逐条动作说明（保留用户在配置中填写的原始写法）
    pub actions: Vec<Action>,
}

/// 单条目标路径的处理动作
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// 黑名单：物理排除
    Exclude(String),
    /// 白名单：仅保留
    Keep(String),
}

/// 生成稀疏检出计划
pub fn build_plan(profile: &Profile) -> Result<SparsePlan> {
    let mut lines: Vec<String> = vec![INCLUDE_ALL.to_string()];
    let mut actions = Vec::new();

    match profile.mode.as_str() {
        // 黑名单：全量放行，再逐项物理挖除
        "black" => {
            for target in &profile.targets {
                let path = normalize_target(target)?;
                actions.push(Action::Exclude(target.clone()));
                lines.push(format!("!{path}"));
            }
        }
        // 白名单：全量封锁目录，仅放行根目录文件、工具目录与目标路径
        "white" => {
            lines.push("!/*/".to_string());
            lines.push(format!("/{TOOL_DIR}/"));
            for target in &profile.targets {
                let path = normalize_target(target)?;
                actions.push(Action::Keep(target.clone()));
                lines.extend(white_patterns(&path));
            }
        }
        other => {
            return Err(format!(
                "未知的 mode 类型 '{other}'，仅支持 \"black\" / \"white\"。"
            ));
        }
    }

    Ok(SparsePlan {
        patterns: format!("{}\n", lines.join("\n")),
        actions,
    })
}

/// 白名单模式下单条路径的放行规则：
/// 逐层放行父目录、封锁父目录下的其它内容，最后精确放行目标本身。
fn white_patterns(normalized: &str) -> Vec<String> {
    let is_dir = normalized.ends_with('/');
    let clean = normalized.trim_matches('/');
    let segments: Vec<&str> = clean.split('/').collect();

    let mut patterns = Vec::with_capacity(segments.len() * 2);
    for depth in 1..segments.len() {
        let ancestor = segments[..depth].join("/");
        patterns.push(format!("/{ancestor}/"));
        patterns.push(format!("!/{ancestor}/*"));
    }
    if is_dir {
        patterns.push(format!("/{clean}/"));
    } else {
        patterns.push(format!("/{clean}"));
    }
    patterns
}

/// 判断是否为“像绝对路径”的写法（跨平台一致）
///
/// 除标准库的 `is_absolute` 外，还识别 Windows 盘符（`C:/x`、`C:\x`）与 UNC（`//server/share`），
/// 这样在 Linux/macOS 上也能拦住从 Windows 拷来的绝对路径。
pub fn is_absolute_like(path: &str) -> bool {
    if std::path::Path::new(path).is_absolute() {
        return true;
    }

    let bytes = path.as_bytes();
    let has_drive = bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes.len() == 2 || bytes[2] == b'/' || bytes[2] == b'\\');
    let has_leading_sep = path.starts_with('/') || path.starts_with('\\');

    has_drive || has_leading_sep
}

/// 规范化并校验单个裁剪目标，返回带前导 `/` 的 sparse-checkout 路径
pub fn normalize_target(raw: &str) -> Result<String> {
    let target = raw.trim();
    if target.is_empty() {
        return Err("配置中存在空路径。".into());
    }
    if target.contains(['\n', '\r']) {
        return Err(format!("路径 '{raw}' 含有非法换行符。"));
    }

    let stripped = target.trim_start_matches('/');
    let stripped = stripped.trim_start_matches("./");
    if stripped.is_empty() {
        return Err(format!(
            "路径 '{target}' 会命中项目根目录，请指定具体的目录或文件。"
        ));
    }
    if is_absolute_like(stripped) {
        return Err(format!(
            "路径 '{target}' 必须是相对项目根目录的路径，不支持绝对路径。"
        ));
    }

    let is_dir = stripped.ends_with('/');
    let clean = stripped.trim_end_matches('/');
    for segment in clean.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(format!("路径 '{target}' 含有非法片段 '{segment}'。"));
        }
    }
    if clean == ".git" || clean.starts_with(".git/") {
        return Err(format!("路径 '{target}' 指向 .git，禁止裁剪。"));
    }
    if clean == TOOL_DIR || clean.starts_with(&format!("{TOOL_DIR}/")) {
        return Err(format!(
            "路径 '{target}' 指向 {TOOL_DIR}，禁止裁剪工具自身与配置。"
        ));
    }

    Ok(if is_dir {
        format!("/{clean}/")
    } else {
        format!("/{clean}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(mode: &str, targets: &[&str]) -> Profile {
        Profile {
            mode: mode.to_string(),
            desc: String::new(),
            targets: targets.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn normalizes_target_paths() {
        assert_eq!(normalize_target("apps/ios/").unwrap(), "/apps/ios/");
        assert_eq!(normalize_target("/apps/ios/").unwrap(), "/apps/ios/");
        assert_eq!(normalize_target("assets/a.mp4").unwrap(), "/assets/a.mp4");
        assert_eq!(normalize_target(" ./Makefile ").unwrap(), "/Makefile");
    }

    #[test]
    fn is_absolute_like_is_cross_platform() {
        let absolutes = [
            "/apps",
            "C:/apps",
            r"C:\apps",
            "//server/share",
            r"\\server\share",
        ];
        for absolute in absolutes {
            assert!(is_absolute_like(absolute), "'{absolute}' 应判定为绝对路径");
        }

        for relative in ["apps/ios/", "./apps", "a/b"] {
            assert!(
                !is_absolute_like(relative),
                "'{relative}' 不应判定为绝对路径"
            );
        }
    }

    #[test]
    fn rejects_unsafe_targets() {
        for target in ["", "/", ".git/", ".git/config", TOOL_DIR, "a/../b", "C:/x"] {
            assert!(
                normalize_target(target).is_err(),
                "目标 '{target}' 应被拒绝"
            );
        }
    }

    #[test]
    fn accepts_dot_prefixed_targets_but_not_empty_segments() {
        assert_eq!(normalize_target("./apps/ios/").unwrap(), "/apps/ios/");
        assert!(normalize_target("apps//ios").is_err());
        assert!(normalize_target("./").is_err());
    }

    #[test]
    fn black_mode_keeps_root_and_excludes_targets() {
        let plan = build_plan(&profile("black", &["apps/ios/", "assets/a.mp4"])).unwrap();

        assert_eq!(plan.patterns, "/*\n!/apps/ios/\n!/assets/a.mp4\n");
        assert_eq!(
            plan.actions,
            [
                Action::Exclude("apps/ios/".into()),
                Action::Exclude("assets/a.mp4".into())
            ]
        );
    }

    #[test]
    fn white_mode_whitelists_parents_but_hides_siblings() {
        let plan = build_plan(&profile("white", &["apps/ios/"])).unwrap();
        assert_eq!(
            plan.patterns,
            format!("/*\n!/*/\n/{TOOL_DIR}/\n/apps/\n!/apps/*\n/apps/ios/\n")
        );

        let plan = build_plan(&profile("white", &["rust/src/main.rs"])).unwrap();
        assert!(
            plan.patterns
                .contains("/rust/\n!/rust/*\n/rust/src/\n!/rust/src/*\n/rust/src/main.rs")
        );
    }

    #[test]
    fn white_mode_always_protects_tool_dir() {
        let plan = build_plan(&profile("white", &[])).unwrap();
        assert_eq!(plan.patterns, format!("/*\n!/*/\n/{TOOL_DIR}/\n"));
        assert!(plan.patterns.ends_with('\n'), "规则文件应以换行结尾");
    }

    #[test]
    fn black_mode_rejects_unsafe_targets() {
        let err = build_plan(&profile("black", &[".git/config"])).unwrap_err();
        assert!(err.contains(".git"), "{err}");
    }

    #[test]
    fn rejects_unknown_mode() {
        assert!(build_plan(&profile("gray", &[])).is_err());
    }
}
