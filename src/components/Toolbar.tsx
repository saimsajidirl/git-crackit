import React, { useState } from "react";
import type { RepoInfo, BranchInfo } from "../types";
import { Icon } from "../icons";
import { Dropdown } from "./ContextMenu";
import { timeAgo } from "../util";

export interface ToolbarProps {
  repo: RepoInfo | null;
  branches: BranchInfo[];
  recent: string[];
  busy: string | null;
  onOpenPath: (path: string) => void;
  onOpenDialog: () => void;
  onClone: () => void;
  onInit: () => void;
  onFetch: () => void;
  onPull: () => void;
  onPush: () => void;
  onCheckout: (name: string) => void;
  onNewBranch: () => void;
  onMerge: () => void;
  onRebase: () => void;
  onStashMenu: () => void;
  onTags: () => void;
  onRemotes: () => void;
  onSettings: () => void;
  onCredentials: () => void;
  onCloseRepo: () => void;
  onRemoveRecent: (path: string) => void;
  onBranchContext: (e: React.MouseEvent, b: BranchInfo) => void;
  onFetchAll: () => void;
  onSubmodules: () => void;
}

function ToolbarButton({
  icon, title, subtitle, onClick, children, dropdown, onDropdown, disabled,
}: {
  icon: string; title: string; subtitle?: string; onClick?: () => void;
  children?: React.ReactNode; dropdown?: boolean; onDropdown?: () => void; disabled?: boolean;
}) {
  return (
    <div className="toolbar-button-wrap">
      <button className="toolbar-button" onClick={onClick ?? onDropdown} disabled={disabled}>
        <Icon name={icon} size={16} />
        <span className="tb-text">
          <span className="tb-title">{title}</span>
          {subtitle && <span className="tb-subtitle">{subtitle}</span>}
        </span>
        {dropdown && <Icon name="chevronDown" size={12} />}
      </button>
      {children}
    </div>
  );
}

export function Toolbar(p: ToolbarProps) {
  const [openMenu, setOpenMenu] = useState<string | null>(null);
  const toggle = (m: string) => setOpenMenu((cur) => (cur === m ? null : m));
  const close = () => setOpenMenu(null);

  const repo = p.repo;
  const ahead = repo?.ahead ?? 0;
  const behind = repo?.behind ?? 0;

  let fetchTitle = "Fetch origin";
  let fetchSub: string | undefined;
  let fetchAction = p.onFetch;
  if (repo && repo.remotes.length === 0) {
    fetchTitle = "Publish";
    fetchSub = "no remote configured";
    fetchAction = p.onRemotes;
  } else if (repo) {
    if (!repo.upstream) {
      fetchTitle = "Publish branch";
      fetchSub = `to ${repo.remotes[0] ?? "origin"}`;
      fetchAction = p.onPush;
    } else if (behind > 0) {
      fetchTitle = `Pull origin`;
      fetchSub = `↓${behind}${ahead > 0 ? ` ↑${ahead}` : ""}`;
      fetchAction = p.onPull;
    } else if (ahead > 0) {
      fetchTitle = `Push origin`;
      fetchSub = `↑${ahead}`;
      fetchAction = p.onPush;
    } else {
      fetchSub = repo.upstream ? "up to date" : undefined;
    }
  }

  const localBranches = p.branches.filter((b) => !b.is_remote);
  const remoteBranches = p.branches.filter((b) => b.is_remote);

  return (
    <div className="toolbar">
      <ToolbarButton icon="repo" title={repo ? repo.name : "No repository"} subtitle={repo ? "Current repository" : "Open a repository"} dropdown onDropdown={() => toggle("repo")}>
        <Dropdown open={openMenu === "repo"} onClose={close} width={280}>
          <div className="dropdown-section">Recent repositories</div>
          {p.recent.length === 0 && <div className="dropdown-empty">No recent repositories</div>}
          {p.recent.map((r) => (
            <div key={r} className="dropdown-item-row">
              <button
                className="menu-item grow"
                onClick={() => { close(); p.onOpenPath(r); }}
                title={r}
              >
                <Icon name="repo" size={14} />
                <span className="ellipsis">{r.split("/").pop()}</span>
                <span className="muted ellipsis small">{r}</span>
              </button>
              <button className="icon-btn" title="Remove from recent" onClick={() => p.onRemoveRecent(r)}>
                <Icon name="x" size={12} />
              </button>
            </div>
          ))}
          <div className="menu-separator" />
          <button className="menu-item" onClick={() => { close(); p.onOpenDialog(); }}>
            <Icon name="folder" size={14} /> Open local repository…
          </button>
          <button className="menu-item" onClick={() => { close(); p.onClone(); }}>
            <Icon name="download" size={14} /> Clone repository…
          </button>
          <button className="menu-item" onClick={() => { close(); p.onInit(); }}>
            <Icon name="plus" size={14} /> Create new repository…
          </button>
          {repo && (
            <>
              <div className="menu-separator" />
              <button className="menu-item danger" onClick={() => { close(); p.onCloseRepo(); }}>
                <Icon name="x" size={14} /> Close repository
              </button>
            </>
          )}
        </Dropdown>
      </ToolbarButton>

      {repo && (
        <>
          <ToolbarButton
            icon="sync"
            title={p.busy ?? fetchTitle}
            subtitle={fetchSub}
            onClick={fetchAction}
          />

          <ToolbarButton
            icon="branch"
            title={repo.is_detached ? `HEAD @ ${repo.head}` : repo.head ?? "no branch"}
            subtitle="Current branch"
            dropdown
            onDropdown={() => toggle("branch")}
          >
            <Dropdown open={openMenu === "branch"} onClose={close} width={320}>
              <div className="dropdown-section">
                <span>Branches</span>
                <button className="link-btn" onClick={() => { close(); p.onNewBranch(); }}>
                  <Icon name="plus" size={12} /> New branch
                </button>
              </div>
              <div className="dropdown-scroll">
                {localBranches.map((b) => (
                  <button
                    key={b.name}
                    className={"menu-item branch-item" + (b.is_head ? " current" : "")}
                    onClick={() => { close(); if (!b.is_head) p.onCheckout(b.name); }}
                    onContextMenu={(e) => { e.preventDefault(); close(); p.onBranchContext(e, b); }}
                  >
                    <Icon name={b.is_head ? "check" : "branch"} size={14} />
                    <span className="ellipsis grow">{b.name}</span>
                    {b.ahead != null && b.ahead > 0 && <span className="badge">↑{b.ahead}</span>}
                    {b.behind != null && b.behind > 0 && <span className="badge">↓{b.behind}</span>}
                    <span className="muted small">{b.last_commit_time ? timeAgo(b.last_commit_time) : ""}</span>
                  </button>
                ))}
                {remoteBranches.length > 0 && <div className="dropdown-section">Remote branches</div>}
                {remoteBranches.map((b) => (
                  <button key={b.name} className="menu-item branch-item" onClick={() => { close(); p.onCheckout(b.name); }}
                    onContextMenu={(e) => { e.preventDefault(); close(); p.onBranchContext(e, b); }}
                  >
                    <Icon name="globe" size={14} />
                    <span className="ellipsis grow">{b.name}</span>
                    <span className="muted small">{b.last_commit_time ? timeAgo(b.last_commit_time) : ""}</span>
                  </button>
                ))}
              </div>
            </Dropdown>
          </ToolbarButton>

          <ToolbarButton icon="merge" title="Branch" subtitle="Actions" dropdown onDropdown={() => toggle("actions")}>
            <Dropdown open={openMenu === "actions"} onClose={close} width={240}>
              <button className="menu-item" onClick={() => { close(); p.onNewBranch(); }}>
                <Icon name="plus" size={14} /> New branch…
              </button>
              <button className="menu-item" onClick={() => { close(); p.onMerge(); }}>
                <Icon name="merge" size={14} /> Merge into current branch…
              </button>
              <button className="menu-item" onClick={() => { close(); p.onRebase(); }}>
                <Icon name="sync" size={14} /> Rebase current branch onto…
              </button>
              <div className="menu-separator" />
              <button className="menu-item" onClick={() => { close(); p.onStashMenu(); }}>
                <Icon name="stash" size={14} /> Stashes…
              </button>
              <button className="menu-item" onClick={() => { close(); p.onTags(); }}>
                <Icon name="tag" size={14} /> Tags…
              </button>
            </Dropdown>
          </ToolbarButton>

          <div className="toolbar-spacer" />

          <ToolbarButton icon="gear" title="Repository" dropdown onDropdown={() => toggle("repo-menu")}>
            <Dropdown open={openMenu === "repo-menu"} onClose={close} align="right" width={240}>
              <button className="menu-item" onClick={() => { close(); p.onFetch(); }}>
                <Icon name="sync" size={14} /> Fetch
              </button>
              <button className="menu-item" onClick={() => { close(); p.onFetchAll(); }}>
                <Icon name="download" size={14} /> Fetch all remotes
              </button>
              <button className="menu-item" onClick={() => { close(); p.onRemotes(); }}>
                <Icon name="globe" size={14} /> Remotes…
              </button>
              <button className="menu-item" onClick={() => { close(); p.onSubmodules(); }}>
                <Icon name="repo" size={14} /> Submodules…
              </button>
              <button className="menu-item" onClick={() => { close(); p.onCredentials(); }}>
                <Icon name="gear" size={14} /> Credentials…
              </button>
              <div className="menu-separator" />
              <button className="menu-item" onClick={() => { close(); p.onSettings(); }}>
                <Icon name="gear" size={14} /> Git settings…
              </button>
            </Dropdown>
          </ToolbarButton>
        </>
      )}
    </div>
  );
}
