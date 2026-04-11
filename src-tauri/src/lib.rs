mod executor;
mod models;
mod search;
mod storage;

use models::{AppConfig, ExportData, FilterResult, ParamType, ShortCutItem};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::io::Write;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, WebviewWindow,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

struct AppState {
    items: Vec<ShortCutItem>,
    config: AppConfig,
    next_id: usize,
}

static WINDOW_VISIBLE: AtomicBool = AtomicBool::new(true);

fn log_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("altrun_debug.log")))
        .unwrap_or_else(|| std::path::PathBuf::from("altrun_debug.log"))
}

fn log(msg: &str) {
    let path = log_path();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let d = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let _ = writeln!(f, "[{}.{:03}] {}", d.as_secs(), d.subsec_millis(), msg);
    }
}

// ===== Single Instance =====

#[cfg(target_os = "windows")]
mod single_instance {
    pub struct Guard { handle: *mut std::ffi::c_void }
    unsafe impl Send for Guard {}
    unsafe impl Sync for Guard {}
    impl Guard {
        pub fn acquire() -> Option<Self> {
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;
            let name = "ALTRun_SingleInstance_Mutex_v2";
            let wide: Vec<u16> = OsStr::new(name).encode_wide().chain(std::iter::once(0)).collect();
            extern "system" {
                fn CreateMutexW(a: *mut std::ffi::c_void, b: i32, c: *const u16) -> *mut std::ffi::c_void;
                fn GetLastError() -> u32;
            }
            let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 1, wide.as_ptr()) };
            if handle.is_null() { return None; }
            if unsafe { GetLastError() } == 183 {
                extern "system" { fn CloseHandle(h: *mut std::ffi::c_void) -> i32; }
                unsafe { CloseHandle(handle); }
                return None;
            }
            Some(Guard { handle })
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            extern "system" { fn CloseHandle(h: *mut std::ffi::c_void) -> i32; }
            unsafe { CloseHandle(self.handle); }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod single_instance {
    use std::fs::OpenOptions;
    use std::io::Write;

    pub struct Guard { _file: std::fs::File }

    impl Guard {
        pub fn acquire() -> Option<Self> {
            let lock_path = std::env::temp_dir().join("altrun.lock");
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .open(&lock_path)
                .ok()?;

            // Try to get an exclusive lock (non-blocking)
            #[cfg(unix)]
            {
                use std::os::unix::io::AsRawFd;
                let fd = file.as_raw_fd();
                let ret = unsafe {
                    libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB)
                };
                if ret != 0 {
                    return None; // Another instance holds the lock
                }
            }

            Some(Guard { _file: file })
        }
    }
}

// ===== Auto-run =====

#[cfg(target_os = "windows")]
mod autorun {
    const AUTORUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const AUTORUN_NAME: &str = "ALTRun";

    pub fn get() -> Option<String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        fn to_wide(s: &str) -> Vec<u16> {
            OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
        }
        extern "system" {
            fn RegOpenKeyExW(hkey: isize, sub: *const u16, opts: u32, sam: u32, out: *mut isize) -> i32;
            fn RegQueryValueExW(hkey: isize, name: *const u16, res: *mut u32, typ: *mut u32, data: *mut u8, len: *mut u32) -> i32;
            fn RegCloseKey(hkey: isize) -> i32;
        }
        const HKCU: isize = -2147483647;
        let mut hkey: isize = 0;
        unsafe {
            if RegOpenKeyExW(HKCU, to_wide(AUTORUN_KEY).as_ptr(), 0, 0x20019, &mut hkey) != 0 { return None; }
            let mut typ: u32 = 0; let mut len: u32 = 512; let mut buf = vec![0u8; 512];
            let ret = RegQueryValueExW(hkey, to_wide(AUTORUN_NAME).as_ptr(), std::ptr::null_mut(), &mut typ, buf.as_mut_ptr(), &mut len);
            RegCloseKey(hkey);
            if ret != 0 || typ != 1 { return None; }
            let words: Vec<u16> = buf[..len as usize].chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
            Some(String::from_utf16_lossy(&words).to_string())
        }
    }

    pub fn set(enable: bool) -> Result<(), String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        fn to_wide(s: &str) -> Vec<u16> {
            OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
        }
        extern "system" {
            fn RegOpenKeyExW(hkey: isize, sub: *const u16, opts: u32, sam: u32, out: *mut isize) -> i32;
            fn RegSetValueExW(hkey: isize, name: *const u16, res: u32, typ: u32, data: *const u8, len: u32) -> i32;
            fn RegDeleteValueW(hkey: isize, name: *const u16) -> i32;
            fn RegCloseKey(hkey: isize) -> i32;
        }
        const HKCU: isize = -2147483647;
        let mut hkey: isize = 0;
        unsafe {
            if RegOpenKeyExW(HKCU, to_wide(AUTORUN_KEY).as_ptr(), 0, 0x0002, &mut hkey) != 0 {
                return Err("Failed to open registry key".into());
            }
            let ret = if enable {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?.to_string_lossy().to_string();
                let wide = to_wide(&exe);
                let bytes: Vec<u8> = wide.iter().flat_map(|w| w.to_le_bytes()).collect();
                RegSetValueExW(hkey, to_wide(AUTORUN_NAME).as_ptr(), 0, 1, bytes.as_ptr(), bytes.len() as u32)
            } else {
                RegDeleteValueW(hkey, to_wide(AUTORUN_NAME).as_ptr())
            };
            RegCloseKey(hkey);
            if ret != 0 && enable { Err(format!("Registry op failed: {}", ret)) } else { Ok(()) }
        }
    }
}

#[cfg(target_os = "macos")]
mod autorun {
    use std::path::PathBuf;

    fn plist_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join("Library/LaunchAgents/com.altrun.app.plist")
    }

    pub fn get() -> Option<String> {
        if plist_path().exists() { Some("enabled".into()) } else { None }
    }

    pub fn set(enable: bool) -> Result<(), String> {
        let path = plist_path();
        if enable {
            let exe = std::env::current_exe()
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .to_string();
            let plist = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.altrun.app</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>"#, exe);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, plist).map_err(|e| e.to_string())
        } else {
            if path.exists() {
                std::fs::remove_file(&path).map_err(|e| e.to_string())
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod autorun {
    // Linux: use ~/.config/autostart
    use std::path::PathBuf;

    fn desktop_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("autostart/altrun.desktop")
    }

    pub fn get() -> Option<String> {
        if desktop_path().exists() { Some("enabled".into()) } else { None }
    }

    pub fn set(enable: bool) -> Result<(), String> {
        let path = desktop_path();
        if enable {
            let exe = std::env::current_exe()
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .to_string();
            let desktop = format!(
                "[Desktop Entry]\nType=Application\nName=ALTRun\nExec={}\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n",
                exe
            );
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, desktop).map_err(|e| e.to_string())
        } else {
            if path.exists() {
                std::fs::remove_file(&path).map_err(|e| e.to_string())
            } else {
                Ok(())
            }
        }
    }
}

// ===== Hotkey normalization =====

fn normalize_hotkey(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() { return String::new(); }
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    parts.iter().map(|p| {
        let lower = p.to_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => "Ctrl".into(),
            "alt" => "Alt".into(),
            "shift" => "Shift".into(),
            "win" | "super" | "meta" | "windows" | "cmd" | "command" => "Super".into(),
            "space" => "Space".into(),
            "enter" | "return" => "Enter".into(),
            "escape" | "esc" => "Escape".into(),
            "pause" => "Pause".into(),
            "tab" => "Tab".into(),
            "backspace" => "Backspace".into(),
            "delete" | "del" => "Delete".into(),
            "insert" | "ins" => "Insert".into(),
            s if s.len() == 1 && s.chars().next().unwrap().is_alphabetic() => s.to_uppercase(),
            s if s.starts_with('f') && s[1..].parse::<u32>().is_ok() => format!("F{}", &s[1..]),
            _ => { let mut c = p.chars(); match c.next() { None => String::new(), Some(f) => f.to_uppercase().to_string() + c.as_str() } }
        }
    }).collect::<Vec<_>>().join("+")
}

impl AppState {
    fn save_items(&self) {
        storage::save_shortcut_list(&storage::shortcut_list_path(), &self.items);
    }
}

type State = Mutex<AppState>;

fn do_show(win: &WebviewWindow, app: &tauri::AppHandle) {
    let _ = win.show();
    let _ = win.unminimize();
    let _ = win.set_always_on_top(true);
    let _ = win.set_focus();
    WINDOW_VISIBLE.store(true, Ordering::SeqCst);
    let _ = app.emit("show-window", ());
}

fn do_hide(win: &WebviewWindow) {
    let _ = win.hide();
    WINDOW_VISIBLE.store(false, Ordering::SeqCst);
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") { do_show(&win, app); }
}

fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") { do_hide(&win); }
}

fn toggle_main_window(app: &tauri::AppHandle) {
    let vis = WINDOW_VISIBLE.load(Ordering::SeqCst);
    if vis { hide_main_window(app); } else { show_main_window(app); }
}

// ===== Tauri Commands =====

#[tauri::command]
fn filter_keyword(keyword: String, state: tauri::State<'_, State>) -> FilterResult {
    let st = state.lock().unwrap();
    let max = if st.config.show_top_ten { 10 } else { 100 };
    let results = search::filter_items(&st.items, &keyword, st.config.enable_regex, st.config.match_anywhere, max);
    FilterResult { total: results.len(), items: results }
}

#[tauri::command]
fn execute_item(id: usize, keyword: String, param: String, state: tauri::State<'_, State>, app: tauri::AppHandle) -> Result<(), String> {
    log(&format!("execute id={} kw='{}' param='{}'", id, keyword, param));
    hide_main_window(&app);
    let mut st = state.lock().unwrap();
    if let Some(item) = st.items.iter_mut().find(|i| i.id == id) {
        item.freq += 1;
        let clone = item.clone();
        storage::save_shortcut_list(&storage::shortcut_list_path(), &st.items);
        drop(st);
        executor::execute_shortcut(&clone, &keyword, &param)
    } else { Err("Item not found".into()) }
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) { hide_main_window(&app); }

#[tauri::command]
fn open_item_dir(id: usize, state: tauri::State<'_, State>) -> Result<(), String> {
    let st = state.lock().unwrap();
    if let Some(item) = st.items.iter().find(|i| i.id == id) { executor::open_directory(item) }
    else { Err("Item not found".into()) }
}

#[tauri::command]
fn get_all_items(state: tauri::State<'_, State>) -> Vec<ShortCutItem> { state.lock().unwrap().items.clone() }

#[tauri::command]
fn add_item(shortcut: String, name: String, command_line: String, param_type: ParamType, state: tauri::State<'_, State>) -> Result<ShortCutItem, String> {
    let mut st = state.lock().unwrap();
    let id = st.next_id; st.next_id += 1;
    let item = ShortCutItem { id, shortcut, name, command_line, param_type, freq: 0, rank: 0 };
    st.items.push(item.clone()); st.save_items(); Ok(item)
}

#[tauri::command]
fn update_item(id: usize, shortcut: String, name: String, command_line: String, param_type: ParamType, state: tauri::State<'_, State>) -> Result<(), String> {
    let mut st = state.lock().unwrap();
    if let Some(item) = st.items.iter_mut().find(|i| i.id == id) {
        item.shortcut = shortcut; item.name = name; item.command_line = command_line; item.param_type = param_type;
        st.save_items(); Ok(())
    } else { Err("Item not found".into()) }
}

#[tauri::command]
fn delete_item(id: usize, state: tauri::State<'_, State>) -> Result<(), String> {
    let mut st = state.lock().unwrap(); st.items.retain(|i| i.id != id); st.save_items(); Ok(())
}

#[tauri::command]
fn get_config(state: tauri::State<'_, State>) -> AppConfig {
    let mut cfg = state.lock().unwrap().config.clone();
    cfg.auto_run = autorun::get().is_some();
    cfg
}

#[tauri::command]
fn save_config(config: AppConfig, state: tauri::State<'_, State>) -> Result<(), String> {
    let mut st = state.lock().unwrap();
    if let Err(e) = autorun::set(config.auto_run) {
        log(&format!("set_autorun({}) failed: {}", config.auto_run, e));
    } else {
        log(&format!("set_autorun({}) OK", config.auto_run));
    }
    st.config = config.clone();
    storage::save_config(&storage::config_path(), &config);
    Ok(())
}

#[tauri::command]
fn export_data(path: String, state: tauri::State<'_, State>) -> Result<(), String> {
    let st = state.lock().unwrap();
    let data = ExportData { version: "2.0.0".into(), config: st.config.clone(), items: st.items.clone() };
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("Failed to write {}: {}", path, e))
}

#[tauri::command]
fn import_data(path: String, state: tauri::State<'_, State>) -> Result<String, String> {
    let content = std::fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {}", path, e))?;
    let data: ExportData = serde_json::from_str(&content).map_err(|e| format!("Invalid file: {}", e))?;
    let mut st = state.lock().unwrap();
    st.config = data.config.clone();
    storage::save_config(&storage::config_path(), &st.config);
    let mut added = 0usize;
    for imp_item in &data.items {
        let exists = st.items.iter().any(|i| i.shortcut == imp_item.shortcut && i.command_line == imp_item.command_line);
        if !exists {
            let id = st.next_id; st.next_id += 1;
            let mut new_item = imp_item.clone(); new_item.id = id;
            st.items.push(new_item); added += 1;
        }
    }
    st.save_items();
    let msg = format!("Imported: config updated, {} new shortcuts added ({} skipped as duplicates)", added, data.items.len() - added);
    log(&msg); Ok(msg)
}

// ===== App Setup =====

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Single instance check
    let _guard = match single_instance::Guard::acquire() {
        Some(g) => g,
        None => std::process::exit(0),
    };

    let _ = std::fs::write(log_path(), "");
    log("=== ALTRun starting ===");

    let config = storage::load_config(&storage::config_path());
    log(&format!("hotkey1='{}' hotkey2='{}'", config.hotkey1, config.hotkey2));

    let items = storage::load_shortcut_list(&storage::shortcut_list_path());
    let next_id = items.iter().map(|i| i.id).max().unwrap_or(0) + 1;
    log(&format!("{} items loaded", items.len()));

    let hk1 = config.hotkey1.clone();
    let hk2 = config.hotkey2.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(AppState { items, config, next_id }))
        .invoke_handler(tauri::generate_handler![
            filter_keyword, execute_item, hide_window, open_item_dir,
            get_all_items, add_item, update_item, delete_item,
            get_config, save_config, export_data, import_data,
        ])
        .setup(move |app| {
            let show_i = MenuItem::with_id(app, "show", "Show ALTRun", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("ALTRun")
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => show_main_window(app),
                        "quit" => app.exit(0),
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        toggle_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            // Register hotkeys
            let mut shortcuts: Vec<Shortcut> = Vec::new();
            let norm1 = normalize_hotkey(&hk1);
            if !norm1.is_empty() {
                match norm1.parse::<Shortcut>() {
                    Ok(sc) => { log(&format!("parsed hk1: {}", norm1)); shortcuts.push(sc); }
                    Err(e) => log(&format!("parse hk1 '{}' FAILED: {:?}", norm1, e)),
                }
            }
            let norm2 = normalize_hotkey(&hk2);
            if !norm2.is_empty() && norm2 != norm1 {
                match norm2.parse::<Shortcut>() {
                    Ok(sc) => { log(&format!("parsed hk2: {}", norm2)); shortcuts.push(sc); }
                    Err(e) => log(&format!("parse hk2 '{}' FAILED: {:?}", norm2, e)),
                }
            }

            if !shortcuts.is_empty() {
                let app_handle = app.handle().clone();
                match app.global_shortcut().on_shortcuts(shortcuts, move |_app, shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        log(&format!("HOTKEY: {:?}", shortcut));
                        toggle_main_window(&app_handle);
                    }
                }) {
                    Ok(_) => log("hotkeys registered OK"),
                    Err(e) => log(&format!("hotkeys FAILED: {:?}", e)),
                }
            }

            log("setup done");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
