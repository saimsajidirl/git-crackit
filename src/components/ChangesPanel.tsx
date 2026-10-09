import React, { useMemo, useState } from "react";
import type { FileChange } from "../types";
import { Icon } from "../icons";
import { Dropdown } from "./ContextMenu";
import { cx, dirName, fileName, statusLetter } from "../util";

export interface FileSelection {
  path: string;
  staged: boolean;
}

export function ChangesPanel({
  changes,
  selected,
  onSelect,
  onToggleStage,
  onToggleAll,
  onCommit,
  onDiscard,
  onContextMenu,
  onDiscardAll,
  onStashAll,
  headName,
  repoState,
  isUnborn,
  busy,
}: {
  changes: FileChange[];
  selected: FileSelection | null;
  onSelect: (sel: FileSelection) => void;
  onToggleStage: (f: FileChange, stage: boolean) => void;
  onToggleAll: (stage: boolean) => void;
  onCommit: (summary: string, description: string, amend: boolean) => void;
  onDiscard: (paths: string[]) => void;
  onContextMenu: (e: React.MouseEvent, f: FileChange) => void;
  onDiscardAll: () => void;
  onStashAll: () => void;
  headName: string;
  repoState: string;
  isUnborn: boolean;
  busy: string | null;
}) {
  const [summary, setSummary] = useState("");
  const [description, setDescription] = useState("");
  const [amend, setAmend] = useState(false);
  const [kebabOpen, setKebabOpen] = useState(false);

  const conflicts = changes.filter((c) => c.conflicted);
  const normal = changes.filter((c) => !c.conflicted);
  const stagedCount = normal.filter((c) => c.staged !== null).length;
  const allChecked = normal.length > 0 && stagedCount === normal.length;
  const canCommit = stagedCount > 0 && summary.trim().length > 0 && !busy;

  const commitLabel = useMemo(() => {
    if (repoState === "merging") return "Commit merge";
    if (repoState === "cherry_picking") return "Commit cherry-pick";
    if (repoState === "reverting") return "Commit revert";
    return amend ? `Amend commit on ${headName}` : `Commit to ${headName}`;
  }, [repoState, headName, amend]);

  const doCommit = () => {
    if (!canCommit) return;
    const msg = description.trim() ? `${summary.trim()}\n\n${description.trim()}` : summary.trim();
    onCommit(msg, "", amend);
    setSummary("");
    setDescription("");
    setAmend(false);
  };

  const renderRow = (f: FileChange) => {
    const checked = f.staged !== null;
    const st = f.conflicted ? "conflicted" : (f.unstaged ?? f.staged);
    const isSel = selected?.path === f.path && selected.staged === checked;
    return (
      <div
        key={f.path + (checked ? ":s" : ":u")}
        className={cx("file-row", isSel && "selected", f.conflicted && "conflicted")}
        onClick={() => onSelect({ path: f.path, staged: checked })}
        onContextMenu={(e) => onContextMenu(e, f)}
      >
        <input
          type="checkbox"
          className="cb"
          checked={checked}
          disabled={f.conflicted}
          onChange={(e) => {
            e.stopPropagation();
            onToggleStage(f, e.target.checked);
          }}
          onClick={(e) => e.stopPropagation()}
          title={checked ? "Unstage" : "Stage"}
        />
        <span className={`status-badge st-${st}`}>{statusLetter(st ?? "modified")}</span>
        <span className="file-path" title={f.path}>
          <span className="file-name">{fileName(f.path)}</span>
          <span className="file-dir">{dirName(f.path)}</span>
        </span>
        {f.staged && f.unstaged && <span className="badge" title="Has both staged and unstaged changes">±</span>}
        {f.conflicted && <Icon name="alert" size={14} className="conflict-icon" />}
      </div>
    );
  };

  return (
    <div className="changes-panel">
      {conflicts.length > 0 && (
        <div className="conflict-banner">
          <Icon name="alert" size={14} />
          <span>{conflicts.length} conflicted file{conflicts.length > 1 ? "s" : ""} — resolve, then stage</span>
        </div>
      )}

      <div className="file-list-header">
        <input
          type="checkbox"
          className="cb"
          checked={allChecked}
          onChange={(e) => onToggleAll(e.target.checked)}
          title="Stage/unstage all"
        />
        <span className="muted grow small">
          {stagedCount > 0 ? `${stagedCount} of ${normal.length} staged` : `${normal.length} changed file${normal.length === 1 ? "" : "s"}`}
        </span>
        <div className="toolbar-button-wrap">
          <button className="icon-btn" title="More actions" onClick={() => setKebabOpen((v) => !v)}>
            <Icon name="kebab" size={14} />
          </button>
          <Dropdown open={kebabOpen} onClose={() => setKebabOpen(false)} align="right" width={220}>
            <button className="menu-item" onClick={() => { setKebabOpen(false); onStashAll(); }}>
              <Icon name="stash" size={14} /> Stash all changes…
            </button>
            <button className="menu-item danger" disabled={normal.length === 0} onClick={() => { setKebabOpen(false); onDiscardAll(); }}>
              <Icon name="trash" size={14} /> Discard all changes…
            </button>
          </Dropdown>
        </div>
      </div>

      <div className="file-list">
        {conflicts.map(renderRow)}
        {normal.map(renderRow)}
        {changes.length === 0 && (
          <div className="list-empty">
            <span className="empty-icon"><Icon name="check" size={22} /></span>
            <p>No local changes</p>
            <p className="faint small">Your working tree is clean</p>
          </div>
        )}
      </div>

      <div className="commit-box">
        <div className="commit-box-inner">
          <div className="summary-wrap">
            <input
              className="input"
              placeholder={isUnborn ? "Summary (required) — first commit" : "Summary (required)"}
              value={summary}
              onChange={(e) => setSummary(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.ctrlKey || e.metaKey || !e.shiftKey)) doCommit();
              }}
            />
            {summary.length > 0 && (
              <span className={cx("char-count", summary.length > 72 && "over")}>{72 - summary.length}</span>
            )}
          </div>
          <textarea
            className="input"
            placeholder="Description (Ctrl+Enter to commit)"
            rows={3}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) doCommit();
            }}
          />
          <label className="amend-row">
            <input
              type="checkbox"
              className="switch"
              checked={amend}
              onChange={(e) => setAmend(e.target.checked)}
              disabled={isUnborn}
            />
            <span>Amend last commit</span>
          </label>
          <button className="btn primary block" disabled={!canCommit} onClick={doCommit}>
            {busy ? <Icon name="sync" size={14} className="spin" /> : <Icon name="check" size={14} />}
            <span className="ellipsis">{busy ?? commitLabel}</span>
          </button>
        </div>
      </div>
    </div>
  );
}
