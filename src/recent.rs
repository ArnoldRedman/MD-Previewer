//! 最近打开文件列表：内存里保持有序去重，每次变更立刻落盘，
//! 启动页和侧栏都从这里取数据

use crate::session::normalize_path;
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_RECENT_FILES: usize = 8;

pub struct RecentFiles {
    paths: Vec<PathBuf>,
    store: PathBuf,
}

impl RecentFiles {
    /// 从落盘文件读取。跳过空行、已经不存在的文件和重复项；
    /// 路径先规范化再比较，旧版本写入的 verbatim 前缀或大小写差异不会产生重复条目
    pub fn load(store: PathBuf) -> Self {
        let mut paths = Vec::new();
        if let Ok(txt) = fs::read_to_string(&store) {
            for line in txt.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let path = normalize_path(PathBuf::from(line));
                if !path.is_file() || paths.contains(&path) {
                    continue;
                }
                paths.push(path);
                if paths.len() == MAX_RECENT_FILES {
                    break;
                }
            }
        }
        Self { paths, store }
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    pub fn get(&self, index: usize) -> Option<&PathBuf> {
        self.paths.get(index)
    }

    /// 把文件提到最前；超出上限的旧记录被挤掉
    pub fn remember(&mut self, path: &Path) {
        let path = normalize_path(path.to_path_buf());
        self.paths.retain(|p| p != &path);
        self.paths.insert(0, path);
        self.paths.truncate(MAX_RECENT_FILES);
        self.save();
    }

    /// 移除一条记录；列表里本来没有时返回 false，调用方据此决定要不要刷新界面
    pub fn forget(&mut self, path: &Path) -> bool {
        let path = normalize_path(path.to_path_buf());
        let before = self.paths.len();
        self.paths.retain(|p| p != &path);
        if self.paths.len() == before {
            return false;
        }
        self.save();
        true
    }

    pub fn clear(&mut self) -> bool {
        if self.paths.is_empty() {
            return false;
        }
        self.paths.clear();
        self.save();
        true
    }

    // 落盘失败只影响下次启动的历史，不打断当前操作，但要留下痕迹便于排查
    fn save(&self) {
        if let Some(parent) = self.store.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                eprintln!("无法创建最近文件目录 {}: {error}", parent.display());
                return;
            }
        }
        let body = self
            .paths
            .iter()
            .map(|p| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");
        if let Err(error) = fs::write(&self.store, body) {
            eprintln!("无法写入最近文件列表 {}: {error}", self.store.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "md-previewer-recent-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_file(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, "# doc").unwrap();
        normalize_path(path)
    }

    #[test]
    fn remember_moves_to_front_dedupes_and_caps() {
        let dir = temp_dir("remember");
        let store = dir.join("recent-files.txt");
        let mut recent = RecentFiles::load(store.clone());
        let files = (0..MAX_RECENT_FILES + 2)
            .map(|i| make_file(&dir, &format!("{i}.md")))
            .collect::<Vec<_>>();
        for file in &files {
            recent.remember(file);
        }
        assert_eq!(recent.paths().len(), MAX_RECENT_FILES);
        assert_eq!(recent.paths()[0], files[files.len() - 1]);
        // 最早的两条被挤掉
        assert!(!recent.paths().contains(&files[0]));
        assert!(!recent.paths().contains(&files[1]));

        recent.remember(&files[3]);
        assert_eq!(recent.paths()[0], files[3]);
        assert_eq!(recent.paths().iter().filter(|p| **p == files[3]).count(), 1);

        let reloaded = RecentFiles::load(store);
        assert_eq!(reloaded.paths(), recent.paths());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn forget_and_clear_report_whether_anything_changed() {
        let dir = temp_dir("forget");
        let store = dir.join("recent-files.txt");
        let mut recent = RecentFiles::load(store.clone());
        let a = make_file(&dir, "a.md");
        let b = make_file(&dir, "b.md");
        recent.remember(&a);
        recent.remember(&b);

        assert!(recent.forget(&a));
        assert!(!recent.forget(&a));
        assert_eq!(recent.paths(), &[b.clone()]);
        assert_eq!(fs::read_to_string(&store).unwrap(), b.to_string_lossy());

        assert!(recent.clear());
        assert!(!recent.clear());
        assert!(recent.paths().is_empty());
        assert_eq!(fs::read_to_string(&store).unwrap(), "");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_skips_missing_blank_and_duplicate_entries() {
        let dir = temp_dir("load");
        let store = dir.join("recent-files.txt");
        let kept = make_file(&dir, "kept.md");
        let missing = dir.join("missing.md");
        let body = format!(
            "{}\n\n{}\n{}\n{}\n",
            kept.display(),
            missing.display(),
            kept.display(),
            dir.display()
        );
        fs::write(&store, body).unwrap();

        let recent = RecentFiles::load(store);
        // 缺失文件、重复行和目录都不进列表
        assert_eq!(recent.paths(), &[kept]);
        let _ = fs::remove_dir_all(dir);
    }
}
