export type ParamType = "None" | "NoEncoding" | "URLQuery" | "UTF8Query";

export interface ShortCutItem {
  id: number;
  shortcut: string;
  name: string;
  command_line: string;
  param_type: ParamType;
  freq: number;
  rank: number;
}

export interface AppConfig {
  hotkey1: string;
  hotkey2: string;
  auto_run: boolean;
  enable_regex: boolean;
  match_anywhere: boolean;
  enable_number_key: boolean;
  index_from_0: boolean;
  show_top_ten: boolean;
  show_command_line: boolean;
  show_hint: boolean;
  exit_when_execute: boolean;
  hide_delay: number;
  form_width: number;
  alpha: number;
  round_border_radius: number;
  theme: string;
}

export interface FilterResult {
  items: ShortCutItem[];
  total: number;
}
