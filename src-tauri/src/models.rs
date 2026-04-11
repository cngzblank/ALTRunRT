use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParamType {
    None,
    NoEncoding,
    URLQuery,
    UTF8Query,
}

impl fmt::Display for ParamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParamType::None => write!(f, ""),
            ParamType::NoEncoding => write!(f, "No_Encoding"),
            ParamType::URLQuery => write!(f, "URL_Query"),
            ParamType::UTF8Query => write!(f, "UTF8_Query"),
        }
    }
}

impl ParamType {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "no_encoding" => ParamType::NoEncoding,
            "url_query" => ParamType::URLQuery,
            "utf8_query" => ParamType::UTF8Query,
            _ => ParamType::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortCutItem {
    pub id: usize,
    pub shortcut: String,
    pub name: String,
    pub command_line: String,
    pub param_type: ParamType,
    pub freq: i32,
    pub rank: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub hotkey1: String,
    pub hotkey2: String,
    pub auto_run: bool,
    pub enable_regex: bool,
    pub match_anywhere: bool,
    pub enable_number_key: bool,
    pub index_from_0: bool,
    pub show_top_ten: bool,
    pub show_command_line: bool,
    pub show_hint: bool,
    pub exit_when_execute: bool,
    pub hide_delay: u32,
    pub form_width: u32,
    pub alpha: u32,
    pub round_border_radius: u32,
    pub theme: String,  // "dark" or "light"
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey1: "Alt+R".into(),
            hotkey2: "Pause".into(),
            auto_run: false,
            enable_regex: true,
            match_anywhere: true,
            enable_number_key: true,
            index_from_0: false,
            show_top_ten: true,
            show_command_line: true,
            show_hint: true,
            exit_when_execute: false,
            hide_delay: 15,
            form_width: 460,
            alpha: 240,
            round_border_radius: 10,
            theme: "dark".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterResult {
    pub items: Vec<ShortCutItem>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportData {
    pub version: String,
    pub config: AppConfig,
    pub items: Vec<ShortCutItem>,
}
