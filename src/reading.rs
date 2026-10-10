// 阅读位置记忆：只为「记住位置」的文件保存阅读比例，键是文件路径
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize)]
struct PersistedReading {
    version: u8,
    positions: BTreeMap<PathBuf, f64>,
}

#[derive(Debug, Default)]
pub(crate) struct ReadingPositions {
    /// 路径 -> 阅读进度比例（0~1）；只存已记住的文件，没记住的走默认的从头打开
    positions: BTreeMap<PathBuf, f64>,
}

impl ReadingPositions {
    /// 读取时顺手清掉文件已经不存在的记录，配置不会随着换文件一直变大
    pub(crate) fn load(path: &Path) -> Self {
        let Ok(raw) = fs::read(path) else {
            return Self::default();
        };
        let Ok(saved) = serde_json::from_slice::<PersistedReading>(&raw) else {
            return Self::default();
        };
        if saved.version != 1 {
            return Self::default();
        }
        let mut positions = Self::default();
        for (path, progress) in saved.positions {
            // 手改或损坏的记录里可能出现相对路径、目录或越界比例，一律丢掉
            if !path.is_absolute() || !is_valid_progress(progress) {
                continue;
            }
            // 只在"父目录还在、文件确实没了"时才当成失效记录：移动硬盘没插、网盘还没挂上
            // 时父目录也不存在，那种情况要留着，不然一次误判就把书签永久删掉了
            if !path.is_file() && path.parent().is_some_and(Path::exists) {
                continue;
            }
            positions.positions.insert(path, progress);
        }
        positions
    }

    /// 没有记录时不留空文件：全部取消记住后配置文件应该消失
    pub(crate) fn save(&self, path: &Path) {
        if self.positions.is_empty() {
            if let Err(error) = fs::remove_file(path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("Could not remove reading positions: {error}");
                }
            }
            return;
        }
        if let Some(dir) = path.parent() {
            if let Err(error) = fs::create_dir_all(dir) {
                eprintln!("Could not create reading positions dir: {error}");
                return;
            }
        }
        let saved = PersistedReading {
            version: 1,
            positions: self.positions.clone(),
        };
        match serde_json::to_vec(&saved) {
            Ok(raw) => {
                if let Err(error) = fs::write(path, raw) {
                    eprintln!("Could not save reading positions: {error}");
                }
            }
            Err(error) => eprintln!("Could not encode reading positions: {error}"),
        }
    }

    pub(crate) fn get(&self, path: &Path) -> Option<f64> {
        self.positions.get(path).copied()
    }

    pub(crate) fn remember(&self, path: &Path) -> bool {
        self.positions.contains_key(path)
    }

    pub(crate) fn set(&mut self, path: &Path, progress: f64) {
        if !is_valid_progress(progress) {
            return;
        }
        self.positions.insert(path.to_path_buf(), progress);
    }

    pub(crate) fn remove(&mut self, path: &Path) {
        self.positions.remove(path);
    }
}

fn is_valid_progress(progress: f64) -> bool {
    progress.is_finite() && (0.0..=1.0).contains(&progress)
}
