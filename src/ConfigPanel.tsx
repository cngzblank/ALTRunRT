import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save, open } from "@tauri-apps/plugin-dialog";
import type { AppConfig } from "./types";

interface Props {
  onClose: () => void;
}

function applyTheme(theme: string) {
  document.documentElement.setAttribute("data-theme", theme === "light" ? "light" : "");
}

export default function ConfigPanel({ onClose }: Props) {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [status, setStatus] = useState("");

  useEffect(() => {
    invoke<AppConfig>("get_config").then(setConfig).catch(console.error);
  }, []);

  const setTheme = (theme: string) => {
    if (!config) return;
    setConfig({ ...config, theme });
    applyTheme(theme);
  };

  const handleSave = async () => {
    if (!config) return;
    try {
      await invoke("save_config", { config });
      onClose();
    } catch (e) {
      console.error(e);
      alert("Save failed: " + e);
    }
  };

  const handleCancel = () => {
    invoke<AppConfig>("get_config").then(cfg => applyTheme(cfg.theme)).catch(() => {});
    onClose();
  };

  const handleExport = async () => {
    try {
      const path = await save({
        title: "Export ALTRun Data",
        defaultPath: "altrun-backup.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return;
      await invoke("export_data", { path });
      setStatus("✅ Exported successfully!");
      setTimeout(() => setStatus(""), 3000);
    } catch (e) {
      setStatus("❌ Export failed: " + e);
    }
  };

  const handleImport = async () => {
    try {
      const selected = await open({
        title: "Import ALTRun Data",
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!selected) return;
      const msg: string = await invoke("import_data", { path: selected });
      setStatus("✅ " + msg + " 请重启 ALTRun，以确保导入的配置全部生效。");
      const newConfig = await invoke<AppConfig>("get_config");
      setConfig(newConfig);
      applyTheme(newConfig.theme);
      setTimeout(() => setStatus(""), 8000);
    } catch (e) {
      setStatus("❌ Import failed: " + e);
    }
  };

  if (!config) return <div className="overlay"><div className="overlay-body">Loading...</div></div>;

  const toggle = (key: keyof AppConfig) => {
    setConfig({ ...config, [key]: !config[key] } as AppConfig);
  };

  const setNum = (key: keyof AppConfig, val: string) => {
    setConfig({ ...config, [key]: parseInt(val) || 0 } as AppConfig);
  };

  const setStr = (key: keyof AppConfig, val: string) => {
    setConfig({ ...config, [key]: val } as AppConfig);
  };

  const CheckItem = ({ label, field, desc }: { label: string; field: keyof AppConfig; desc?: string }) => (
    <label style={{ display: "flex", alignItems: "flex-start", gap: 8, padding: "5px 0", fontSize: 13, cursor: "pointer" }}>
      <input type="checkbox" checked={!!config[field]} onChange={() => toggle(field)} style={{ marginTop: 2, flexShrink: 0 }} />
      <span>
        {label}
        {desc && <span style={{ display: "block", fontSize: 11, color: "var(--text-dim)", marginTop: 1 }}>{desc}</span>}
      </span>
    </label>
  );

  const isLight = config.theme === "light";

  return (
    <div className="overlay">
      <div className="overlay-header">
        <h2>Configuration</h2>
        <button className="btn btn-accent" onClick={handleSave}>Save</button>
        <button className="btn" onClick={handleCancel}>Cancel</button>
      </div>
      <div className="overlay-body">

        {/* Theme */}
        <h3 style={{ fontSize: 13, color: "var(--text-dim)", marginBottom: 8 }}>Theme</h3>
        <div className="theme-toggle" onClick={() => setTheme(isLight ? "dark" : "light")}>
          <span>🌙</span>
          <div className={`theme-toggle-track ${isLight ? "active" : ""}`}>
            <div className="theme-toggle-knob" />
          </div>
          <span>☀️</span>
          <span style={{ marginLeft: 4, color: "var(--text-dim)" }}>{isLight ? "Light" : "Dark"}</span>
        </div>

        {/* Import / Export */}
        <h3 style={{ fontSize: 13, color: "var(--text-dim)", margin: "12px 0 8px" }}>Import / Export</h3>
        <div style={{ display: "flex", gap: 8, marginBottom: 8 }}>
          <button className="btn" onClick={handleExport}>📤 Export All</button>
          <button className="btn" onClick={handleImport}>📥 Import</button>
        </div>
        {status && <div style={{ fontSize: 12, padding: "4px 0", color: status.startsWith("✅") ? "#4ade80" : "#f87171" }}>{status}</div>}

        {/* Hotkeys */}
        <h3 style={{ fontSize: 13, color: "var(--text-dim)", margin: "12px 0 8px" }}>Hotkeys</h3>
        <div className="form-group">
          <label>Primary Hotkey</label>
          <input value={config.hotkey1} onChange={e => setStr("hotkey1", e.target.value)} />
        </div>
        <div className="form-group">
          <label>Secondary Hotkey</label>
          <input value={config.hotkey2} onChange={e => setStr("hotkey2", e.target.value)} />
        </div>

        {/* Behavior */}
        <h3 style={{ fontSize: 13, color: "var(--text-dim)", margin: "12px 0 8px" }}>Behavior</h3>
        <CheckItem
          label="Launch at Windows startup"
          field="auto_run"
          desc="Automatically start ALTRun when Windows boots"
        />
        <CheckItem label="Enable Regex (wildcard *, ?)" field="enable_regex" />
        <CheckItem label="Match keyword from anywhere" field="match_anywhere" />
        <CheckItem label="Enable number key (0-9) to launch" field="enable_number_key" />
        <CheckItem label="Index from 0 to 9 (instead of 1-0)" field="index_from_0" />
        <CheckItem label="Show top 10 only" field="show_top_ten" />
        <CheckItem label="Show command line" field="show_command_line" />
        <CheckItem label="Show hints" field="show_hint" />
        <CheckItem label="Exit after execute" field="exit_when_execute" />

        {/* Appearance */}
        <h3 style={{ fontSize: 13, color: "var(--text-dim)", margin: "12px 0 8px" }}>Appearance</h3>
        <div className="form-group">
          <label>Hide delay (seconds)</label>
          <input type="number" value={config.hide_delay} onChange={e => setNum("hide_delay", e.target.value)} />
        </div>
        <div className="form-group">
          <label>Window width (px)</label>
          <input type="number" value={config.form_width} onChange={e => setNum("form_width", e.target.value)} />
        </div>
        <div className="form-group">
          <label>Opacity (0-255)</label>
          <input type="number" min={50} max={255} value={config.alpha} onChange={e => setNum("alpha", e.target.value)} />
        </div>
        <div className="form-group">
          <label>Border radius (px)</label>
          <input type="number" value={config.round_border_radius} onChange={e => setNum("round_border_radius", e.target.value)} />
        </div>

      </div>
    </div>
  );
}
