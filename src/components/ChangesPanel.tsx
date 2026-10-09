import React, { useMemo, useState } from "react";
import type { FileChange } from "../types";
import { Icon } from "../icons";
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
  headName: string;
  repoState: string;
  isUnborn: boolean;
  busy: string | null;
}) {
  const [summary, setSummary] = useState("");
  const [description, setDescription] = useState("");
  const [amend, setAmend] = useState(false);

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
      <div className="panel-header">
        <span className="panel-title">Changes</span>
        <span className="badge blue">{changes.length}</span>
      </div>

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
        <span className="muted">{normal.length} changed file{normal.length === 1 ? "" : "s"}</span>
      </div>

      <div className="file-list">
        {conflicts.map(renderRow)}
        {normal.map(renderRow)}
        {changes.length === 0 && (
          <div className="list-empty">
            <Icon name="check" size={28} />
            <p>No local changes</p>
          </div>
        )}
      </div>

      <div className="commit-box">
        <div className="commit-box-inner">
          <input
            className="input"
            placeholder={isUnborn ? "Summary (required) — first commit" : "Summary (required)"}
            value={summary}
            onChange={(e) => setSummary(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && doCommit()}
          />
          <textarea
            className="input"
            placeholder="Description"
            rows={3}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
          <label className="amend-row">
            <input
              type="checkbox"
              className="cb"
              checked={amend}
              onChange={(e) => setAmend(e.target.checked)}
              disabled={isUnborn}
            />
            <span>Amend last commit</span>
          </label>
          <button className="btn primary block" disabled={!canCommit} onClick={doCommit}>
            {busy ?? commitLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
