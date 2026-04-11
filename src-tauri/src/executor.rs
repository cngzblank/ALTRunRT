use crate::models::{ParamType, ShortCutItem};
use std::env;
use std::path::Path;

/// Replace environment variables like %WINDIR%, %PROGRAMFILES% etc.
pub fn expand_env_vars(s: &str) -> String {
    let mut result = s.to_string();
    let re = regex::Regex::new(r"%([^%]+)%").unwrap();
    for cap in re.captures_iter(s) {
        let var_name = &cap[1];
        if let Ok(val) = env::var(var_name) {
            result = result.replace(&cap[0], &val);
        }
    }
    result
}

/// Execute using Windows ShellExecuteW (no console window)
fn shell_execute(file: &str, params: &str, dir: &str, show: i32) -> Result<(), String> {
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

    let w_file = to_wide(file);
    let w_params = to_wide(params);
    let w_dir = to_wide(dir);
    let w_open = to_wide("open");

    let dir_ptr = if dir.is_empty() { ptr::null() } else { w_dir.as_ptr() };
    let params_ptr = if params.is_empty() { ptr::null() } else { w_params.as_ptr() };

    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            w_open.as_ptr(),
            w_file.as_ptr(),
            params_ptr,
            dir_ptr,
            show,
        )
    };

    if result as usize > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecuteW failed with code {}", result))
    }
}

/// Replace multi-param placeholders in command line.
///
/// Supported placeholders:
///   {%1} {%2} {%3} ...  — individual positional params
///   {%p} or %p           — ALL params joined by space (legacy compat)
///   {%c}                 — clipboard / all params (legacy compat)
///
/// `param` is the raw string after the first space, e.g. "arg1 arg2 arg3".
/// It gets split by whitespace into positional args.
fn substitute_params(cmd: &str, param: &str, param_type: &ParamType) -> String {
    let mut result = cmd.to_string();
    let all_param = param.to_string();

    // Split into individual args (respecting quoted strings)
    let args = split_args(param);

    // Encode helper
    let encode = |s: &str| -> String {
        match param_type {
            ParamType::URLQuery | ParamType::UTF8Query => urlencoding::encode(s).to_string(),
            _ => s.to_string(),
        }
    };

    // 1) Replace numbered placeholders {%1}, {%2}, ... {%9}
    let mut has_numbered = false;
    for i in 1..=9 {
        let placeholder = format!("{{%{}}}", i);
        if result.contains(&placeholder) {
            has_numbered = true;
            let val = args.get(i - 1).map(|s| s.as_str()).unwrap_or("");
            result = result.replace(&placeholder, &encode(val));
        }
    }

    // If numbered placeholders were used, we're done with substitution
    if has_numbered {
        return result;
    }

    // 2) Replace {%p} — all params as one string
    if result.contains("{%p}") {
        result = result.replace("{%p}", &encode(&all_param));
        return result;
    }

    // 3) Replace %p (legacy, no braces)
    if result.contains("%p") {
        result = result.replace("%p", &encode(&all_param));
        return result;
    }

    // 4) Replace {%c} — clipboard / all params
    if result.contains("{%c}") {
        result = result.replace("{%c}", &encode(&all_param));
        return result;
    }

    // 5) No placeholder found — append or concat depending on type
    if !all_param.is_empty() {
        match param_type {
            ParamType::None | ParamType::NoEncoding => {
                // Append as extra arguments
                result = format!("{} {}", result, all_param);
            }
            ParamType::URLQuery | ParamType::UTF8Query => {
                // Append encoded (for search URLs like https://google.com/search?q=)
                result.push_str(&encode(&all_param));
            }
        }
    }

    result
}

/// Split a parameter string into individual args, respecting "quoted strings"
fn split_args(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.is_empty() {
        return Vec::new();
    }

    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_quote = !in_quote;
            }
            ' ' if !in_quote => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        args.push(current);
    }

    args
}

/// Execute a shortcut item.
pub fn execute_shortcut(item: &ShortCutItem, _keyword: &str, param: &str) -> Result<(), String> {
    let mut cmd_line = item.command_line.clone();

    // Expand environment variables
    cmd_line = expand_env_vars(&cmd_line);

    // Handle show flags: @+ (maximized), @- (minimized), @ (hidden)
    let show_cmd: i32;
    if cmd_line.starts_with("@+") {
        show_cmd = 3;
        cmd_line = cmd_line[2..].trim().to_string();
    } else if cmd_line.starts_with("@-") {
        show_cmd = 7;
        cmd_line = cmd_line[2..].trim().to_string();
    } else if cmd_line.starts_with('@') {
        show_cmd = 0;
        cmd_line = cmd_line[1..].trim().to_string();
    } else {
        show_cmd = 1;
    }

    // Substitute parameters
    cmd_line = substitute_params(&cmd_line, param, &item.param_type);

    // Handle relative paths (.\ or ..\)
    let mut working_dir = String::new();
    if cmd_line.contains(".\\") || cmd_line.contains("..\\") {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                working_dir = exe_dir.to_string_lossy().to_string();
                cmd_line = cmd_line.replace(".\\", &format!("{}\\", exe_dir.display()));
            }
        }
    }

    // Split into file and parameters for ShellExecuteW
    let (file, shell_params) = split_command(&cmd_line);

    // Determine working directory
    if working_dir.is_empty() {
        let clean = file.trim_matches('"');
        let path = Path::new(clean);
        if path.exists() {
            if let Some(parent) = path.parent() {
                working_dir = parent.to_string_lossy().to_string();
            }
        }
    }

    shell_execute(&file, &shell_params, &working_dir, show_cmd)
}

/// Open the directory containing the shortcut's target
pub fn open_directory(item: &ShortCutItem) -> Result<(), String> {
    let cmd = expand_env_vars(&item.command_line);
    let cmd = cmd.trim_start_matches("@+")
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

    shell_execute("explorer.exe", &dir, "", 1)
}

fn split_command(cmd: &str) -> (String, String) {
    let cmd = cmd.trim();
    if cmd.starts_with('"') {
        if let Some(end) = cmd[1..].find('"') {
            let exe = format!("\"{}\"", &cmd[1..=end]);
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
