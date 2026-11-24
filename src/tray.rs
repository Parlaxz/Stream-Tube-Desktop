//! System-tray icon (Windows). The window's close button hides the window and
//! leaves the app running here; Exit and Restart live in the icon's right-click
//! menu. Other platforms keep the normal close-to-quit behavior.
use futures::channel::mpsc::UnboundedReceiver;

#[derive(Clone, Copy, Debug)]
pub enum TrayEvent {
    Show,
    Restart,
    Exit,
}

#[cfg(not(windows))]
pub fn init() -> Option<UnboundedReceiver<TrayEvent>> {
    None
}

#[cfg(not(windows))]
pub fn hide_to_tray(_window: &gpui_kit::Window) -> bool {
    false
}

#[cfg(windows)]
pub fn hide_to_tray(window: &gpui_kit::Window) -> bool {
    if !win::ACTIVE.load(std::sync::atomic::Ordering::SeqCst) {
        return false;
    }
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    #[link(name = "user32")]
    unsafe extern "system" {
        fn ShowWindowAsync(hwnd: isize, cmd: i32) -> i32;
    }
    if let Ok(h) = HasWindowHandle::window_handle(window) {
        if let RawWindowHandle::Win32(w) = h.as_raw() {
            // SAFETY: SW_HIDE on our own window handle
            unsafe { ShowWindowAsync(w.hwnd.get(), 0) };
            return true;
        }
    }
    false
}

#[cfg(windows)]
pub fn init() -> Option<UnboundedReceiver<TrayEvent>> {
    win::start(crate::store::settings_snapshot().accent)
}

#[cfg(windows)]
mod win {
    use super::TrayEvent;
    use futures::channel::mpsc::{unbounded, UnboundedSender};
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// False while no icon is on screen, so closing during exit still quits.
    pub static ACTIVE: AtomicBool = AtomicBool::new(false);
    static TX: OnceLock<UnboundedSender<TrayEvent>> = OnceLock::new();

    const WM_DESTROY: u32 = 0x0002;
    const WM_COMMAND: u32 = 0x0111;
    const WM_RBUTTONUP: u32 = 0x0205;
    const WM_CONTEXTMENU: u32 = 0x007B;
    const WM_LBUTTONUP: u32 = 0x0202;
    const WM_LBUTTONDBLCLK: u32 = 0x0203;
    const WM_NULL: u32 = 0x0000;
    const WM_TRAY: u32 = 0x8000 + 1; // WM_APP + 1

    const NIM_ADD: u32 = 0;
    const NIM_DELETE: u32 = 2;
    const NIF_MESSAGE: u32 = 0x1;
    const NIF_ICON: u32 = 0x2;
    const NIF_TIP: u32 = 0x4;

    const IMAGE_ICON: u32 = 1;
    const LR_DEFAULTSIZE: u32 = 0x40;
    const MF_STRING: u32 = 0x0;
    const MF_SEPARATOR: u32 = 0x800;
    const TPM_LEFTALIGN: u32 = 0x0;
    const TPM_BOTTOMALIGN: u32 = 0x20;
    const TPM_RIGHTBUTTON: u32 = 0x2;

    const ID_SHOW: usize = 1;
    const ID_RESTART: usize = 2;
    const ID_EXIT: usize = 3;

    #[repr(C)]
    #[allow(dead_code)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    #[repr(C)]
    #[allow(dead_code)]
    struct NotifyIconDataW {
        cb_size: u32,
        hwnd: isize,
        uid: u32,
        flags: u32,
        callback_message: u32,
        hicon: isize,
        tip: [u16; 128],
        state: u32,
        state_mask: u32,
        info: [u16; 256],
        timeout_or_version: u32,
        info_title: [u16; 64],
        info_flags: u32,
        guid: Guid,
        balloon_icon: isize,
    }

    #[repr(C)]
    #[allow(dead_code)]
    struct WndClassExW {
        cb_size: u32,
        style: u32,
        wnd_proc: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
        cls_extra: i32,
        wnd_extra: i32,
        instance: isize,
        icon: isize,
        cursor: isize,
        background: isize,
        menu_name: *const u16,
        class_name: *const u16,
        icon_small: isize,
    }

    #[repr(C)]
    #[allow(dead_code)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[allow(dead_code)]
    struct Msg {
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: Point,
        private: u32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterClassExW(class: *const WndClassExW) -> u16;
        fn CreateWindowExW(
            ex_style: u32,
            class: *const u16,
            name: *const u16,
            style: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            parent: isize,
            menu: isize,
            instance: isize,
            param: isize,
        ) -> isize;
        fn DefWindowProcW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn DestroyWindow(hwnd: isize) -> i32;
        fn PostQuitMessage(code: i32);
        fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn CreatePopupMenu() -> isize;
        fn AppendMenuW(menu: isize, flags: u32, id: usize, text: *const u16) -> i32;
        fn DestroyMenu(menu: isize) -> i32;
        fn TrackPopupMenu(menu: isize, flags: u32, x: i32, y: i32, reserved: i32, hwnd: isize, rect: isize) -> i32;
        fn SetForegroundWindow(hwnd: isize) -> i32;
        fn GetCursorPos(pt: *mut Point) -> i32;
        fn LoadImageW(instance: isize, name: *const u16, kind: u32, cx: i32, cy: i32, flags: u32) -> isize;
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn Shell_NotifyIconW(msg: u32, data: *const NotifyIconDataW) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn send(event: TrayEvent) {
        if let Some(tx) = TX.get() {
            let _ = tx.unbounded_send(event);
        }
    }

    unsafe fn remove_icon(hwnd: isize) {
        let mut nid: NotifyIconDataW = unsafe { std::mem::zeroed() };
        nid.cb_size = std::mem::size_of::<NotifyIconDataW>() as u32;
        nid.hwnd = hwnd;
        nid.uid = 1;
        unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
    }

    unsafe fn show_menu(hwnd: isize) {
        let menu = unsafe { CreatePopupMenu() };
        if menu == 0 {
            return;
        }
        let open = wide("Open Stream Tube");
        let restart = wide("Restart");
        let exit = wide("Exit");
        unsafe {
            AppendMenuW(menu, MF_STRING, ID_SHOW, open.as_ptr());
            AppendMenuW(menu, MF_STRING, ID_RESTART, restart.as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(menu, MF_STRING, ID_EXIT, exit.as_ptr());
            let mut pt = Point { x: 0, y: 0 };
            GetCursorPos(&mut pt);
            // without this the menu stays up after a click elsewhere
            SetForegroundWindow(hwnd);
            TrackPopupMenu(
                menu,
                TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
                pt.x,
                pt.y,
                0,
                hwnd,
                0,
            );
            PostMessageW(hwnd, WM_NULL, 0, 0);
            DestroyMenu(menu);
        }
    }

    unsafe extern "system" fn wnd_proc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize {
        match msg {
            WM_TRAY => {
                let event = (lparam as u32) & 0xFFFF;
                if event == WM_LBUTTONUP || event == WM_LBUTTONDBLCLK {
                    send(TrayEvent::Show);
                } else if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                    unsafe { show_menu(hwnd) };
                }
                0
            }
            WM_COMMAND => {
                match wparam & 0xFFFF {
                    ID_SHOW => send(TrayEvent::Show),
                    ID_RESTART => {
                        ACTIVE.store(false, Ordering::SeqCst);
                        unsafe {
                            remove_icon(hwnd);
                            DestroyWindow(hwnd);
                        }
                        send(TrayEvent::Restart);
                    }
                    ID_EXIT => {
                        ACTIVE.store(false, Ordering::SeqCst);
                        unsafe {
                            remove_icon(hwnd);
                            DestroyWindow(hwnd);
                        }
                        send(TrayEvent::Exit);
                    }
                    _ => {}
                }
                0
            }
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    unsafe fn run(accent: u32) {
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let class_name = wide("StreamTubeTray");
        let class = WndClassExW {
            cb_size: std::mem::size_of::<WndClassExW>() as u32,
            style: 0,
            wnd_proc: Some(wnd_proc),
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: 0,
            cursor: 0,
            background: 0,
            menu_name: std::ptr::null(),
            class_name: class_name.as_ptr(),
            icon_small: 0,
        };
        if unsafe { RegisterClassExW(&class) } == 0 {
            return;
        }
        let title = wide("Stream Tube");
        let hwnd = unsafe { CreateWindowExW(0, class_name.as_ptr(), title.as_ptr(), 0, 0, 0, 0, 0, 0, 0, instance, 0) };
        if hwnd == 0 {
            return;
        }
        let icon_id = crate::ui::app_icon::resource_id(accent);
        let icon = unsafe {
            LoadImageW(instance, icon_id as usize as *const u16, IMAGE_ICON, 0, 0, LR_DEFAULTSIZE)
        };
        let mut nid: NotifyIconDataW = unsafe { std::mem::zeroed() };
        nid.cb_size = std::mem::size_of::<NotifyIconDataW>() as u32;
        nid.hwnd = hwnd;
        nid.uid = 1;
        nid.flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.callback_message = WM_TRAY;
        nid.hicon = icon;
        let tip = wide("Stream Tube");
        let n = tip.len().min(nid.tip.len());
        nid.tip[..n].copy_from_slice(&tip[..n]);
        if unsafe { Shell_NotifyIconW(NIM_ADD, &nid) } == 0 {
            unsafe { DestroyWindow(hwnd) };
            return;
        }
        ACTIVE.store(true, Ordering::SeqCst);
        let mut msg: Msg = unsafe { std::mem::zeroed() };
        while unsafe { GetMessageW(&mut msg, 0, 0, 0) } > 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    pub fn start(accent: u32) -> Option<futures::channel::mpsc::UnboundedReceiver<TrayEvent>> {
        let (tx, rx) = unbounded();
        if TX.set(tx).is_err() {
            return None;
        }
        std::thread::Builder::new()
            .name("streamtube-tray".into())
            .spawn(move || unsafe { run(accent) })
            .ok()?;
        Some(rx)
    }
}
