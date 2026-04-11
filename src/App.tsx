import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ShortCutItem, FilterResult, ParamType, AppConfig } from "./types";
import ShortCutManager from "./ShortCutManager";
import ConfigPanel from "./ConfigPanel";

function splitInput(raw: string): { search: string; param: string } {
  const idx = raw.indexOf(" ");
  if (idx < 0) return { search: raw, param: "" };
  return { search: raw.substring(0, idx), param: raw.substring(idx + 1) };
}

function applyTheme(theme: string) {
  document.documentElement.setAttribute("data-theme", theme === "light" ? "light" : "");
}

function AddDialog({ initial, onSave, onCancel }: {
  initial: string;
  onSave: (sc: string, name: string, cmd: string, pt: ParamType) => void;
  onCancel: () => void;
}) {
  const [shortcut, setShortcut] = useState(initial);
  const [name, setName] = useState("");
  const [cmd, setCmd] = useState("");
  const [pt, setPt] = useState<ParamType>("None");
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => { ref.current?.focus(); }, []);

  return (
    <div className="overlay">
      <div className="overlay-header">
        <h2>Add Shortcut</h2>
        <button className="btn btn-accent" onClick={() => onSave(shortcut, name || shortcut, cmd, pt)}>Save</button>
        <button className="btn" onClick={onCancel}>Cancel</button>
      </div>
      <div className="overlay-body">
        <div className="form-group">
          <label>Shortcut (keyword)</label>
          <input ref={ref} value={shortcut} onChange={e => setShortcut(e.target.value)} />
        </div>
        <div className="form-group">
          <label>Name</label>
          <input value={name} onChange={e => setName(e.target.value)} placeholder={shortcut} />
        </div>
        <div className="form-group">
          <label>Command Line</label>
          <input value={cmd} onChange={e => setCmd(e.target.value)} placeholder="e.g. notepad.exe, https://..." />
        </div>
        <div className="form-group">
          <label>Param Type</label>
          <select value={pt} onChange={e => setPt(e.target.value as ParamType)}>
            <option value="None">No Param</option>
            <option value="NoEncoding">Param (no encoding)</option>
            <option value="URLQuery">URL Query encoding</option>
            <option value="UTF8Query">UTF-8 Query encoding</option>
          </select>
        </div>
      </div>
    </div>
  );
}

function App() {
  const [keyword, setKeyword] = useState("");
  const [items, setItems] = useState<ShortCutItem[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [cmdLine, setCmdLine] = useState("");
  const [hint, setHint] = useState("Type keyword, press Enter to run");
  const [showManager, setShowManager] = useState(false);
  const [showConfig, setShowConfig] = useState(false);
  const [showAdd, setShowAdd] = useState(false);
  const [addInitial, setAddInitial] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const { search: searchWord, param: paramWord } = splitInput(keyword);

  useEffect(() => {
    invoke<AppConfig>("get_config").then(cfg => applyTheme(cfg.theme)).catch(() => {});
  }, []);

  useEffect(() => {
    const unlisten = listen<string>("theme-changed", (event) => applyTheme(event.payload));
    return () => { unlisten.then(f => f()); };
  }, []);

  useEffect(() => {
    const doFilter = async () => {
      try {
        const result: FilterResult = await invoke("filter_keyword", { keyword: searchWord });
        setItems(result.items);
        setSelectedIndex(0);
        setCmdLine(result.items.length > 0 ? result.items[0].command_line : "");
      } catch (e) { console.error("filter error:", e); }
    };
    doFilter();
  }, [searchWord]);

  useEffect(() => {
    if (items.length > 0 && selectedIndex >= 0 && selectedIndex < items.length) {
      setCmdLine(items[selectedIndex].command_line);
    }
  }, [selectedIndex, items]);

  useEffect(() => {
    const unlisten = listen("show-window", () => {
      setKeyword("");
      setTimeout(() => inputRef.current?.focus(), 50);
    });
    return () => { unlisten.then(f => f()); };
  }, []);

  useEffect(() => { inputRef.current?.focus(); }, []);

  const hideWindow = useCallback(async () => {
    try { await invoke("hide_window"); } catch (e) { console.error(e); }
  }, []);

  const executeSelected = useCallback(async () => {
    if (items.length === 0) {
      if (searchWord.trim()) { setAddInitial(searchWord.trim()); setShowAdd(true); }
      return;
    }
    const item = items[selectedIndex];
    try {
      await invoke("execute_item", { id: item.id, keyword: searchWord, param: paramWord });
      setKeyword("");
    } catch (e) { console.error("execute error:", e); }
  }, [items, selectedIndex, searchWord, paramWord]);

  const executeByIndex = useCallback(async (idx: number) => {
    if (idx < 0 || idx >= items.length) return;
    const item = items[idx];
    try {
      await invoke("execute_item", { id: item.id, keyword: searchWord, param: paramWord });
      setKeyword("");
    } catch (e) { console.error("execute error:", e); }
  }, [items, searchWord, paramWord]);

  const handleAddSave = async (sc: string, name: string, cmd: string, pt: ParamType) => {
    if (!sc.trim() || !cmd.trim()) return;
    try {
      await invoke("add_item", { shortcut: sc, name, commandLine: cmd, paramType: pt });
      setShowAdd(false);
      setKeyword(sc);
    } catch (e) { console.error("add error:", e); }
  };

  const handleConfigClose = useCallback(() => {
    setShowConfig(false);
    invoke<AppConfig>("get_config").then(cfg => applyTheme(cfg.theme)).catch(() => {});
  }, []);

  // Resize handle — drag from bottom-right corner
  const startResize = useCallback(async (e: React.MouseEvent) => {
    e.preventDefault();
    try {
      await getCurrentWindow().startResizeDragging("SouthEast");
    } catch (err) {
      console.error("resize error:", err);
    }
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setSelectedIndex(prev => (prev + 1) % Math.max(items.length, 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setSelectedIndex(prev => (prev - 1 + items.length) % Math.max(items.length, 1));
        break;
      case "Enter":
        e.preventDefault();
        executeSelected();
        break;
      case "Escape":
        e.preventDefault();
        if (keyword) { setKeyword(""); } else { hideWindow(); }
        break;
      case "Tab":
        e.preventDefault();
        if (e.shiftKey) {
          setSelectedIndex(prev => (prev - 1 + items.length) % Math.max(items.length, 1));
        } else {
          setSelectedIndex(prev => (prev + 1) % Math.max(items.length, 1));
        }
        break;
      default:
        if ((e.altKey || e.ctrlKey) && e.key >= "0" && e.key <= "9") {
          e.preventDefault();
          const num = parseInt(e.key);
          executeByIndex(num === 0 ? 9 : num - 1);
        }
        if (e.ctrlKey && e.key === "d") {
          e.preventDefault();
          if (items.length > 0) invoke("open_item_dir", { id: items[selectedIndex].id });
        }
        if (e.ctrlKey && e.key === "c" && !window.getSelection()?.toString()) {
          e.preventDefault();
          if (items.length > 0) {
            navigator.clipboard.writeText(items[selectedIndex].command_line);
            setHint("Copied!"); setTimeout(() => setHint(""), 1500);
          }
        }
        if (e.altKey && e.key === "s") { e.preventDefault(); setShowManager(true); }
        if (e.altKey && e.key === "c") { e.preventDefault(); setShowConfig(true); }
        break;
    }
  };

  const getDisplayIndex = (i: number) => ((i + 1) % 10).toString();

  const getParamBadge = (pt: string) => {
    if (pt === "None") return null;
    const labels: Record<string, string> = { NoEncoding: "Param", URLQuery: "URL", UTF8Query: "UTF8" };
    return <span className="result-param-badge">{labels[pt] || pt}</span>;
  };

  return (
    <>
      <div className="altrun">
        <div className="titlebar">
          <span className="titlebar-label">
            {items.length > 0 && selectedIndex < items.length ? items[selectedIndex].name : "ALTRun"}
          </span>
          <button className="titlebar-btn" title="Shortcut Manager (Alt+S)" onClick={() => setShowManager(true)}>☰</button>
          <button className="titlebar-btn" title="Config (Alt+C)" onClick={() => setShowConfig(true)}>⚙</button>
          <button className="titlebar-btn" title="Hide (Esc)" onClick={hideWindow}>✕</button>
        </div>

        <div className="search-row">
          <input
            ref={inputRef}
            className="search-input"
            type="text"
            placeholder="Type keyword [space param]..."
            value={keyword}
            onChange={e => setKeyword(e.target.value)}
            onKeyDown={handleKeyDown}
            spellCheck={false}
            autoFocus
          />
        </div>

        {items.length > 0 ? (
          <div className="result-list">
            {items.map((item, i) => (
              <div
                key={item.id}
                className={`result-item ${i === selectedIndex ? "selected" : ""}`}
                onClick={() => { setSelectedIndex(i); executeByIndex(i); }}
                onMouseEnter={() => setSelectedIndex(i)}
              >
                <span className="result-index">{getDisplayIndex(i)}</span>
                <span className="result-shortcut">{item.shortcut}</span>
                <span className="result-name">{item.name}</span>
                {getParamBadge(item.param_type)}
              </div>
            ))}
          </div>
        ) : (
          <div className="empty-state">
            {keyword ? `No match for "${searchWord}" — press Enter to add` : "Ready"}
          </div>
        )}

        <div className="statusbar">
          <span className="statusbar-cmd">
            {cmdLine ? `CMD=${cmdLine}` : ""}
            {paramWord ? ` | Param: ${paramWord}` : ""}
          </span>
          <span className="statusbar-hint">{hint}</span>
        </div>
      </div>

      {/* Resize grip — sits at the true window corner, outside .altrun */}
      <div className="resize-handle" onMouseDown={startResize} />

      {showAdd && <AddDialog initial={addInitial} onSave={handleAddSave} onCancel={() => setShowAdd(false)} />}
      {showManager && <ShortCutManager onClose={() => { setShowManager(false); setKeyword(""); }} />}
      {showConfig && <ConfigPanel onClose={handleConfigClose} />}
    </>
  );
}

export default App;
