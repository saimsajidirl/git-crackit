import React, { useEffect, useState } from "react";
import { Modal, Field } from "./Modal";
import { Icon } from "../icons";
import { api } from "../api";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { BranchInfo, RemoteInfo, StashInfo, TagInfo, OpProgress, SubmoduleInfo } from "../types";
import { timeAgo } from "../util";
import { listen } from "@tauri-apps/api/event";

/* ---------------- Clone ---------------- */
export function CloneDialog({ onClose, onDone, onError }: { onClose: () => void; onDone: () => void; onError: (m: string) => void }) {
  const [url, setUrl] = useState("");
  const [dir, setDir] = useState("");
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<OpProgress | null>(null);

  useEffect(() => {
    const un = listen<OpProgress>("clone-progress", (e) => setProgress(e.payload));
    return () => { un.then((f) => f()); };
  }, []);

  const suggested = url.split("/").pop()?.replace(/\.git$/, "") ?? "";
  const dest = dir ? (suggested && !dir.endsWith(suggested) ? `${dir.replace(/\/$/, "")}/${suggested}` : dir) : "";

  const pickDir = async () => {
    const p = await openDialog({ directory: true, title: "Choose destination folder" });
    if (typeof p === "string") setDir(p);
  };

  const doClone = async () => {
    if (!url.trim() || !dest) return;
    setBusy(true);
    try {
      await api.cloneRepository(url.trim(), dest);
      onDone();
      onClose();
    } catch (e) {
      onError(String(e));
      setBusy(false);
    }
  };

  return (
    <Modal title="Clone a repository" onClose={onClose}>
      <Field label="Repository URL">
        <input className="input" autoFocus placeholder="https://github.com/user/repo.git" value={url} onChange={(e) => setUrl(e.target.value)} />
      </Field>
      <Field label="Local folder">
        <div className="input-row">
          <input className="input grow" placeholder="/path/to/parent-folder" value={dir} onChange={(e) => setDir(e.target.value)} />
          <button className="btn" onClick={pickDir}>Browse…</button>
        </div>
        {suggested && dir && <div className="hint">Will clone into <code>{dest}</code></div>}
      </Field>
      {progress && busy && (
        <div className="progress-line">
          <div className="progress-bar"><div className="progress-fill" style={{ width: progress.total ? `${(progress.received / progress.total) * 100}%` : "30%" }} /></div>
          <span className="muted small">Receiving objects: {progress.received}/{progress.total || "?"}</span>
        </div>
      )}
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" disabled={!url.trim() || !dir || busy} onClick={doClone}>
          {busy ? "Cloning…" : "Clone"}
        </button>
      </div>
    </Modal>
  );
}

/* ---------------- Init ---------------- */
export function InitDialog({ onClose, onDone, onError }: { onClose: () => void; onDone: () => void; onError: (m: string) => void }) {
  const [dir, setDir] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const path = dir ? (name ? `${dir.replace(/\/$/, "")}/${name}` : dir) : "";
  const pickDir = async () => {
    const p = await openDialog({ directory: true, title: "Choose parent folder" });
    if (typeof p === "string") setDir(p);
  };
  const doInit = async () => {
    if (!path) return;
    setBusy(true);
    try {
      await api.initRepository(path);
      onDone();
      onClose();
    } catch (e) {
      onError(String(e));
      setBusy(false);
    }
  };
  return (
    <Modal title="Create a new repository" onClose={onClose}>
      <Field label="Parent folder">
        <div className="input-row">
          <input className="input grow" value={dir} onChange={(e) => setDir(e.target.value)} placeholder="/path/to/parent-folder" />
          <button className="btn" onClick={pickDir}>Browse…</button>
        </div>
      </Field>
      <Field label="Repository name (optional — leave empty to init the folder itself)">
        <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="my-project" />
      </Field>
      {path && <div className="hint">Will initialize <code>{path}</code></div>}
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" disabled={!dir || busy} onClick={doInit}>{busy ? "Creating…" : "Create repository"}</button>
      </div>
    </Modal>
  );
}

/* ---------------- Confirm ---------------- */
export function ConfirmDialog({
  title, message, confirmLabel = "Confirm", danger, onClose, onConfirm,
}: {
  title: string; message: React.ReactNode; confirmLabel?: string; danger?: boolean;
  onClose: () => void; onConfirm: () => void;
}) {
  return (
    <Modal title={title} onClose={onClose} width={420}>
      <p className="confirm-msg">{message}</p>
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className={`btn ${danger ? "danger" : "primary"}`} onClick={() => { onConfirm(); onClose(); }}>
          {confirmLabel}
        </button>
      </div>
    </Modal>
  );
}

/* ---------------- New branch ---------------- */
export function NewBranchDialog({
  branches, defaultBase, onClose, onCreate, onError,
}: {
  branches: BranchInfo[]; defaultBase: string;
  onClose: () => void; onCreate: (name: string, base: string, checkout: boolean) => Promise<void> | void; onError: (m: string) => void;
}) {
  const [name, setName] = useState("");
  const [base, setBase] = useState(defaultBase);
  const [checkout, setCheckout] = useState(true);
  const allBases = branches;
  return (
    <Modal title="Create a new branch" onClose={onClose}>
      <Field label="Branch name">
        <input className="input" autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="feature/my-change" />
      </Field>
      <Field label="Create from">
        <select className="input" value={base} onChange={(e) => setBase(e.target.value)}>
          <option value="">HEAD</option>
          {base && !allBases.some((b) => b.name === base) && (
            <option value={base}>Commit {base.slice(0, 7)}</option>
          )}
          {allBases.map((b) => (
            <option key={(b.is_remote ? "r:" : "") + b.name} value={b.name}>{b.name}</option>
          ))}
        </select>
      </Field>
      <label className="amend-row">
        <input type="checkbox" className="cb" checked={checkout} onChange={(e) => setCheckout(e.target.checked)} />
        <span>Check out the new branch</span>
      </label>
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button
          className="btn primary"
          disabled={!name.trim()}
          onClick={async () => {
            try {
              await onCreate(name.trim(), base, checkout);
              onClose();
            } catch (e) { onError(String(e)); }
          }}
        >
          Create branch
        </button>
      </div>
    </Modal>
  );
}

/* ---------------- Pick a branch (merge / rebase) ---------------- */
export function PickBranchDialog({
  title, actionLabel, branches, exclude, onClose, onPick, onError,
}: {
  title: string; actionLabel: string; branches: BranchInfo[]; exclude?: string;
  onClose: () => void; onPick: (name: string) => Promise<void>; onError: (m: string) => void;
}) {
  const [sel, setSel] = useState("");
  const list = branches.filter((b) => b.name !== exclude);
  return (
    <Modal title={title} onClose={onClose}>
      <Field label="Branch">
        <select className="input" value={sel} onChange={(e) => setSel(e.target.value)} autoFocus>
          <option value="">Choose a branch…</option>
          <optgroup label="Local">
            {list.filter((b) => !b.is_remote).map((b) => <option key={b.name} value={b.name}>{b.name}</option>)}
          </optgroup>
          <optgroup label="Remote">
            {list.filter((b) => b.is_remote).map((b) => <option key={b.name} value={b.name}>{b.name}</option>)}
          </optgroup>
        </select>
      </Field>
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" disabled={!sel} onClick={async () => {
          try { await onPick(sel); onClose(); } catch (e) { onError(String(e)); }
        }}>{actionLabel}</button>
      </div>
    </Modal>
  );
}

/* ---------------- Rename branch ---------------- */
export function RenameBranchDialog({
  branch, onClose, onRename, onError,
}: { branch: string; onClose: () => void; onRename: (old: string, n: string) => Promise<void>; onError: (m: string) => void }) {
  const [name, setName] = useState(branch);
  return (
    <Modal title={`Rename branch "${branch}"`} onClose={onClose}>
      <Field label="New name">
        <input className="input" autoFocus value={name} onChange={(e) => setName(e.target.value)} />
      </Field>
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" disabled={!name.trim() || name === branch} onClick={async () => {
          try { await onRename(branch, name.trim()); onClose(); } catch (e) { onError(String(e)); }
        }}>Rename</button>
      </div>
    </Modal>
  );
}

/* ---------------- Stash ---------------- */
export function StashDialog({ onClose, onChanged, onError }: { onClose: () => void; onChanged: () => void; onError: (m: string) => void }) {
  const [stashes, setStashes] = useState<StashInfo[]>([]);
  const [msg, setMsg] = useState("");
  const [untracked, setUntracked] = useState(true);
  const load = () => api.listStashes().then(setStashes).catch((e) => onError(String(e)));
  useEffect(() => { load(); }, []);

  const wrap = async (fn: () => Promise<unknown>) => {
    try { await fn(); load(); onChanged(); } catch (e) { onError(String(e)); }
  };

  return (
    <Modal title="Stashes" onClose={onClose} width={520}>
      <div className="stash-new">
        <input className="input grow" placeholder="Stash message (optional)" value={msg} onChange={(e) => setMsg(e.target.value)} />
        <label className="amend-row"><input type="checkbox" className="cb" checked={untracked} onChange={(e) => setUntracked(e.target.checked)} /><span>Include untracked</span></label>
        <button className="btn primary" onClick={() => wrap(() => api.stashSave(msg, untracked))}>Stash changes</button>
      </div>
      <div className="modal-list">
        {stashes.length === 0 && <div className="list-empty small-pad"><p className="muted">No stashes</p></div>}
        {stashes.map((s) => (
          <div key={s.index} className="list-row">
            <Icon name="stash" size={14} />
            <div className="grow">
              <div className="ellipsis">{s.message.replace(/^On [^:]+: /, "")}</div>
              <div className="muted small">stash@{"{" + s.index + "}"} · {timeAgo(s.time)}</div>
            </div>
            <button className="btn small" onClick={() => wrap(() => api.stashApply(s.index))}>Apply</button>
            <button className="btn small" onClick={() => wrap(() => api.stashPop(s.index))}>Pop</button>
            <button className="btn small danger" onClick={() => wrap(() => api.stashDrop(s.index))}>Drop</button>
          </div>
        ))}
      </div>
      <div className="modal-actions"><button className="btn" onClick={onClose}>Close</button></div>
    </Modal>
  );
}

/* ---------------- Tags ---------------- */
export function TagsDialog({ onClose, onChanged, onError, initialTarget }: { onClose: () => void; onChanged: () => void; onError: (m: string) => void; initialTarget?: string }) {
  const [tags, setTags] = useState<TagInfo[]>([]);
  const [name, setName] = useState("");
  const [msg, setMsg] = useState("");
  const [target, setTarget] = useState(initialTarget ?? "");
  const load = () => api.listTags().then(setTags).catch((e) => onError(String(e)));
  useEffect(() => { load(); }, []);
  const wrap = async (fn: () => Promise<unknown>) => {
    try { await fn(); load(); onChanged(); } catch (e) { onError(String(e)); }
  };
  return (
    <Modal title="Tags" onClose={onClose} width={540}>
      <div className="stash-new">
        <input className="input" placeholder="Tag name" value={name} onChange={(e) => setName(e.target.value)} style={{ width: 140 }} />
        <input className="input grow" placeholder="Target (commit/branch — default HEAD)" value={target} onChange={(e) => setTarget(e.target.value)} />
        <input className="input grow" placeholder="Message (annotated, optional)" value={msg} onChange={(e) => setMsg(e.target.value)} />
        <button className="btn primary" disabled={!name.trim()} onClick={() => wrap(() => api.createTag(name.trim(), target || undefined, msg || undefined))}>Create</button>
      </div>
      <div className="modal-list">
        {tags.length === 0 && <div className="list-empty small-pad"><p className="muted">No tags</p></div>}
        {tags.map((t) => (
          <div key={t.name} className="list-row">
            <Icon name="tag" size={14} />
            <div className="grow">
              <div>{t.name} <span className="muted small">{t.annotated ? "annotated" : "lightweight"}</span></div>
              <div className="muted small">{t.oid.slice(0, 7)} · {t.time ? timeAgo(t.time) : ""}</div>
            </div>
            <button className="btn small" onClick={() => wrap(() => api.pushTag(t.name))}>Push</button>
            <button className="btn small danger" onClick={() => wrap(() => api.deleteTag(t.name))}>Delete</button>
          </div>
        ))}
      </div>
      <div className="modal-actions"><button className="btn" onClick={onClose}>Close</button></div>
    </Modal>
  );
}

/* ---------------- Remotes ---------------- */
export function RemotesDialog({ onClose, onChanged, onError }: { onClose: () => void; onChanged: () => void; onError: (m: string) => void }) {
  const [remotes, setRemotes] = useState<RemoteInfo[]>([]);
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const load = () => api.listRemotes().then(setRemotes).catch((e) => onError(String(e)));
  useEffect(() => { load(); }, []);
  const wrap = async (fn: () => Promise<unknown>) => {
    try { await fn(); load(); onChanged(); } catch (e) { onError(String(e)); }
  };
  return (
    <Modal title="Remotes" onClose={onClose} width={560}>
      <div className="stash-new">
        <input className="input" placeholder="name" value={name} onChange={(e) => setName(e.target.value)} style={{ width: 110 }} />
        <input className="input grow" placeholder="https://github.com/user/repo.git" value={url} onChange={(e) => setUrl(e.target.value)} />
        <button className="btn primary" disabled={!name.trim() || !url.trim()} onClick={() => wrap(() => api.addRemote(name.trim(), url.trim()))}>Add</button>
      </div>
      <div className="modal-list">
        {remotes.length === 0 && <div className="list-empty small-pad"><p className="muted">No remotes configured</p></div>}
        {remotes.map((r) => (
          <div key={r.name} className="list-row">
            <Icon name="globe" size={14} />
            <div className="grow">
              <div>{r.name}</div>
              <input
                className="input small-input"
                defaultValue={r.url ?? ""}
                onBlur={(e) => e.target.value !== r.url && wrap(() => api.setRemoteUrl(r.name, e.target.value))}
              />
            </div>
            <button className="btn small" onClick={() => wrap(() => api.fetchRemote(r.name))}>Fetch</button>
            <button className="btn small danger" onClick={() => wrap(() => api.removeRemote(r.name))}>Remove</button>
          </div>
        ))}
      </div>
      <div className="modal-actions"><button className="btn" onClick={onClose}>Close</button></div>
    </Modal>
  );
}

/* ---------------- Settings (git identity) ---------------- */
export function SettingsDialog({ onClose, onError }: { onClose: () => void; onError: (m: string) => void }) {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [global, setGlobal] = useState(false);
  const [loaded, setLoaded] = useState(false);
  useEffect(() => {
    api.getGitIdentity().then((c) => {
      setName(c.name ?? "");
      setEmail(c.email ?? "");
      setLoaded(true);
    }).catch((e) => onError(String(e)));
  }, []);
  return (
    <Modal title="Git settings" onClose={onClose}>
      <Field label="Name">
        <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Your name" />
      </Field>
      <Field label="Email">
        <input className="input" value={email} onChange={(e) => setEmail(e.target.value)} placeholder="you@example.com" />
      </Field>
      <label className="amend-row">
        <input type="checkbox" className="cb" checked={global} onChange={(e) => setGlobal(e.target.checked)} />
        <span>Save globally (~/.gitconfig) instead of this repository</span>
      </label>
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" disabled={!loaded} onClick={async () => {
          try { await api.setGitIdentity(name.trim(), email.trim(), global); onClose(); } catch (e) { onError(String(e)); }
        }}>Save</button>
      </div>
    </Modal>
  );
}

/* ---------------- Submodules ---------------- */
export function SubmodulesDialog({ onClose, onChanged, onError }: { onClose: () => void; onChanged: () => void; onError: (m: string) => void }) {
  const [subs, setSubs] = useState<SubmoduleInfo[]>([]);
  const [busy, setBusy] = useState(false);
  useEffect(() => { api.listSubmodules().then(setSubs).catch((e) => onError(String(e))); }, []);
  return (
    <Modal title="Submodules" onClose={onClose} width={520}>
      <div className="modal-list">
        {subs.length === 0 && <div className="list-empty small-pad"><p className="muted">No submodules</p></div>}
        {subs.map((s) => (
          <div key={s.name} className="list-row">
            <Icon name="repo" size={14} />
            <div className="grow">
              <div>{s.name}</div>
              <div className="muted small">{s.path}{s.url ? ` · ${s.url}` : ""}</div>
            </div>
          </div>
        ))}
      </div>
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Close</button>
        <button className="btn primary" disabled={subs.length === 0 || busy} onClick={async () => {
          setBusy(true);
          try { await api.updateSubmodules(); onChanged(); } catch (e) { onError(String(e)); }
          setBusy(false);
        }}>{busy ? "Updating…" : "Init & update all"}</button>
      </div>
    </Modal>
  );
}

/* ---------------- Credentials (HTTPS) ---------------- */
export function CredentialsDialog({ onClose, onSaved, onError }: { onClose: () => void; onSaved: () => void; onError: (m: string) => void }) {
  const [user, setUser] = useState("");
  const [pass, setPass] = useState("");
  return (
    <Modal title="Remote credentials (HTTPS)" onClose={onClose}>
      <p className="muted small">
        Used for fetch/push/clone over HTTPS. For GitHub, use your username and a personal access token as the password.
      </p>
      <Field label="Username">
        <input className="input" autoFocus value={user} onChange={(e) => setUser(e.target.value)} autoComplete="off" />
      </Field>
      <Field label="Password / token">
        <input className="input" type="password" value={pass} onChange={(e) => setPass(e.target.value)} autoComplete="off" />
      </Field>
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" onClick={async () => {
          try { await api.setCredentials(user || null, pass || null); onSaved(); onClose(); } catch (e) { onError(String(e)); }
        }}>Save & retry</button>
      </div>
    </Modal>
  );
}
