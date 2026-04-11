use crate::models::{ParamType, ShortCutItem};
use std::env;
use std::path::Path;

/// Replace environment variables.
/// Windows: %VAR% style
/// macOS/Linux: $VAR or ${VAR} style, also expands ~
pub fn expand_env_vars(s: &str) -> String {
    let mut result = s.to_string();

    #[cfg(target_os = "windows")]
    {
        let re = regex::Regex::new(r"%([^%]+)%").unwrap();
        for cap in re.captures_iter(s) {
            if let Ok(val) = env::var(&cap[1]) {
                result = result.replace(&cap[0], &val);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Expand ~ to home dir
        if result.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                result = result.replacen('~', &home.to_string_lossy(), 1);
            }
        }
        // Expand $VAR or ${VAR}
        let re = regex::Regex::new(r"\$\{?([A-Za-z_][A-Za-z0-9_]*)\}?").unwrap();
        let s_clone = result.clone();
        for cap in re.captures_iter(&s_clone) {
            if let Ok(val) = env::var(&cap[1]) {
                result = result.replace(&cap[0], &val);
            }
        }
    }

    result
}

/// Launch a file/URL/app using the OS-native method.
/// Windows: ShellExecuteW (no console window)
/// macOS:   open
/// Linux:   xdg-open
fn shell_execute(file: &str, params: &str, dir: &str, show: i32) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;

        fn to_wide(s: &str) -> Vec<u16> {
            OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
        }

        #[link(name = "shell32")]
        extern "system" {
            fn ShellExecuteW(
                hwnd: *mut std::ffi::c_void,
                operation: *const u16,
                file: *const u16,
                parameters: *const u16,
                directory: *const u16,
                show_cmd: i32,
            ) -> isize;
        }

        let w_file   = to_wide(file);
        let w_params = to_wide(params);
        let w_dir    = to_wide(dir);
        let w_open   = to_wide("open");

        let dir_ptr    = if dir.is_empty()    { ptr::null() } else { w_dir.as_ptr() };
        let params_ptr = if params.is_empty() { ptr::null() } else { w_params.as_ptr() };

        let ret = unsafe {
            ShellExecuteW(ptr::null_mut(), w_open.as_ptr(), w_file.as_ptr(), params_ptr, dir_ptr, show)
        };
        return if ret as usize > 32 { Ok(()) }
               else { Err(format!("ShellExecuteW failed: {}", ret)) };
    }

    #[cfg(not(target_os = "windows"))]
    {
        use std::process::Command;
        let _ = (dir, show); // unused on non-Windows

        let is_url = file.starts_with("http://") || file.starts_with("https://")
            || file.starts_with("ftp://") || file.starts_with("mailto:");

        #[cfg(target_os = "macos")]
        let open_cmd = "open";
        #[cfg(not(target_os = "macos"))]
        let open_cmd = "xdg-open";

        if is_url || params.is_empty() {
            Command::new(open_cmd)
                .arg(file)
                .spawn()
                .map_err(|e| format!("Failed to open '{}': {}", file, e))?;
        } else {
            let mut cmd = Command::new(file);
            for arg in params.split_whitespace() { cmd.arg(arg); }
            cmd.spawn().map_err(|e| format!("Failed to execute '{}': {}", file, e))?;
        }
        Ok(())
    }
}

/// Execute a shortcut item.
/// `param` is the user-typed text after the first space (e.g. "dir" from "cmd dir").
pub fn execute_shortcut(item: &ShortCutItem, _keyword: &str, param: &str) -> Result<(), String> {
    let mut cmd_line = item.command_line.clone();

    // Expand environment variables
    cmd_line = expand_env_vars(&cmd_line);

    // Parse show-mode prefix (@+, @-, @) — Windows only in practice, but parsed everywhere
    let show: i32 = if cmd_line.starts_with("@+") { 3 }
        else if cmd_line.starts_with("@-") { 7 }
        else if cmd_line.starts_with('@')  { 0 }
        else { 1 };

    if cmd_line.starts_with("@+") || cmd_line.starts_with("@-") {
        cmd_line = cmd_line[2..].trim().to_string();
    } else if cmd_line.starts_with('@') {
        cmd_line = cmd_line[1..].trim().to_string();
    }

    // Parameter substitution
    match item.param_type {
        ParamType::None => {
            if !param.is_empty() {
                if cmd_line.contains("{%p}") {
                    cmd_line = cmd_line.replace("{%p}", param);
                } else if cmd_line.contains("%p") {
                    cmd_line = cmd_line.replace("%p", param);
                } else {
                    cmd_line = format!("{} {}", cmd_line, param);
                }
            }
        }
        ParamType::NoEncoding => {
            let p = if param.is_empty() { _keyword } else { param };
            if cmd_line.contains("{%p}") {
                cmd_line = cmd_line.replace("{%p}", p);
            } else if cmd_line.contains("%p") {
                cmd_line = cmd_line.replace("%p", p);
            } else if cmd_line.contains("{%c}") {
                cmd_line = cmd_line.replace("{%c}", p);
            } else {
                cmd_line = format!("{} {}", cmd_line, p);
            }
        }
        ParamType::URLQuery | ParamType::UTF8Query => {
            let p = if param.is_empty() { _keyword } else { param };
            let encoded = urlencoding::encode(p).to_string();
            if cmd_line.contains("{%p}") {
                cmd_line = cmd_line.replace("{%p}", &encoded);
            } else if cmd_line.contains("{%c}") {
                cmd_line = cmd_line.replace("{%c}", &encoded);
            } else {
                cmd_line.push_str(&encoded);
            }
        }
    }

    // Resolve relative paths (./ or .\ ) using exe directory as base
    let mut working_dir = String::new();
    let rel = format!(".{}", std::path::MAIN_SEPARATOR);
    if cmd_line.contains(&rel) || cmd_line.contains("./") {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                working_dir = exe_dir.to_string_lossy().to_string();
                // Normalise both separators
                cmd_line = cmd_line
                    .replace(".\\", &format!("{}\\", exe_dir.display()))
                    .replace("./",  &format!("{}/",  exe_dir.display()));
            }
        }
    }

    // Split into executable + arguments
    let (file, shell_params) = split_command(&cmd_line);

    // Determine working directory from the file path if not already set
    if working_dir.is_empty() {
        let clean = file.trim_matches('"');
        let path = Path::new(clean);
        if path.exists() {
            if let Some(parent) = path.parent() {
                working_dir = parent.to_string_lossy().to_string();
            }
        }
    }

    shell_execute(&file, &shell_params, &working_dir, show)
}

/// Open the folder that contains the shortcut's target.
pub fn open_directory(item: &ShortCutItem) -> Result<(), String> {
    let cmd = expand_env_vars(&item.command_line);
    let cmd = cmd
        .trim_start_matches("@+")
        .trim_start_matches("@-")
        .trim_start_matches('@')
        .trim()
        .trim_matches('"');

    let path = Path::new(cmd);
    let dir = if path.is_dir() {
        path.to_string_lossy().to_string()
    } else if let Some(parent) = path.parent() {
        parent.to_string_lossy().to_string()
    } else {
        return Err("Cannot determine directory".into());
    };

    #[cfg(target_os = "windows")]
    return shell_execute("explorer.exe", &dir, "", 1);

    #[cfg(not(target_os = "windows"))]
    return shell_execute(&dir, "", "", 1);
}

/// Split a command string into (executable, arguments).
/// Handles quoted executables: `"C:\path\to\app.exe" --flag`
fn split_command(cmd: &str) -> (String, String) {
    let cmd = cmd.trim();
    if cmd.starts_with('"') {
        if let Some(end) = cmd[1..].find('"') {
            let exe  = format!("\"{}\"", &cmd[1..=end]);
            let args = cmd[end + 2..].trim().to_string();
            return (exe, args);
        }
    }
    if let Some(pos) = cmd.find(' ') {
        (cmd[..pos].to_string(), cmd[pos + 1..].trim().to_string())
    } else {
        (cmd.to_string(), String::new())
    }
}
