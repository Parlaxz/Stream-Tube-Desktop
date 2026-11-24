//! The app icon follows the accent colour: the logo inside the app, the window and
//! taskbar icon, the desktop / Start menu shortcuts (Windows) and the menu icon (Linux).
//!
//! The icons are made by `tools/accent_icons.py` (assets/accent/) and, on Windows,
//! embedded as icon resources 101.. (see assets/streamtube.rc).
use gpui_kit::*;

/// Accent colours with a ready-made icon, in resource order (101, 102, ...).
pub const ACCENTS: [u32; 7] = [0xFF0033, 0xF0307E, 0xAB47BC, 0x3EA6FF, 0x00BFA5, 0x2BA640, 0xFFA000];
/// The brand colour: its icon is the original logo.
pub const BRAND: u32 = 0xF0307E;

/// A ReVanced custom branding icon, from `anddea/revanced-patches`
/// (`patches/src/main/resources/youtube/branding/<key>/launcher`).
pub struct AppIcon {
    pub key: &'static str,
    pub label: &'static str,
    pub png: &'static [u8],
}

pub const ICONS: &[AppIcon] = &[
    AppIcon { key: "afn_blue", label: "AFN Blue", png: include_bytes!("../../assets/app-icons/afn_blue.png") },
    AppIcon { key: "afn_red", label: "AFN Red", png: include_bytes!("../../assets/app-icons/afn_red.png") },
    AppIcon { key: "mmt", label: "MMT", png: include_bytes!("../../assets/app-icons/mmt.png") },
    AppIcon { key: "mmt_blue", label: "MMT Blue", png: include_bytes!("../../assets/app-icons/mmt_blue.png") },
    AppIcon { key: "mmt_green", label: "MMT Green", png: include_bytes!("../../assets/app-icons/mmt_green.png") },
    AppIcon { key: "mmt_orange", label: "MMT Orange", png: include_bytes!("../../assets/app-icons/mmt_orange.png") },
    AppIcon { key: "mmt_pink", label: "MMT Pink", png: include_bytes!("../../assets/app-icons/mmt_pink.png") },
    AppIcon { key: "mmt_turquoise", label: "MMT Turquoise", png: include_bytes!("../../assets/app-icons/mmt_turquoise.png") },
    AppIcon { key: "mmt_yellow", label: "MMT Yellow", png: include_bytes!("../../assets/app-icons/mmt_yellow.png") },
    AppIcon { key: "revancify_blue", label: "Revancify Blue", png: include_bytes!("../../assets/app-icons/revancify_blue.png") },
    AppIcon { key: "revancify_red", label: "Revancify Red", png: include_bytes!("../../assets/app-icons/revancify_red.png") },
    AppIcon { key: "squid_game", label: "Squid Game", png: include_bytes!("../../assets/app-icons/squid_game.png") },
    AppIcon { key: "vanced_black", label: "Vanced Black", png: include_bytes!("../../assets/app-icons/vanced_black.png") },
    AppIcon { key: "vanced_light", label: "Vanced Light", png: include_bytes!("../../assets/app-icons/vanced_light.png") },
    AppIcon { key: "xisr_aurora", label: "Xisr Aurora", png: include_bytes!("../../assets/app-icons/xisr_aurora.png") },
    AppIcon { key: "xisr_evergreen", label: "Xisr Evergreen", png: include_bytes!("../../assets/app-icons/xisr_evergreen.png") },
    AppIcon { key: "xisr_special", label: "Xisr Special", png: include_bytes!("../../assets/app-icons/xisr_special.png") },
    AppIcon { key: "xisr_white", label: "Xisr White", png: include_bytes!("../../assets/app-icons/xisr_white.png") },
    AppIcon { key: "xisr_winter", label: "Xisr Winter", png: include_bytes!("../../assets/app-icons/xisr_winter.png") },
    AppIcon { key: "xisr_yellow", label: "Xisr Yellow", png: include_bytes!("../../assets/app-icons/xisr_yellow.png") },
    AppIcon { key: "youtube_black", label: "YouTube Black", png: include_bytes!("../../assets/app-icons/youtube_black.png") },
];

/// The chosen icon, when its stored key is one we ship.
pub fn chosen(key: &str) -> Option<&'static AppIcon> {
    ICONS.iter().find(|i| i.key == key)
}

/// In-app asset path for a chosen icon (served by `assets::Assets`).
pub fn asset_path(key: &str) -> Option<String> {
    chosen(key).map(|i| format!("brand/appicon/{}.png", i.key))
}

/// What a window/taskbar icon comes from: an embedded accent resource or a
/// custom branding icon written to an .ico file.
#[cfg(windows)]
enum IconSource {
    Resource(u16),
    File(std::path::PathBuf),
}

/// The closest accent that has an icon.
pub fn icon_accent(accent: u32) -> u32 {
    if ACCENTS.contains(&accent) {
        return accent;
    }
    let d = |a: u32, b: u32| {
        let c = |s: u32| ((a >> s) & 255) as i32 - ((b >> s) & 255) as i32;
        c(16).pow(2) + c(8).pow(2) + c(0).pow(2)
    };
    *ACCENTS.iter().min_by_key(|a| d(**a, accent)).unwrap_or(&BRAND)
}

fn hls(c: u32) -> (f64, f64, f64) {
    let (r, g, b) = (((c >> 16) & 255) as f64 / 255., ((c >> 8) & 255) as f64 / 255., (c & 255) as f64 / 255.);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.;
    if mx == mn {
        return (0., l, 0.);
    }
    let d = mx - mn;
    let s = if l <= 0.5 { d / (mx + mn) } else { d / (2. - mx - mn) };
    let h = if mx == r {
        (g - b) / d + if g < b { 6. } else { 0. }
    } else if mx == g {
        (b - r) / d + 2.
    } else {
        (r - g) / d + 4.
    } / 6.;
    (h, l, s)
}

fn from_hls(h: f64, l: f64, s: f64) -> u32 {
    let f = |p: f64, q: f64, mut t: f64| {
        t = t.rem_euclid(1.0);
        if t < 1. / 6. {
            p + (q - p) * 6. * t
        } else if t < 0.5 {
            q
        } else if t < 2. / 3. {
            p + (q - p) * (2. / 3. - t) * 6.
        } else {
            p
        }
    };
    let (r, g, b) = if s == 0. {
        (l, l, l)
    } else {
        let q = if l < 0.5 { l * (1. + s) } else { l + s - l * s };
        let p = 2. * l - q;
        (f(p, q, h + 1. / 3.), f(p, q, h), f(p, q, h - 1. / 3.))
    };
    let c = |x: f64| (x.clamp(0., 1.) * 255.).round() as u32;
    (c(r) << 16) | (c(g) << 8) | c(b)
}

fn shift(c: u32, dh: f64, sk: f64, lk: f64) -> u32 {
    let (h, l, s) = hls(c);
    from_hls((h + dh / 360.).rem_euclid(1.), (l * lk).clamp(0., 1.), (s * sk).clamp(0., 1.))
}

/// The logo's three gradient stops for an accent (same math as the icon generator).
pub fn accent_stops(accent: u32) -> [u32; 3] {
    if accent == BRAND {
        return [0xFF5A5F, 0xF0307E, 0x7A3CF5];
    }
    [shift(accent, -12., 1.0, 1.18), accent, shift(accent, 32., 0.95, 0.72)]
}

/// The logo SVG recoloured for `accent` (served as `brand/logo-<rrggbb>.svg`).
pub fn logo_svg(accent: u32) -> String {
    let base = include_str!("../../assets/logo.svg");
    let [a, b, c] = accent_stops(accent);
    base.replace("#FF5A5F", &format!("#{a:06X}")).replace("#F0307E", &format!("#{b:06X}")).replace("#7A3CF5", &format!("#{c:06X}"))
}

/// Applies the chosen icon (or the accent icon) everywhere outside the UI.
pub fn apply(cx: &mut App) {
    let s = crate::store::settings_snapshot();
    let accent = icon_accent(s.accent);
    let custom = chosen(&s.app_icon);
    #[cfg(windows)]
    {
        let source = match custom.and_then(ico_file) {
            Some(path) => IconSource::File(path),
            None => IconSource::Resource(resource_id(accent)),
        };
        for w in cx.windows() {
            let _ = w.update(cx, |_, window, _| set_window_icon(window, &source));
        }
        update_shortcuts(&source);
    }
    #[cfg(target_os = "linux")]
    {
        let _ = cx;
        update_menu_icon(accent, custom);
    }
}

#[cfg(windows)]
pub fn resource_id(accent: u32) -> u16 {
    ACCENTS.iter().position(|a| *a == accent).map(|i| 101 + i as u16).unwrap_or(1)
}

/// Writes the icon as a single-image .ico (a PNG-compressed icon, Vista+) so
/// Windows can load it from a file. Returns the path on success.
#[cfg(windows)]
fn ico_file(icon: &AppIcon) -> Option<std::path::PathBuf> {
    let dir = crate::store::data_dir().join("icons");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{}.ico", icon.key));
    let (w, h) = png_size(icon.png).unwrap_or((192, 192));
    let mut ico = Vec::with_capacity(icon.png.len() + 22);
    ico.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    ico.push(if w >= 256 { 0 } else { w as u8 });
    ico.push(if h >= 256 { 0 } else { h as u8 });
    ico.extend_from_slice(&[0, 0, 1, 0, 32, 0]);
    ico.extend_from_slice(&(icon.png.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(icon.png);
    if std::fs::read(&path).ok().as_deref() != Some(ico.as_slice()) {
        std::fs::write(&path, &ico).ok()?;
    }
    Some(path)
}

/// Width and height from a PNG's IHDR chunk.
#[cfg(windows)]
fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    let hdr = png.get(16..24)?;
    let w = u32::from_be_bytes(hdr[..4].try_into().ok()?);
    let h = u32::from_be_bytes(hdr[4..].try_into().ok()?);
    Some((w, h))
}

#[cfg(windows)]
fn set_window_icon(window: &Window, source: &IconSource) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    #[link(name = "user32")]
    unsafe extern "system" {
        fn LoadImageW(hinst: isize, name: *const u16, kind: u32, cx: i32, cy: i32, flags: u32) -> isize;
        fn SendMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn GetSystemMetrics(i: i32) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
    }
    const IMAGE_ICON: u32 = 1;
    const WM_SETICON: u32 = 0x0080;
    const SM_CXICON: i32 = 11;
    const SM_CXSMICON: i32 = 49;
    const LR_LOADFROMFILE: u32 = 0x10;
    let Ok(h) = HasWindowHandle::window_handle(window) else { return };
    let RawWindowHandle::Win32(w) = h.as_raw() else { return };
    let inst = unsafe { GetModuleHandleW(std::ptr::null()) };
    let load = |size: i32| unsafe {
        match source {
            IconSource::Resource(id) => LoadImageW(inst, *id as usize as *const u16, IMAGE_ICON, size, size, 0),
            IconSource::File(path) => {
                let mut wide: Vec<u16> = std::os::windows::ffi::OsStrExt::encode_wide(path.as_os_str()).collect();
                wide.push(0);
                LoadImageW(inst, wide.as_ptr(), IMAGE_ICON, size, size, LR_LOADFROMFILE)
            }
        }
    };
    let big_size = unsafe { GetSystemMetrics(SM_CXICON) }.max(32);
    let small_size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    let big = load(big_size);
    let small = load(small_size);
    if big != 0 {
        unsafe { SendMessageW(w.hwnd.get(), WM_SETICON, 1, big) };
    }
    if small != 0 {
        unsafe { SendMessageW(w.hwnd.get(), WM_SETICON, 0, small) };
    }
}

/// Points the app's shortcuts (desktop, Start menu, taskbar pin) at the matching icon.
#[cfg(windows)]
fn update_shortcuts(source: &IconSource) {
    let Ok(exe) = std::env::current_exe() else { return };
    // an update reinstalls the shortcuts with the default icon, so the version and
    // the exe's timestamp are part of what "already done" means
    let built = std::fs::metadata(&exe).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    let exe = exe.display().to_string();
    let location = match source {
        IconSource::Resource(id) => format!("{exe},-{id}"),
        IconSource::File(path) => path.display().to_string(),
    };
    let key = format!("{exe}|{location}|{}|{built}", env!("CARGO_PKG_VERSION"));
    if crate::store::kv_get("shortcut_icon").as_deref() == Some(key.as_str()) {
        return;
    }
    std::thread::spawn(move || {
        // .lnk files are COM objects; PowerShell's WScript.Shell edits them simply
        let script = format!(
            r#"$exe = '{exe}'
$shell = New-Object -ComObject WScript.Shell
$dirs = @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'), (Join-Path $env:APPDATA 'Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar'))
foreach ($d in $dirs) {{
  if (-not (Test-Path $d)) {{ continue }}
  Get-ChildItem -Path $d -Filter *.lnk -Recurse -Depth 1 -ErrorAction SilentlyContinue | ForEach-Object {{
    $l = $shell.CreateShortcut($_.FullName)
    if ($l.TargetPath -ieq $exe) {{ $l.IconLocation = '{icon}'; $l.Save() }}
  }}
}}"#,
            exe = exe.replace('\'', "''"),
            icon = location.replace('\'', "''"),
        );
        let mut c = std::process::Command::new("powershell");
        c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script]);
        crate::api::streams::hide_window(&mut c);
        if c.output().map(|o| o.status.success()).unwrap_or(false) {
            crate::store::kv_set("shortcut_icon", &key);
            // ask Explorer to redraw icons
            #[link(name = "shell32")]
            unsafe extern "system" {
                fn SHChangeNotify(event: i32, flags: u32, a: *const std::ffi::c_void, b: *const std::ffi::c_void);
            }
            const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
            unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, 0, std::ptr::null(), std::ptr::null()) };
        }
    });
}

/// The user's icon theme folder wins over the system one, so the menu icon can follow the accent.
#[cfg(target_os = "linux")]
fn update_menu_icon(accent: u32, custom: Option<&AppIcon>) {
    let png: &[u8] = match custom {
        Some(icon) => icon.png,
        None => match accent {
            0xFF0033 => include_bytes!("../../assets/accent/ff0033.png"),
            0xAB47BC => include_bytes!("../../assets/accent/ab47bc.png"),
            0x3EA6FF => include_bytes!("../../assets/accent/3ea6ff.png"),
            0x00BFA5 => include_bytes!("../../assets/accent/00bfa5.png"),
            0x2BA640 => include_bytes!("../../assets/accent/2ba640.png"),
            0xFFA000 => include_bytes!("../../assets/accent/ffa000.png"),
            _ => include_bytes!("../../assets/accent/f0307e.png"),
        },
    };
    let Some(base) = dirs::data_dir() else { return };
    let dir = base.join("icons/hicolor/256x256/apps");
    let path = dir.join("streamtube.png");
    if std::fs::read(&path).map(|b| b == png).unwrap_or(false) {
        return;
    }
    let _ = std::fs::create_dir_all(&dir);
    if std::fs::write(&path, png).is_ok() {
        // icon caches look at the folder's modification time
        let _ = std::process::Command::new("touch").arg(base.join("icons/hicolor")).status();
        let _ = std::process::Command::new("gtk-update-icon-cache").args(["-q", "-t", "-f"]).arg(base.join("icons/hicolor")).status();
    }
}

