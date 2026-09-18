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

/// 界面外观。跟随系统时由 WebView 和窗口各自读取系统偏好，
/// 手动指定时同时压到窗口标题栏和 WebView 的首选配色上
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
    }

    /// 未知取值退回跟随系统，配置文件被手改坏也不至于没有主题
    pub fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "light" => ThemeChoice::Light,
            "dark" => ThemeChoice::Dark,
            _ => ThemeChoice::System,
        }
    }
}

fn default_true() -> bool {
    true
}

/// 用户偏好。容器上带 `serde(default)`，所以手工编辑漏了字段、
/// 或者以后新增字段，都不会让整份配置失效
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub open_mode: OpenMode,
    pub tab_mode: TabMode,
    /// 左侧栏是否展开。不进设置面板，只是把开合状态记住跨启动
    pub sidebar_open: bool,
    /// 作者模式：在标题和正文旁边显示复制按钮，方便整段取用去发布
    pub author_mode: bool,
    /// 自动换行：代码块和编辑器正文是否自动软换行
    #[serde(default = "default_true")]
    pub word_wrap: bool,
    /// 外观：跟随系统 / 浅色 / 深色。macOS 菜单和设置面板共用这一个字段
    pub theme: ThemeChoice,
    /// 是否禁用所有快捷键
    pub disable_all_shortcuts: bool,
    /// 单独禁用的快捷键 ID 列表
    #[serde(default)]
    pub disabled_shortcuts: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            open_mode: OpenMode::default(),
            tab_mode: TabMode::default(),
            sidebar_open: false,
            author_mode: false,
            word_wrap: true,
            theme: ThemeChoice::System,
            disable_all_shortcuts: false,
            disabled_shortcuts: Vec::new(),
        }
    }
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
        match (key, value) {
            ("open-mode", "new-tab") => {
                if self.open_mode == OpenMode::NewTab {
                    return false;
                }
                self.open_mode = OpenMode::NewTab;
                true
            }
            ("open-mode", "new-window") => {
                if self.open_mode == OpenMode::NewWindow {
                    return false;
                }
                self.open_mode = OpenMode::NewWindow;
                true
            }
            ("tab-mode", "accumulate") => {
                if self.tab_mode == TabMode::Accumulate {
                    return false;
                }
                self.tab_mode = TabMode::Accumulate;
                true
            }
            ("tab-mode", "single") => {
                if self.tab_mode == TabMode::Single {
                    return false;
                }
                self.tab_mode = TabMode::Single;
                true
            }
            ("author-mode", "on") => {
                if self.author_mode {
                    return false;
                }
                self.author_mode = true;
                true
            }
            ("author-mode", "off") => {
                if !self.author_mode {
                    return false;
                }
                self.author_mode = false;
                true
            }
            ("sidebar", "1") => {
                if self.sidebar_open {
                    return false;
                }
                self.sidebar_open = true;
                true
            }
            ("sidebar", "0") => {
                if !self.sidebar_open {
                    return false;
                }
                self.sidebar_open = false;
                true
            }
            ("word-wrap", "on") => {
                if self.word_wrap {
                    return false;
                }
                self.word_wrap = true;
                true
            }
            ("word-wrap", "off") => {
                if !self.word_wrap {
                    return false;
                }
                self.word_wrap = false;
                true
            }
            ("theme", "system" | "light" | "dark") => {
                let choice = ThemeChoice::from_str(value);
                if self.theme == choice {
                    return false;
                }
                self.theme = choice;
                true
            }
            ("disable-all-shortcuts", "on" | "1" | "true") => {
                if self.disable_all_shortcuts {
                    return false;
                }
                self.disable_all_shortcuts = true;
                true
            }
            ("disable-all-shortcuts", "off" | "0" | "false") => {
                if !self.disable_all_shortcuts {
                    return false;
                }
                self.disable_all_shortcuts = false;
                true
            }
            ("disable-shortcut", shortcut_id) => {
                if shortcut_id.is_empty()
                    || self.disabled_shortcuts.iter().any(|s| s == shortcut_id)
                {
                    return false;
                }
                self.disabled_shortcuts.push(shortcut_id.to_string());
                true
            }
            ("enable-shortcut", shortcut_id) => {
                let original_len = self.disabled_shortcuts.len();
                self.disabled_shortcuts.retain(|s| s != shortcut_id);
                self.disabled_shortcuts.len() != original_len
            }
            ("reset-shortcuts", _) => {
                if !self.disable_all_shortcuts && self.disabled_shortcuts.is_empty() {
                    return false;
                }
                self.disable_all_shortcuts = false;
                self.disabled_shortcuts.clear();
                true
            }
            _ => false,
        }
    }

    /// 推给页面回显当前选中项
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("settings are serializable")
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
        assert_eq!(settings.word_wrap, true);
        assert_eq!(settings.theme, ThemeChoice::System);
        assert!(!settings.disable_all_shortcuts);
        assert!(settings.disabled_shortcuts.is_empty());
    }

    #[test]
    fn theme_choice_parses_menu_and_panel_values() {
        assert_eq!(ThemeChoice::from_str("system"), ThemeChoice::System);
        assert_eq!(ThemeChoice::from_str("light"), ThemeChoice::Light);
        assert_eq!(ThemeChoice::from_str(" Dark\n"), ThemeChoice::Dark);
        assert_eq!(ThemeChoice::from_str("unexpected"), ThemeChoice::System);
        assert_eq!(ThemeChoice::Dark.as_str(), "dark");
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
        assert!(settings.apply("word-wrap", "off"));
        assert!(!settings.word_wrap);
        assert!(!settings.apply("word-wrap", "off"));
        assert!(settings.apply("word-wrap", "on"));
        assert!(settings.word_wrap);
        assert!(settings.apply("theme", "dark"));
        assert_eq!(settings.theme, ThemeChoice::Dark);
        assert!(!settings.apply("theme", "dark"));
        assert!(!settings.apply("theme", "blue"));
        assert_eq!(settings.theme, ThemeChoice::Dark);
        assert!(settings.apply("theme", "system"));
        assert_eq!(settings.theme, ThemeChoice::System);
        assert_eq!(settings.tab_mode, TabMode::Single);
        assert_eq!(settings.open_mode, OpenMode::NewTab);

        // 快捷键设置测试
        assert!(settings.apply("disable-all-shortcuts", "on"));
        assert!(settings.disable_all_shortcuts);
        assert!(!settings.apply("disable-all-shortcuts", "on"));
        assert!(settings.apply("disable-all-shortcuts", "off"));
        assert!(!settings.disable_all_shortcuts);

        assert!(settings.apply("disable-shortcut", "close-tab"));
        assert_eq!(settings.disabled_shortcuts, vec!["close-tab"]);
        assert!(!settings.apply("disable-shortcut", "close-tab"));
        assert!(settings.apply("disable-shortcut", "new-file"));
        assert_eq!(settings.disabled_shortcuts, vec!["close-tab", "new-file"]);
        assert!(settings.apply("enable-shortcut", "close-tab"));
        assert_eq!(settings.disabled_shortcuts, vec!["new-file"]);
        assert!(!settings.apply("enable-shortcut", "close-tab"));
        assert!(settings.apply("reset-shortcuts", ""));
        assert!(settings.disabled_shortcuts.is_empty());
        assert!(!settings.disable_all_shortcuts);
        assert!(!settings.apply("reset-shortcuts", ""));
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
            word_wrap: false,
            theme: ThemeChoice::Dark,
            disable_all_shortcuts: true,
            disabled_shortcuts: vec!["close-tab".to_string(), "toggle-edit".to_string()],
        };

        saved.save(&path).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"openMode\": \"new-window\""), "{raw}");
        assert!(raw.contains("\"tabMode\": \"single\""), "{raw}");
        assert!(raw.contains("\"wordWrap\": false"), "{raw}");
        assert!(raw.contains("\"theme\": \"dark\""), "{raw}");
        assert!(raw.contains("\"disableAllShortcuts\": true"), "{raw}");
        assert!(raw.contains("\"disabledShortcuts\": ["), "{raw}");
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
        assert!(loaded.word_wrap);
        assert_eq!(loaded.theme, ThemeChoice::System);
        assert!(!loaded.disable_all_shortcuts);
        assert!(loaded.disabled_shortcuts.is_empty());
        let _ = fs::remove_dir_all(dir);
    }
}
