use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

/// 从资源管理器点开一个 Markdown 文件时的窗口行为
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpenMode {
    /// 复用已经打开的窗口，在里面加一个标签
    #[default]
    NewTab,
    /// 每个文件用自己的窗口。Windows 上表现为新起一个进程，
    /// 所以该模式不做会话持久化，见 [`Settings::keeps_session`]
    NewWindow,
}

/// 标签栏是否保留之前打开过的文件
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabMode {
    /// 累计标签，并在下次启动时恢复
    #[default]
    Accumulate,
    /// 只保留当前一个标签，启动时也不恢复上次的标签
    Single,
}

/// 用户偏好。容器上带 `serde(default)`，所以手工编辑漏了字段、
/// 或者以后新增字段，都不会让整份配置失效
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub open_mode: OpenMode,
    pub tab_mode: TabMode,
    /// 左侧栏是否展开。不进设置面板，只是把开合状态记住跨启动
    pub sidebar_open: bool,
    /// 作者模式：在标题和正文旁边显示复制按钮，方便整段取用去发布
    pub author_mode: bool,
}

impl Settings {
    /// 读不到或解析失败都退回默认值：偏好文件损坏不该让应用起不来
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        fs::write(path, body)
    }

    /// 只有「复用窗口 + 累计标签」这一种组合需要跨启动的 `session.json`：
    /// 新窗口模式是多进程，两个进程会互相覆盖对方的标签；单标签模式按设计不恢复
    pub fn keeps_session(&self) -> bool {
        self.open_mode == OpenMode::NewTab && self.tab_mode == TabMode::Accumulate
    }

    /// 应用页面发来的 `set-setting:<key>=<value>`。
    /// 返回是否真的改变了取值，未知键、未知值和重复设置都返回 false
    pub fn apply(&mut self, key: &str, value: &str) -> bool {
        let updated = match (key, value) {
            ("open-mode", "new-tab") => Self {
                open_mode: OpenMode::NewTab,
                ..*self
            },
            ("open-mode", "new-window") => Self {
                open_mode: OpenMode::NewWindow,
                ..*self
            },
            ("tab-mode", "accumulate") => Self {
                tab_mode: TabMode::Accumulate,
                ..*self
            },
            ("tab-mode", "single") => Self {
                tab_mode: TabMode::Single,
                ..*self
            },
            ("author-mode", "on") => Self {
                author_mode: true,
                ..*self
            },
            ("author-mode", "off") => Self {
                author_mode: false,
                ..*self
            },
            ("sidebar", "1") => Self {
                sidebar_open: true,
                ..*self
            },
            ("sidebar", "0") => Self {
                sidebar_open: false,
                ..*self
            },
            _ => return false,
        };
        if updated == *self {
            return false;
        }
        *self = updated;
        true
    }

    /// 推给页面回显当前选中项
    pub fn to_json(self) -> String {
        serde_json::to_string(&self).expect("settings are serializable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_the_current_reuse_window_and_accumulate_behavior() {
        let settings = Settings::default();

        assert_eq!(settings.open_mode, OpenMode::NewTab);
        assert_eq!(settings.tab_mode, TabMode::Accumulate);
        assert!(settings.keeps_session());
        assert!(!settings.sidebar_open);
        assert!(!settings.author_mode);
    }

    #[test]
    fn only_reuse_window_with_accumulated_tabs_keeps_a_session_file() {
        let mut settings = Settings::default();

        settings.open_mode = OpenMode::NewWindow;
        assert!(!settings.keeps_session());

        settings.open_mode = OpenMode::NewTab;
        settings.tab_mode = TabMode::Single;
        assert!(!settings.keeps_session());
    }

    #[test]
    fn apply_reports_real_changes_and_ignores_unknown_input() {
        let mut settings = Settings::default();

        assert!(settings.apply("tab-mode", "single"));
        assert_eq!(settings.tab_mode, TabMode::Single);
        assert!(!settings.apply("tab-mode", "single"));
        assert!(!settings.apply("tab-mode", "banana"));
        assert!(!settings.apply("zoom", "200"));
        assert!(settings.apply("author-mode", "on"));
        assert!(settings.author_mode);
        assert!(!settings.apply("author-mode", "on"));
        assert!(!settings.apply("author-mode", "yes"));
        assert!(settings.author_mode);
        assert!(settings.apply("sidebar", "1"));
        assert!(settings.sidebar_open);
        assert!(!settings.apply("sidebar", "1"));
        assert!(settings.apply("sidebar", "0"));
        assert!(!settings.sidebar_open);
        assert_eq!(settings.tab_mode, TabMode::Single);
        assert_eq!(settings.open_mode, OpenMode::NewTab);
    }

    #[test]
    fn settings_round_trip_through_disk_and_survive_a_corrupt_file() {
        let dir = std::env::temp_dir().join(format!(
            "md-previewer-settings-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let saved = Settings {
            open_mode: OpenMode::NewWindow,
            tab_mode: TabMode::Single,
            sidebar_open: true,
            author_mode: true,
        };

        saved.save(&path).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"openMode\": \"new-window\""), "{raw}");
        assert!(raw.contains("\"tabMode\": \"single\""), "{raw}");
        assert_eq!(Settings::load(&path), saved);

        fs::write(&path, "{ not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        assert_eq!(
            Settings::load(&dir.join("absent.json")),
            Settings::default()
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_partial_settings_file_fills_missing_fields_with_defaults() {
        let dir = std::env::temp_dir().join(format!(
            "md-previewer-settings-partial-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"tabMode":"single"}"#).unwrap();

        let loaded = Settings::load(&path);

        assert_eq!(loaded.tab_mode, TabMode::Single);
        assert_eq!(loaded.open_mode, OpenMode::NewTab);
        let _ = fs::remove_dir_all(dir);
    }
}
