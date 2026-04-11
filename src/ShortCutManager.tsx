import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ShortCutItem, ParamType } from "./types";

interface Props {
  onClose: () => void;
}

const PARAM_OPTIONS: { value: ParamType; label: string }[] = [
  { value: "None", label: "No Param" },
  { value: "NoEncoding", label: "Param (no encoding)" },
  { value: "URLQuery", label: "URL Query encoding" },
  { value: "UTF8Query", label: "UTF-8 Query encoding" },
];

export default function ShortCutManager({ onClose }: Props) {
  const [items, setItems] = useState<ShortCutItem[]>([]);
  const [selected, setSelected] = useState(-1);
  const [editing, setEditing] = useState<ShortCutItem | null>(null);
  const [isNew, setIsNew] = useState(false);

  const loadItems = async () => {
    try {
      const list: ShortCutItem[] = await invoke("get_all_items");
      setItems(list);
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => { loadItems(); }, []);

  const handleAdd = () => {
    setEditing({ id: 0, shortcut: "", name: "", command_line: "", param_type: "None", freq: 0, rank: 0 });
    setIsNew(true);
  };

  const handleEdit = () => {
    if (selected >= 0 && selected < items.length) {
      setEditing({ ...items[selected] });
      setIsNew(false);
    }
  };

  const handleDelete = async () => {
    if (selected < 0 || selected >= items.length) return;
    const item = items[selected];
    if (!confirm(`Delete "${item.shortcut}" (${item.name})?`)) return;
    try {
      await invoke("delete_item", { id: item.id });
      await loadItems();
      setSelected(-1);
    } catch (e) {
      console.error(e);
    }
  };

  const handleSave = async () => {
    if (!editing) return;
    try {
      if (isNew) {
        await invoke("add_item", {
          shortcut: editing.shortcut,
          name: editing.name,
          commandLine: editing.command_line,
          paramType: editing.param_type,
        });
      } else {
        await invoke("update_item", {
          id: editing.id,
          shortcut: editing.shortcut,
          name: editing.name,
          commandLine: editing.command_line,
          paramType: editing.param_type,
        });
      }
      setEditing(null);
      await loadItems();
    } catch (e) {
      console.error(e);
      alert("Save failed: " + e);
    }
  };

  return (
    <div className="overlay">
      <div className="overlay-header">
        <h2>Shortcut Manager</h2>
        <button className="btn" onClick={onClose}>Close</button>
      </div>

      <div className="toolbar">
        <button className="btn" onClick={handleAdd}>+ Add</button>
        <button className="btn" onClick={handleEdit} disabled={selected < 0}>Edit</button>
        <button className="btn" onClick={handleDelete} disabled={selected < 0}>Delete</button>
      </div>

      {editing ? (
        <div className="overlay-body">
          <div className="form-group">
            <label>Shortcut (keyword)</label>
            <input value={editing.shortcut} onChange={e => setEditing({ ...editing, shortcut: e.target.value })} autoFocus />
          </div>
          <div className="form-group">
            <label>Name</label>
            <input value={editing.name} onChange={e => setEditing({ ...editing, name: e.target.value })} />
          </div>
          <div className="form-group">
            <label>Command Line</label>
            <input value={editing.command_line} onChange={e => setEditing({ ...editing, command_line: e.target.value })} />
          </div>
          <div className="form-group">
            <label>Param Type</label>
            <select value={editing.param_type} onChange={e => setEditing({ ...editing, param_type: e.target.value as ParamType })}>
              {PARAM_OPTIONS.map(o => <option key={o.value} value={o.value}>{o.label}</option>)}
            </select>
          </div>
          <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
            <button className="btn btn-accent" onClick={handleSave}>Save</button>
            <button className="btn" onClick={() => setEditing(null)}>Cancel</button>
          </div>
        </div>
      ) : (
        <div className="overlay-body" style={{ padding: 0 }}>
          <table className="manager-table">
            <thead>
              <tr>
                <th style={{ width: 120 }}>Shortcut</th>
                <th style={{ width: 160 }}>Name</th>
                <th style={{ width: 80 }}>Param</th>
                <th>Command Line</th>
                <th style={{ width: 50 }}>Freq</th>
              </tr>
            </thead>
            <tbody>
              {items.map((item, i) => (
                <tr
                  key={item.id}
                  className={i === selected ? "selected" : ""}
                  onClick={() => setSelected(i)}
                  onDoubleClick={handleEdit}
                >
                  <td>{item.shortcut}</td>
                  <td>{item.name}</td>
                  <td>{item.param_type === "None" ? "" : item.param_type}</td>
                  <td style={{ maxWidth: 300, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{item.command_line}</td>
                  <td>{item.freq}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
