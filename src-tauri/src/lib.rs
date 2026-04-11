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

// ===== Auto-run registry helpers =====

const AUTORUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const AUTORUN_NAME: &str = "ALTRun";

/// Read current exe path from registry autorun entry.
/// Returns Some(path) if the entry exists, None otherwise.
fn get_autorun() -> Option<String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    extern "system" {
        fn RegOpenKeyExW(
            hkey: isize, lp_sub_key: *const u16, ul_options: u32,
            sam_desired: u32, phk_result: *mut isize,
        ) -> i32;
        fn RegQueryValueExW(
            hkey: isize, lp_value_name: *const u16, lp_reserved: *mut u32,
            lp_type: *mut u32, lp_data: *mut u8, lpcb_data: *mut u32,
        ) -> i32;
        fn RegCloseKey(hkey: isize) -> i32;
    }

    const HKCU: isize = -2147483647; // 0x80000001
    const KEY_READ: u32 = 0x20019;
    const REG_SZ: u32 = 1;

    let key_wide = to_wide(AUTORUN_KEY);
    let name_wide = to_wide(AUTORUN_NAME);
    let mut hkey: isize = 0;

    unsafe {
        if RegOpenKeyExW(HKCU, key_wide.as_ptr(), 0, KEY_READ, &mut hkey) != 0 {
            return None;
        }
        let mut data_type: u32 = 0;
        let mut data_len: u32 = 512;
        let mut buf = vec![0u8; 512];
        let ret = RegQueryValueExW(
            hkey, name_wide.as_ptr(), std::ptr::null_mut(),
            &mut data_type, buf.as_mut_ptr(), &mut data_len,
        );
        RegCloseKey(hkey);
        if ret != 0 || data_type != REG_SZ { return None; }
        // Convert UTF-16 LE bytes to String
        let words: Vec<u16> = buf[..data_len as usize]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&c| c != 0)
            .collect();
        Some(String::from_utf16_lossy(&words).to_string())
    }
}

/// Set or remove the autorun registry entry.
fn set_autorun(enable: bool) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    extern "system" {
        fn RegOpenKeyExW(
            hkey: isize, lp_sub_key: *const u16, ul_options: u32,
            sam_desired: u32, phk_result: *mut isize,
        ) -> i32;
        fn RegSetValueExW(
            hkey: isize, lp_value_name: *const u16, reserved: u32,
            dw_type: u32, lp_data: *const u8, cb_data: u32,
        ) -> i32;
        fn RegDeleteValueW(hkey: isize, lp_value_name: *const u16) -> i32;
        fn RegCloseKey(hkey: isize) -> i32;
    }

    const HKCU: isize = -2147483647;
    const KEY_SET_VALUE: u32 = 0x0002;
    const REG_SZ: u32 = 1;

    let key_wide = to_wide(AUTORUN_KEY);
    let name_wide = to_wide(AUTORUN_NAME);
    let mut hkey: isize = 0;

    unsafe {
        if RegOpenKeyExW(HKCU, key_wide.as_ptr(), 0, KEY_SET_VALUE, &mut hkey) != 0 {
            return Err("Failed to open registry key".into());
        }

        let ret = if enable {
            let exe = std::env::current_exe()
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .to_string();
            let value_wide = to_wide(&exe);
            let bytes: Vec<u8> = value_wide.iter()
                .flat_map(|w| w.to_le_bytes())
                .collect();
            RegSetValueExW(
                hkey, name_wide.as_ptr(), 0, REG_SZ,
                bytes.as_ptr(), bytes.len() as u32,
            )
        } else {
            RegDeleteValueW(hkey, name_wide.as_ptr())
        };

        RegCloseKey(hkey);

        if ret != 0 && enable {
            Err(format!("Registry operation failed: {}", ret))
        } else {
            Ok(())
        }
    }
}

// ===== Single instance =====

#[cfg(target_os = "windows")]
fn try_single_instance() -> Option<windows_single_instance::SingleInstanceGuard> {
    windows_single_instance::SingleInstanceGuard::new("ALTRun_SingleInstance_Mutex_v2")
}

#[cfg(target_os = "windows")]
mod windows_single_instance {
    pub struct SingleInstanceGuard { handle: *mut std::ffi::c_void }
    unsafe impl Send for SingleInstanceGuard {}
    unsafe impl Sync for SingleInstanceGuard {}
    impl SingleInstanceGuard {
        pub fn new(name: &str) -> Option<Self> {
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;
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
            Some(SingleInstanceGuard { handle })
        }
    }
    impl Drop for SingleInstanceGuard {
        fn drop(&mut self) {
            extern "system" { fn CloseHandle(h: *mut std::ffi::c_void) -> i32; }
            unsafe { CloseHandle(self.handle); }
        }
    }
}

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
            "win" | "super" | "meta" | "windows" => "Super".into(),
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
    // Sync auto_run from actual registry state
    cfg.auto_run = get_autorun().is_some();
    cfg
}

#[tauri::command]
fn save_config(config: AppConfig, state: tauri::State<'_, State>) -> Result<(), String> {
    let mut st = state.lock().unwrap();
    // Apply auto-run registry change
    if let Err(e) = set_autorun(config.auto_run) {
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
            let mut new_item = imp_item.clone();
            new_item.id = id;
            st.items.push(new_item);
            added += 1;
        }
    }
    st.save_items();
    let msg = format!("Imported: config updated, {} new shortcuts added ({} skipped as duplicates)", added, data.items.len() - added);
    log(&msg);
    Ok(msg)
}

// ===== App Setup =====

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    let _single_instance_guard = match try_single_instance() {
        Some(guard) => guard,
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
            get_config, save_config,
            export_data, import_data,
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
