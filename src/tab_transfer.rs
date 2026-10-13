// 拖动协议只传状态；窗口归属与标签身份由宿主验证，页面不能指定任意接收窗口
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    pub(crate) raw: String,
    pub(crate) dirty: bool,
    pub(crate) edit_mode: bool,
    pub(crate) autosave_paused: bool,
    pub(crate) progress: f64,
    pub(crate) zoom: f64,
    pub(crate) selection_start: usize,
    pub(crate) selection_end: usize,
}

impl Snapshot {
    pub(crate) fn valid(&self) -> bool {
        self.progress.is_finite()
            && (0.0..=1.0).contains(&self.progress)
            && self.zoom.is_finite()
            && (25.0..=500.0).contains(&self.zoom)
            && self.selection_start <= self.selection_end
            && self.selection_end <= self.raw.encode_utf16().count()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct DragRequest {
    pub(crate) id: u64,
    pub(crate) snapshot: Option<Snapshot>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReceiveReady {
    pub(crate) token: u64,
    pub(crate) active_id: u64,
    pub(crate) content: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub(crate) struct TabBarRect {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl TabBarRect {
    pub(crate) fn contains(&self, x: f64, y: f64) -> bool {
        self.width > 0.0
            && self.height > 0.0
            && x >= self.x
            && x < self.x + self.width
            && y >= self.y
            && y < self.y + self.height
    }
}

pub(crate) struct Pointer {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) down: bool,
    pub(crate) cancel: bool,
    pub(crate) root: isize,
}

#[cfg(target_os = "windows")]
pub(crate) fn pointer() -> Option<Pointer> {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetCursorPos(point: *mut Point) -> i32;
        fn GetAsyncKeyState(key: i32) -> i16;
        fn WindowFromPoint(point: Point) -> isize;
        fn GetAncestor(window: isize, flags: u32) -> isize;
    }
    let mut point = Point { x: 0, y: 0 };
    unsafe {
        if GetCursorPos(&mut point) == 0 {
            return None;
        }
        Some(Pointer {
            x: point.x,
            y: point.y,
            down: GetAsyncKeyState(1) < 0,
            cancel: GetAsyncKeyState(0x1b) < 0,
            root: GetAncestor(WindowFromPoint(point), 2),
        })
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn pointer() -> Option<Pointer> {
    None
}

#[cfg(target_os = "windows")]
pub(crate) fn root(window: &tao::window::Window) -> isize {
    use tao::platform::windows::WindowExtWindows;
    window.hwnd()
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn root(_window: &tao::window::Window) -> isize {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_validates_unicode_selection_and_numeric_bounds() {
        let mut state = Snapshot {
            raw: "中😀文".into(),
            dirty: true,
            edit_mode: true,
            autosave_paused: false,
            progress: 0.5,
            zoom: 125.0,
            selection_start: 1,
            selection_end: 3,
        };
        assert!(state.valid());
        state.selection_end = 5;
        assert!(!state.valid());
        state.selection_end = 3;
        state.progress = f64::NAN;
        assert!(!state.valid());
        state.progress = 0.0;
        state.zoom = 0.0;
        assert!(!state.valid());
    }

    #[test]
    fn drop_rect_excludes_body_and_right_edge() {
        let rect = TabBarRect {
            x: 10.0,
            y: 0.0,
            width: 500.0,
            height: 40.0,
        };
        assert!(rect.contains(10.0, 0.0));
        assert!(rect.contains(509.0, 39.0));
        assert!(!rect.contains(510.0, 20.0));
        assert!(!rect.contains(20.0, 40.0));
        assert!(!rect.contains(f64::NAN, 10.0));
        assert!(!TabBarRect::default().contains(0.0, 0.0));
    }
}
