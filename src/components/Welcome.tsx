import React from "react";
import { Icon } from "../icons";

export function Welcome({
  recent,
  onOpenPath,
  onOpenDialog,
  onClone,
  onInit,
  onRemoveRecent,
}: {
  recent: string[];
  onOpenPath: (p: string) => void;
  onOpenDialog: () => void;
  onClone: () => void;
  onInit: () => void;
  onRemoveRecent: (p: string) => void;
}) {
  return (
    <div className="welcome">
      <div className="welcome-inner">
        <div className="welcome-brand">
          <div className="welcome-logo">
            <Icon name="branch" size={30} />
          </div>
          <h1>Git Crackit</h1>
          <p className="muted">A fast, friendly Git client</p>
        </div>
        <div className="welcome-grid">
          <div className="welcome-actions">
            <button className="welcome-card" onClick={onClone}>
              <Icon name="download" size={22} />
              <div>
                <div className="card-title">Clone a repository</div>
                <div className="muted">Copy a remote repository to your computer</div>
              </div>
            </button>
            <button className="welcome-card" onClick={onOpenDialog}>
              <Icon name="folder" size={22} />
              <div>
                <div className="card-title">Open a local repository</div>
                <div className="muted">Add an existing repository from disk</div>
              </div>
            </button>
            <button className="welcome-card" onClick={onInit}>
              <Icon name="plus" size={22} />
              <div>
                <div className="card-title">Create a new repository</div>
                <div className="muted">Initialize a fresh Git repository</div>
              </div>
            </button>
          </div>
          {recent.length > 0 && (
            <div className="welcome-recent">
              <div className="panel-header">
                <span className="panel-title">Recent repositories</span>
              </div>
              {recent.map((r) => (
                <div key={r} className="recent-row">
                  <button className="menu-item grow" onClick={() => onOpenPath(r)} title={r}>
                    <Icon name="repo" size={16} />
                    <span className="ellipsis">{r.split("/").pop()}</span>
                    <span className="muted small ellipsis">{r}</span>
                  </button>
                  <button className="icon-btn" title="Remove" onClick={() => onRemoveRecent(r)}>
                    <Icon name="x" size={12} />
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
