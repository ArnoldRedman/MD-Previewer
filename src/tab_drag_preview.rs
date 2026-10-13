// 原生预览跨越 WebView 边界，透明命中和不激活样式保证它不会挡住目标标签栏
#[cfg(target_os = "windows")]
mod native {
    use std::io;
    use std::ptr;

    const WS_EX_LAYERED: u32 = 0x00080000;
    const WS_EX_TRANSPARENT: u32 = 0x00000020;
    const WS_EX_TOOLWINDOW: u32 = 0x00000080;
    const WS_EX_NOACTIVATE: u32 = 0x08000000;
    const WS_POPUP: u32 = 0x80000000;
    const WS_BORDER: u32 = 0x00800000;
    const WS_CHILD: u32 = 0x40000000;
    const WS_VISIBLE: u32 = 0x10000000;
    const SS_CENTERIMAGE: u32 = 0x00000200;
    const SS_ENDELLIPSIS: u32 = 0x00004000;
    const SS_NOPREFIX: u32 = 0x00000080;
    const WM_SETFONT: u32 = 0x0030;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_SHOWWINDOW: u32 = 0x0040;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn CreateWindowExW(
            ex: u32,
            class: *const u16,
            text: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: isize,
            menu: isize,
            instance: isize,
            param: *const std::ffi::c_void,
        ) -> isize;
        fn DestroyWindow(window: isize) -> i32;
        fn SetLayeredWindowAttributes(window: isize, color: u32, alpha: u8, flags: u32) -> i32;
        fn SetWindowPos(
            window: isize,
            after: isize,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            flags: u32,
        ) -> i32;
        fn SendMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateFontW(
            height: i32,
            width: i32,
            escapement: i32,
            orientation: i32,
            weight: i32,
            italic: u32,
            underline: u32,
            strikeout: u32,
            charset: u32,
            output: u32,
            clip: u32,
            quality: u32,
            pitch: u32,
            face: *const u16,
        ) -> isize;
        fn DeleteObject(object: isize) -> i32;
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub(crate) struct DragPreview {
        window: isize,
        font: isize,
        offset: i32,
    }

    impl DragPreview {
        pub(crate) fn new(name: &str, dirty: bool, scale: f64, x: i32, y: i32) -> io::Result<Self> {
            let px = |value: f64| (value * scale).round() as i32;
            let class = wide("STATIC");
            let mut preview = Self {
                window: 0,
                font: 0,
                offset: px(12.0),
            };
            unsafe {
                preview.window = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    class.as_ptr(),
                    wide("").as_ptr(),
                    WS_POPUP | WS_BORDER,
                    x + preview.offset,
                    y + preview.offset,
                    px(180.0),
                    px(32.0),
                    0,
                    0,
                    0,
                    ptr::null(),
                );
                if preview.window == 0 {
                    return Err(io::Error::last_os_error());
                }
                preview.font = CreateFontW(
                    -px(13.0),
                    0,
                    0,
                    0,
                    400,
                    0,
                    0,
                    0,
                    1,
                    0,
                    0,
                    5,
                    0,
                    wide("Segoe UI").as_ptr(),
                );
                if preview.font == 0 {
                    return Err(io::Error::last_os_error());
                }
                // 关闭标记单独占位，长文件名省略时也保持完整标签轮廓
                for (text, left, width) in [
                    (if dirty { "●" } else { "" }, 8.0, 14.0),
                    (name, 24.0, 128.0),
                    ("×", 158.0, 18.0),
                ] {
                    let child = CreateWindowExW(
                        0,
                        class.as_ptr(),
                        wide(text).as_ptr(),
                        WS_CHILD | WS_VISIBLE | SS_CENTERIMAGE | SS_ENDELLIPSIS | SS_NOPREFIX,
                        px(left),
                        0,
                        px(width),
                        px(30.0),
                        preview.window,
                        0,
                        0,
                        ptr::null(),
                    );
                    if child == 0 {
                        return Err(io::Error::last_os_error());
                    }
                    SendMessageW(child, WM_SETFONT, preview.font as usize, 1);
                }
                if SetLayeredWindowAttributes(preview.window, 0, 235, 2) == 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            preview.move_to(x, y);
            Ok(preview)
        }

        pub(crate) fn move_to(&self, x: i32, y: i32) {
            unsafe {
                SetWindowPos(
                    self.window,
                    -1,
                    x + self.offset,
                    y + self.offset,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }
    }

    impl Drop for DragPreview {
        fn drop(&mut self) {
            unsafe {
                if self.window != 0 {
                    DestroyWindow(self.window);
                }
                if self.font != 0 {
                    DeleteObject(self.font);
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) use native::DragPreview;

#[cfg(not(target_os = "windows"))]
pub(crate) struct DragPreview;

#[cfg(not(target_os = "windows"))]
impl DragPreview {
    pub(crate) fn new(
        _name: &str,
        _dirty: bool,
        _scale: f64,
        _x: i32,
        _y: i32,
    ) -> std::io::Result<Self> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "此平台暂不支持标签拖动预览",
        ))
    }
    pub(crate) fn move_to(&self, _x: i32, _y: i32) {}
}
