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
  const cards = [
    { icon: "download", title: "Clone a repository", desc: "Copy a remote repository to your computer", onClick: onClone },
    { icon: "folder", title: "Open a local repository", desc: "Add an existing repository from disk", onClick: onOpenDialog },
    { icon: "plus", title: "Create a new repository", desc: "Initialize a fresh Git repository", onClick: onInit },
  ];
  return (
    <div className="welcome">
      <div className="welcome-inner">
        <div className="welcome-brand">
          <div className="welcome-logo">
            <Icon name="branch" size={32} />
          </div>
          <h1>
            Welcome to <span>Git Crackit</span>
          </h1>
          <p className="muted">A fast, friendly Git client</p>
        </div>
        <div className="welcome-actions">
          {cards.map((c) => (
            <button key={c.title} className="welcome-card" onClick={c.onClick}>
              <span className="card-icon">
                <Icon name={c.icon} size={18} />
              </span>
              <div>
                <div className="card-title">{c.title}</div>
                <div className="muted small">{c.desc}</div>
              </div>
            </button>
          ))}
        </div>
        {recent.length > 0 && (
          <div className="welcome-recent">
            <div className="welcome-recent-title">Recent repositories</div>
            <div className="welcome-recent-list">
              {recent.map((r) => (
                <div key={r} className="recent-row">
                  <button className="menu-item grow" onClick={() => onOpenPath(r)} title={r}>
                    <span className="recent-icon">
                      <Icon name="repo" size={14} />
                    </span>
                    <span className="ellipsis" style={{ fontWeight: 600 }}>{r.split("/").pop()}</span>
                    <span className="faint small ellipsis">{r}</span>
                  </button>
                  <button className="icon-btn" title="Remove" onClick={() => onRemoveRecent(r)}>
                    <Icon name="x" size={12} />
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
