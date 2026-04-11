use crate::models::{AppConfig, ParamType, ShortCutItem};
use std::fs;
use std::path::{Path, PathBuf};

/// Get the data directory.
/// - Windows: same directory as the exe (portable, compatible with original ALTRun)
/// - macOS:   ~/Library/Application Support/ALTRun/
/// - Linux:   ~/.config/ALTRun/
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                return dir.to_path_buf();
            }
        }
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ALTRun")
    }

    #[cfg(target_os = "macos")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("~"))
                    .join("Library/Application Support")
            })
            .join("ALTRun")
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        dirs::config_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("~"))
                    .join(".config")
            })
            .join("ALTRun")
    }
}

pub fn shortcut_list_path() -> PathBuf {
    let dir = data_dir();
    let _ = fs::create_dir_all(&dir);
    dir.join("ShortCutList.txt")
}

pub fn config_path() -> PathBuf {
    let dir = data_dir();
    let _ = fs::create_dir_all(&dir);
    dir.join("ALTRun.ini")
}

// ===== ShortCutList.txt parsing =====

pub fn load_shortcut_list(path: &Path) -> Vec<ShortCutItem> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            match fs::read(path) {
                Ok(bytes) => String::from_utf8(bytes.clone())
                    .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).to_string()),
                Err(_) => return default_shortcut_list(),
            }
        }
    };

    let mut items = Vec::new();
    let mut id = 1usize;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Some(item) = parse_shortcut_line(line, id) {
            items.push(item);
            id += 1;
        }
    }

    if items.is_empty() { return default_shortcut_list(); }
    items
}

fn parse_shortcut_line(line: &str, id: usize) -> Option<ShortCutItem> {
    let parts: Vec<&str> = line.splitn(5, '|').collect();

    if parts.len() >= 3 {
        let mut offset = 0;
        let mut freq = 0i32;

        let first = parts[0].trim();
        if first.starts_with('F') || first.starts_with('f') {
            if let Ok(f) = first[1..].trim().parse::<i32>() {
                freq = f;
                offset = 1;
            }
        }

        let mut param_type = ParamType::None;
        if offset < parts.len() {
            let pt = ParamType::from_str(parts[offset].trim());
            if pt != ParamType::None || parts[offset].trim().is_empty() {
                param_type = pt;
                offset += 1;
            }
        }

        if offset + 2 < parts.len() {
            let shortcut = parts[offset].trim().to_string();
            let name = parts[offset + 1].trim().to_string();
            let cmd = parts[offset + 2..].join("|").trim().to_string();

            if !shortcut.is_empty() || !cmd.is_empty() {
                return Some(ShortCutItem {
                    id, shortcut, name, command_line: cmd, param_type, freq, rank: 0,
                });
            }
        }
    }

    // Old comma-delimited format
    let parts: Vec<&str> = line.splitn(3, ',').collect();
    if parts.len() >= 3 {
        let shortcut = parts[0].trim().to_string();
        let name = parts[1].trim().to_string();
        let cmd = parts[2].trim().to_string();
        if !shortcut.is_empty() {
            return Some(ShortCutItem {
                id, shortcut, name, command_line: cmd, param_type: ParamType::None, freq: 0, rank: 0,
            });
        }
    }

    None
}

pub fn save_shortcut_list(path: &Path, items: &[ShortCutItem]) {
    let mut lines = Vec::new();
    for item in items {
        let line = format!(
            "F{:<8}|{:<20}|{:<30}|{:<30}|{}",
            item.freq, item.param_type.to_string(), item.shortcut, item.name, item.command_line
        );
        lines.push(line);
    }
    let _ = fs::write(path, lines.join("\n"));
}

// ===== INI Config =====

pub fn load_config(path: &Path) -> AppConfig {
    let content = fs::read_to_string(path).unwrap_or_default();
    let mut cfg = AppConfig::default();

    let mut section = String::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim();
            let val = line[eq_pos + 1..].trim();

            match section.as_str() {
                "Config" => match key {
                    "HotKey1" => cfg.hotkey1 = val.to_string(),
                    "HotKey2" => cfg.hotkey2 = val.to_string(),
                    "AutoRun" => cfg.auto_run = val == "1" || val.to_lowercase() == "true",
                    "Regex" => cfg.enable_regex = val != "0" && val.to_lowercase() != "false",
                    "MatchAnywhere" => cfg.match_anywhere = val != "0",
                    "NumberKey" => cfg.enable_number_key = val != "0",
                    "IndexFrom0to9" => cfg.index_from_0 = val == "1",
                    "ShowTopTen" => cfg.show_top_ten = val != "0",
                    "ShowCommandLine" => cfg.show_command_line = val != "0",
                    "ShowOperationHint" => cfg.show_hint = val != "0",
                    "ExitWhenExecute" => cfg.exit_when_execute = val == "1",
                    "HideDelay" => cfg.hide_delay = val.parse().unwrap_or(15),
                    _ => {}
                },
                "GUI" => match key {
                    "FormWidth" => cfg.form_width = val.parse().unwrap_or(460),
                    "Alpha" => cfg.alpha = val.parse().unwrap_or(240),
                    "RoundBorderRadius" => cfg.round_border_radius = val.parse().unwrap_or(10),
                    "Theme" => cfg.theme = val.to_string(),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    cfg
}

pub fn save_config(path: &Path, cfg: &AppConfig) {
    let content = format!(
        "[Config]\n\
         HotKey1={}\n\
         HotKey2={}\n\
         AutoRun={}\n\
         Regex={}\n\
         MatchAnywhere={}\n\
         NumberKey={}\n\
         IndexFrom0to9={}\n\
         ShowTopTen={}\n\
         ShowCommandLine={}\n\
         ShowOperationHint={}\n\
         ExitWhenExecute={}\n\
         HideDelay={}\n\
         [GUI]\n\
         FormWidth={}\n\
         Alpha={}\n\
         RoundBorderRadius={}\n\
         Theme={}\n",
        cfg.hotkey1, cfg.hotkey2,
        if cfg.auto_run { 1 } else { 0 },
        if cfg.enable_regex { 1 } else { 0 },
        if cfg.match_anywhere { 1 } else { 0 },
        if cfg.enable_number_key { 1 } else { 0 },
        if cfg.index_from_0 { 1 } else { 0 },
        if cfg.show_top_ten { 1 } else { 0 },
        if cfg.show_command_line { 1 } else { 0 },
        if cfg.show_hint { 1 } else { 0 },
        if cfg.exit_when_execute { 1 } else { 0 },
        cfg.hide_delay, cfg.form_width, cfg.alpha, cfg.round_border_radius, cfg.theme,
    );
    let _ = fs::write(path, content);
}

// ===== Default shortcuts =====

fn default_shortcut_list() -> Vec<ShortCutItem> {
    #[cfg(target_os = "windows")]
    let defaults: Vec<(&str, &str, &str, ParamType, i32)> = vec![
        ("Computer", "My Computer", "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}", ParamType::None, 100),
        ("Explorer", "Explorer", "explorer.exe", ParamType::None, 50),
        ("notepad", "Notepad", "notepad", ParamType::None, 30),
        ("cmd", "Command Prompt", "cmd", ParamType::NoEncoding, 20),
        ("calc", "Calculator", "calc", ParamType::None, 15),
        ("taskmgr", "Task Manager", "taskmgr", ParamType::None, 10),
        ("regedit", "Registry Editor", "regedit", ParamType::None, 5),
        ("control", "Control Panel", "control.exe", ParamType::None, 5),
        ("b", "Baidu Search", "https://www.baidu.com/s?wd=", ParamType::URLQuery, 50),
        ("g", "Google Search", "https://www.google.com/search?q=", ParamType::UTF8Query, 50),
        ("shutdown", "Shutdown", "shutdown /s /t 5", ParamType::None, 0),
        ("reboot", "Reboot", "shutdown /r /t 5", ParamType::None, 0),
    ];

    #[cfg(target_os = "macos")]
    let defaults: Vec<(&str, &str, &str, ParamType, i32)> = vec![
        ("finder", "Finder", "/System/Library/CoreServices/Finder.app", ParamType::None, 100),
        ("terminal", "Terminal", "/System/Applications/Utilities/Terminal.app", ParamType::None, 50),
        ("safari", "Safari", "/Applications/Safari.app", ParamType::None, 30),
        ("notes", "Notes", "/System/Applications/Notes.app", ParamType::None, 20),
        ("calculator", "Calculator", "/System/Applications/Calculator.app", ParamType::None, 15),
        ("activity", "Activity Monitor", "/System/Applications/Utilities/Activity Monitor.app", ParamType::None, 10),
        ("prefs", "System Preferences", "/System/Applications/System Preferences.app", ParamType::None, 10),
        ("g", "Google Search", "https://www.google.com/search?q=", ParamType::UTF8Query, 50),
        ("b", "Baidu Search", "https://www.baidu.com/s?wd=", ParamType::URLQuery, 30),
    ];

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let defaults: Vec<(&str, &str, &str, ParamType, i32)> = vec![
        ("files", "File Manager", "xdg-open ~", ParamType::None, 50),
        ("terminal", "Terminal", "x-terminal-emulator", ParamType::None, 50),
        ("g", "Google Search", "https://www.google.com/search?q=", ParamType::UTF8Query, 50),
        ("b", "Baidu Search", "https://www.baidu.com/s?wd=", ParamType::URLQuery, 30),
    ];

    defaults.into_iter().enumerate().map(|(i, (sc, name, cmd, pt, freq))| ShortCutItem {
        id: i + 1, shortcut: sc.to_string(), name: name.to_string(),
        command_line: cmd.to_string(), param_type: pt, freq, rank: 0,
    }).collect()
}
