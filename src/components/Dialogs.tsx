import React, { useEffect, useState } from "react";
import { Modal, Field } from "./Modal";
import { Avatar } from "./Avatar";
import { Icon } from "../icons";
import { api } from "../api";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { BranchInfo, RemoteInfo, StashInfo, TagInfo, OpProgress, SubmoduleInfo, GhRepo, GhPR, GhIssue, GhCheckStatus, DeviceFlow, CommitInfo, FileDiff, CompareResult } from "../types";
import { timeAgo, fileName, dirName, statusLetter, cx } from "../util";
import { DiffView } from "./DiffView";
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
        <input className="input" autoFocus placeholder="https://github.com/user/repo.git or git@github.com:user/repo.git" value={url} onChange={(e) => setUrl(e.target.value)} />
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

/* ---------------- Dirty-worktree choice (switch/merge/pull/rebase) ---------------- */
export function DirtyCheckoutDialog({
  target, current, count, op = "switch", onClose, onPick,
}: {
  target: string; current: string; count: number; op?: "switch" | "merge" | "pull" | "rebase";
  onClose: () => void; onPick: (mode: "carry" | "stash") => void;
}) {
  const copy = {
    switch: {
      title: `Switch to "${target}"`,
      intro: <>You have <b>{count}</b> uncommitted change{count === 1 ? "" : "s"} on <b>{current}</b>. What should happen to them?</>,
      carry: [`Take changes to ${target}`, "Your edits move with you to the new branch"],
      stash: [`Keep changes on ${current}`, `Stashed on ${current} — switch back and pop the stash to continue`],
    },
    merge: {
      title: `Merge "${target}"`,
      intro: <>You have <b>{count}</b> uncommitted change{count === 1 ? "" : "s"} that this merge would touch.</>,
      carry: ["Stash, merge, then re-apply", "Your edits are re-applied on top of the merge (may conflict)"],
      stash: ["Keep changes in a stash", `Stashed on ${current} — pop it after the merge`],
    },
    pull: {
      title: `Pull "${target}"`,
      intro: <>You have <b>{count}</b> uncommitted change{count === 1 ? "" : "s"} that this pull would touch.</>,
      carry: ["Stash, pull, then re-apply", "Your edits are re-applied on top of the pull (may conflict)"],
      stash: ["Keep changes in a stash", `Stashed on ${current} — pop it after the pull`],
    },
    rebase: {
      title: `Rebase onto "${target}"`,
      intro: <>Rebase requires a clean working tree — you have <b>{count}</b> uncommitted change{count === 1 ? "" : "s"}.</>,
      carry: ["Stash, rebase, then re-apply", "Your edits are re-applied on top of the rebase (may conflict)"],
      stash: ["Keep changes in a stash", `Stashed on ${current} — pop it after the rebase`],
    },
  }[op];
  return (
    <Modal title={copy.title} onClose={onClose}>
      <p className="confirm-msg">{copy.intro}</p>
      <div className="choice-list">
        <button className="btn choice-btn" onClick={() => onPick("carry")}>
          <span>{copy.carry[0]}</span>
          <span className="choice-sub">{copy.carry[1]}</span>
        </button>
        <button className="btn choice-btn" onClick={() => onPick("stash")}>
          <span>{copy.stash[0]}</span>
          <span className="choice-sub">{copy.stash[1]}</span>
        </button>
      </div>
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
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
export function SettingsDialog({ aiOnly, onClose, onError }: { aiOnly?: boolean; onClose: () => void; onError: (m: string) => void }) {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [global, setGlobal] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [aiProvider, setAiProvider] = useState("auto");
  const [aiModel, setAiModel] = useState("");
  const [aiBase, setAiBase] = useState("");
  const [aiKey, setAiKey] = useState("");
  const [aiKeyed, setAiKeyed] = useState<string[]>([]);
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [gpgsign, setGpgsign] = useState(false);
  const [signingkey, setSigningkey] = useState("");
  const [gpgformat, setGpgformat] = useState("openpgp");
  const KEY_PROVIDERS = ["anthropic", "openai", "groq", "custom"];
  const KEY_URLS: Record<string, string> = {
    anthropic: "https://console.anthropic.com/settings/keys",
    openai: "https://platform.openai.com/api-keys",
    groq: "https://console.groq.com/keys",
  };
  const needsKey = KEY_PROVIDERS.includes(aiProvider);
  const keySaved = aiKeyed.includes(aiProvider);
  useEffect(() => {
    api.getGitIdentity().then((c) => {
      setName(c.name ?? "");
      setEmail(c.email ?? "");
      setGpgsign(c.gpgsign);
      setSigningkey(c.signingkey ?? "");
      setGpgformat(c.gpgformat || "openpgp");
      setLoaded(true);
    }).catch((e) => onError(String(e)));
    api.getAiSettings().then((s) => {
      setAiProvider(s.provider);
      setAiModel(s.model);
      setAiBase(s.base_url);
      setAiKeyed(s.keyed_providers);
    }).catch(() => {});
  }, []);
  useEffect(() => {
    if (aiProvider === "ollama") api.listOllamaModels().then(setOllamaModels).catch(() => setOllamaModels([]));
  }, [aiProvider]);
  return (
    <Modal title={aiOnly ? "AI keys" : "Git settings"} onClose={onClose}>
      {!aiOnly && (
        <>
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
          <div className="settings-title">Commit signing</div>
          <label className="amend-row">
            <input type="checkbox" className="switch" checked={gpgsign} onChange={(e) => setGpgsign(e.target.checked)} />
            <span>Sign commits (commit.gpgsign)</span>
          </label>
          {gpgsign && (
            <>
              <Field label="Signing key (user.signingkey)">
                <input
                  className="input"
                  value={signingkey}
                  onChange={(e) => setSigningkey(e.target.value)}
                  placeholder={gpgformat === "ssh" ? "~/.ssh/id_ed25519.pub" : "GPG key ID or fingerprint"}
                />
              </Field>
              <Field label="Format (gpg.format)">
                <select className="input" value={gpgformat} onChange={(e) => setGpgformat(e.target.value)}>
                  <option value="openpgp">OpenPGP (gpg)</option>
                  <option value="ssh">SSH (ssh-keygen)</option>
                </select>
              </Field>
            </>
          )}
          <hr className="settings-sep" />
        </>
      )}
      <div className="settings-title">AI commit messages</div>
      <Field label="Provider">
        <select className="input" value={aiProvider} onChange={(e) => setAiProvider(e.target.value)}>
          <option value="auto">Auto — first saved key, else local Ollama</option>
          <option value="anthropic">Claude (Anthropic)</option>
          <option value="openai">OpenAI</option>
          <option value="groq">Groq</option>
          <option value="ollama">Ollama (local — diffs stay on this machine)</option>
          <option value="custom">Custom OpenAI-compatible endpoint</option>
        </select>
      </Field>
      {(aiProvider === "ollama" || aiProvider === "custom") && (
        <Field label="Model">
          <input className="input" list="gc-ai-models" value={aiModel} onChange={(e) => setAiModel(e.target.value)}
            placeholder={aiProvider === "ollama" ? "qwen2.5-coder:3b" : "model name"} />
          {aiProvider === "ollama" && <datalist id="gc-ai-models">{ollamaModels.map((m) => <option key={m} value={m} />)}</datalist>}
        </Field>
      )}
      {KEY_PROVIDERS.includes(aiProvider) && aiProvider !== "custom" && (
        <div className="hint">Always uses the cheapest model the provider offers — no picking needed.</div>
      )}
      {aiProvider === "custom" && (
        <Field label="Base URL">
          <input className="input" value={aiBase} onChange={(e) => setAiBase(e.target.value)} placeholder="https://api.example.com/v1" />
        </Field>
      )}
      {needsKey && (
        <Field label="API key">
          <input className="input" type="password" value={aiKey} onChange={(e) => setAiKey(e.target.value)}
            placeholder={keySaved ? "saved — enter to replace" : "paste your API key"} />
          {KEY_URLS[aiProvider] && (
            <button className="link-btn" onClick={() => api.openExternalUrl(KEY_URLS[aiProvider]).catch((e) => onError(String(e)))}>
              Get a key →
            </button>
          )}
        </Field>
      )}
      <div className="hint">
        AI is used <b>only</b> for generating commit messages from your staged diff — nothing else.
        Keys stay on this device in your OS keychain; they are never stored in the cloud or sent
        anywhere except the chosen provider when you click ✦.
      </div>
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" disabled={!loaded} onClick={async () => {
          try {
            if (!aiOnly) {
              await api.setGitIdentity(name.trim(), email.trim(), global);
              await api.setSigningConfig(gpgsign, signingkey.trim() || null, gpgformat, global);
            }
            await api.setAiSettings(aiProvider, aiModel.trim(), aiBase.trim(), aiKey || null);
            onClose();
          } catch (e) { onError(String(e)); }
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
const GITHUB_TOKEN_URL = "https://github.com/settings/tokens/new?scopes=repo&description=Git%20Crackit";

export function CredentialsDialog({ onClose, onSaved, onError }: { onClose: () => void; onSaved: () => void; onError: (m: string) => void }) {
  const [user, setUser] = useState("");
  const [pass, setPass] = useState("");
  const [remember, setRemember] = useState(true);
  const [savedUser, setSavedUser] = useState<string | null>(null);
  const [remembered, setRemembered] = useState(false);
  const [flow, setFlow] = useState<DeviceFlow | null>(null);
  const [flowMsg, setFlowMsg] = useState<string | null>(null);
  const rememberRef = React.useRef(remember);
  rememberRef.current = remember;

  useEffect(() => {
    api.getCredentials()
      .then((c) => {
        if (c.username) setUser(c.username);
        if (c.remembered) {
          setSavedUser(c.username);
          setRemembered(true);
        }
      })
      .catch(() => {});
  }, []);

  // Poll GitHub while a device flow is active.
  useEffect(() => {
    if (!flow) return;
    let wait = Math.max(flow.interval, 3) * 1000;
    let dead = false;
    let timer: number;
    const tick = async () => {
      if (dead) return;
      try {
        const r = await api.githubDevicePoll(flow.device_code);
        if (dead) return;
        if (r.status === "authorized" && r.token) {
          try {
            await api.setCredentials(null, r.token, rememberRef.current);
            const u = await api.githubUser().catch(() => null);
            await api.setCredentials(u?.login ?? null, r.token, rememberRef.current);
            onSaved();
            onClose();
            return;
          } catch (e) {
            setFlowMsg(String(e));
            setFlow(null);
            return;
          }
        }
        if (r.status === "expired") {
          setFlowMsg("That code expired — start sign-in again.");
          setFlow(null);
          return;
        }
        if (r.status === "slow_down") wait += 5000;
        timer = window.setTimeout(tick, wait);
      } catch (e) {
        if (!dead) {
          setFlowMsg(String(e));
          setFlow(null);
        }
      }
    };
    timer = window.setTimeout(tick, wait);
    return () => {
      dead = true;
      window.clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [flow]);

  const startGithub = async () => {
    setFlowMsg(null);
    try {
      const f = await api.githubDeviceStart();
      setFlow(f);
      api.openExternalUrl(f.verification_uri).catch(() => {});
    } catch (e) {
      const m = String(e);
      if (m.includes("NO_CLIENT_ID")) {
        // No OAuth app configured → fall back to the token page.
        api.openExternalUrl(GITHUB_TOKEN_URL).catch((err) => onError(String(err)));
        setFlowMsg("OAuth not configured in this build — create a token on the opened page and paste it below.");
      } else {
        setFlowMsg(m);
      }
    }
  };

  const save = async () => {
    try {
      await api.setCredentials(user || null, pass || null, remember);
      onSaved();
      onClose();
    } catch (e) {
      onError(String(e));
    }
  };

  const forget = async () => {
    try {
      await api.clearCredentials();
      setSavedUser(null);
      setRemembered(false);
      setPass("");
    } catch (e) {
      onError(String(e));
    }
  };

  return (
    <Modal title="Sign in" onClose={onClose}>
      <p className="muted small">
        Used for fetch/push/clone over HTTPS and the GitHub API (pull requests, issues, CI status).
      </p>
      {flow ? (
        <div className="device-flow">
          <div className="muted small">Enter this code at <b>{flow.verification_uri.replace(/^https:\/\//, "")}</b>:</div>
          <div className="device-code">{flow.user_code}</div>
          <div className="input-row">
            <button className="btn grow" onClick={() => navigator.clipboard.writeText(flow.user_code)}>
              <Icon name="copy" size={13} /> Copy code
            </button>
            <button className="btn grow" onClick={() => api.openExternalUrl(flow.verification_uri).catch((e) => onError(String(e)))}>
              <Icon name="globe" size={13} /> Open GitHub
            </button>
          </div>
          <div className="muted small">Waiting for authorization…</div>
        </div>
      ) : (
        <button className="btn signin-btn" onClick={startGithub}>
          <Icon name="globe" size={14} /> Sign in with GitHub
        </button>
      )}
      {flowMsg && <p className="muted small">{flowMsg}</p>}
      {remembered && (
        <p className="muted small">
          Remembered credentials{savedUser ? <> for <b>{savedUser}</b></> : ""} are being reused.{" "}
          <button className="link-btn" onClick={forget}>Forget</button>
        </p>
      )}
      <Field label="Username">
        <input className="input" autoFocus={!flow} value={user} onChange={(e) => setUser(e.target.value)} autoComplete="off" />
      </Field>
      <Field label="Personal access token">
        <input className="input" type="password" value={pass} onChange={(e) => setPass(e.target.value)} autoComplete="off" />
      </Field>
      <label className="amend-row">
        <input type="checkbox" className="cb" checked={remember} onChange={(e) => setRemember(e.target.checked)} />
        <span>Remember me — store credentials in the OS keychain</span>
      </label>
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" disabled={!user.trim() || !pass} onClick={save}>Save &amp; continue</button>
      </div>
    </Modal>
  );
}

/* ---------------- GitHub: shared ---------------- */
function useGithub(onError: (m: string) => void) {
  const [gh, setGh] = useState<GhRepo | null>(null);
  const [checked, setChecked] = useState(false);
  useEffect(() => {
    api.githubRepo()
      .then((r) => {
        setGh(r);
        setChecked(true);
      })
      .catch((e) => {
        setChecked(true);
        onError(String(e));
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return { gh, checked };
}

function CiBadge({ sha }: { sha: string }) {
  const [st, setSt] = useState<GhCheckStatus | null>(null);
  useEffect(() => {
    api.githubCheckRuns(sha).then(setSt).catch(() => {});
  }, [sha]);
  if (!st || st.status === "none") return null;
  const label = st.status === "failure" ? `${st.failed} check${st.failed > 1 ? "s" : ""} failed` : st.status;
  return (
    <span className={`ci-badge ${st.status}`} title={`CI: ${label}`}>
      <Icon name={st.status === "success" ? "check" : st.status === "failure" ? "x" : "dot"} size={10} />
      {st.status === "pending" ? "pending" : `${st.total - st.failed}/${st.total}`}
    </span>
  );
}

function NotGithub({ onClose }: { onClose: () => void }) {
  return (
    <>
      <p className="muted small">This repository has no GitHub remote — pull requests, issues and CI status only work for repos hosted on github.com.</p>
      <div className="modal-actions"><button className="btn" onClick={onClose}>Close</button></div>
    </>
  );
}

function NoToken({ onClose, onSignIn }: { onClose: () => void; onSignIn: () => void }) {
  return (
    <>
      <p className="muted small">Sign in to GitHub to see pull requests, issues and CI status.</p>
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" onClick={onSignIn}>Sign in</button>
      </div>
    </>
  );
}

/* ---------------- Pull requests ---------------- */
export function PullRequestsDialog({ onClose, onError, onCheckout, onSignIn }: { onClose: () => void; onError: (m: string) => void; onCheckout: (branch: string) => void; onSignIn: () => void }) {
  const { gh, checked } = useGithub(onError);
  const [prs, setPrs] = useState<GhPR[] | null>(null);
  const [noToken, setNoToken] = useState(false);
  const [busyN, setBusyN] = useState<number | null>(null);

  useEffect(() => {
    if (!gh) return;
    api.githubPrs()
      .then(setPrs)
      .catch((e) => {
        if (String(e).includes("NO_TOKEN")) setNoToken(true);
        else onError(String(e));
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gh]);

  const checkout = async (pr: GhPR) => {
    setBusyN(pr.number);
    try {
      const branch = await api.fetchPrBranch(pr.number);
      onCheckout(branch);
      onClose();
    } catch (e) {
      onError(String(e));
      setBusyN(null);
    }
  };

  return (
    <Modal title={`Pull requests${gh ? ` — ${gh.owner}/${gh.name}` : ""}`} onClose={onClose} width={560}>
      {!checked ? null : !gh ? <NotGithub onClose={onClose} /> : noToken ? <NoToken onClose={onClose} onSignIn={onSignIn} /> : (
        <>
          <div className="modal-list">
            {prs === null && <div className="list-empty small-pad"><p className="muted">Loading…</p></div>}
            {prs?.length === 0 && <div className="list-empty small-pad"><p className="muted">No open pull requests</p></div>}
            {prs?.map((pr) => (
              <div key={pr.number} className="list-row">
                <Avatar name={pr.author} email="" url={pr.avatar_url} size={22} />
                <div className="grow ellipsis">
                  <div className="ellipsis">
                    <span className="muted">#{pr.number}</span> {pr.title}
                    {pr.draft && <span className="badge">draft</span>}
                  </div>
                  <div className="muted small ellipsis">{pr.author} · {pr.head_ref} → {pr.base_ref} · {timeAgo(Date.parse(pr.updated_at) / 1000)}</div>
                </div>
                <CiBadge sha={pr.head_sha} />
                <button className="btn small" disabled={busyN !== null} onClick={() => checkout(pr)}>
                  {busyN === pr.number ? "Fetching…" : "Checkout"}
                </button>
                <button className="icon-btn" title="Open on GitHub" onClick={() => api.openExternalUrl(pr.html_url).catch((e) => onError(String(e)))}>
                  <Icon name="globe" size={13} />
                </button>
              </div>
            ))}
          </div>
          <div className="modal-actions">
            <button className="btn" onClick={() => api.openExternalUrl(`${gh.url}/pulls`).catch((e) => onError(String(e)))}>
              <Icon name="globe" size={13} /> View all on GitHub
            </button>
            <span className="grow" />
            <button className="btn" onClick={onClose}>Close</button>
          </div>
        </>
      )}
    </Modal>
  );
}

/* ---------------- Issues ---------------- */
export function IssuesDialog({ onClose, onError, onSignIn }: { onClose: () => void; onError: (m: string) => void; onSignIn: () => void }) {
  const { gh, checked } = useGithub(onError);
  const [issues, setIssues] = useState<GhIssue[] | null>(null);
  const [noToken, setNoToken] = useState(false);

  useEffect(() => {
    if (!gh) return;
    api.githubIssues()
      .then(setIssues)
      .catch((e) => {
        if (String(e).includes("NO_TOKEN")) setNoToken(true);
        else onError(String(e));
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gh]);

  return (
    <Modal title={`Issues${gh ? ` — ${gh.owner}/${gh.name}` : ""}`} onClose={onClose} width={560}>
      {!checked ? null : !gh ? <NotGithub onClose={onClose} /> : noToken ? <NoToken onClose={onClose} onSignIn={onSignIn} /> : (
        <>
          <div className="modal-list">
            {issues === null && <div className="list-empty small-pad"><p className="muted">Loading…</p></div>}
            {issues?.length === 0 && <div className="list-empty small-pad"><p className="muted">No open issues</p></div>}
            {issues?.map((i) => (
              <div key={i.number} className="list-row">
                <Avatar name={i.author} email="" url={i.avatar_url} size={22} />
                <div className="grow ellipsis">
                  <div className="ellipsis"><span className="muted">#{i.number}</span> {i.title}</div>
                  <div className="muted small ellipsis">
                    {i.author} · {timeAgo(Date.parse(i.created_at) / 1000)} · {i.comments} comment{i.comments === 1 ? "" : "s"}
                  </div>
                </div>
                {i.labels.slice(0, 3).map((l) => (
                  <span key={l.name} className="label-chip" style={{ borderColor: `#${l.color}`, color: `#${l.color}` }}>{l.name}</span>
                ))}
                <button className="icon-btn" title="Open on GitHub" onClick={() => api.openExternalUrl(i.html_url).catch((e) => onError(String(e)))}>
                  <Icon name="globe" size={13} />
                </button>
              </div>
            ))}
          </div>
          <div className="modal-actions">
            <button className="btn" onClick={() => api.openExternalUrl(`${gh.url}/issues`).catch((e) => onError(String(e)))}>
              <Icon name="globe" size={13} /> View all on GitHub
            </button>
            <button className="btn" onClick={() => api.openExternalUrl(`${gh.url}/issues/new`).catch((e) => onError(String(e)))}>
              <Icon name="plus" size={13} /> New issue
            </button>
            <span className="grow" />
            <button className="btn" onClick={onClose}>Close</button>
          </div>
        </>
      )}
    </Modal>
  );
}

/* ---------------- SSH keys ---------------- */
export function SshKeyDialog({ onClose, onLoaded, onError }: { onClose: () => void; onLoaded: () => void; onError: (m: string) => void }) {
  const [keys, setKeys] = useState<string[]>([]);
  const [path, setPath] = useState("");
  const [pass, setPass] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.listSshKeys().then((k) => { setKeys(k); if (k.length) setPath((p) => p || k[0]); }).catch(() => {});
  }, []);

  const pickKey = async () => {
    const p = await openDialog({ title: "Select SSH private key" });
    if (typeof p === "string") setPath(p);
  };

  const load = async () => {
    if (!path.trim()) return;
    setBusy(true);
    try {
      await api.loadSshKey(path.trim(), pass);
      onLoaded();
      onClose();
    } catch (e) {
      onError(String(e));
      setBusy(false);
    }
  };

  return (
    <Modal title="Load SSH key into agent" onClose={onClose} width={470}>
      <p className="hint">
        SSH keys with a passphrase must be loaded into your running <code>ssh-agent</code> — Git Crackit
        uses the system agent and never prompts on a terminal. Pick a key, enter its passphrase
        (leave empty if the key has none), and it stays loaded until the agent restarts.
      </p>
      <Field label="Private key">
        <div className="input-row">
          <input className="input grow" autoFocus list="gc-ssh-keys" placeholder="~/.ssh/id_ed25519" value={path} onChange={(e) => setPath(e.target.value)} />
          <datalist id="gc-ssh-keys">{keys.map((k) => <option key={k} value={k} />)}</datalist>
          <button className="btn" onClick={pickKey}>Browse…</button>
        </div>
        {keys.length > 0 && <div className="hint">Detected {keys.length} key{keys.length > 1 ? "s" : ""} under ~/.ssh</div>}
      </Field>
      <Field label="Passphrase">
        <input className="input" type="password" placeholder="leave empty for unencrypted keys" value={pass}
          onChange={(e) => setPass(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") load(); }} />
      </Field>
      <div className="modal-actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" disabled={busy || !path.trim()} onClick={load}>
          {busy ? "Loading…" : "Load into agent"}
        </button>
      </div>
    </Modal>
  );
}

/* ---------------- About ---------------- */
export function AboutDialog({ onClose, onError }: { onClose: () => void; onError: (m: string) => void }) {
  const [version, setVersion] = useState("…");
  useEffect(() => {
    import("@tauri-apps/api/app").then((m) => m.getVersion()).then(setVersion).catch(() => setVersion("0.1.0"));
  }, []);
  const open = (url: string) => api.openExternalUrl(url).catch((e) => onError(String(e)));
  return (
    <Modal title="About Git Crackit" onClose={onClose} width={480}>
      <div className="about-head">
        <div className="about-mark"><Icon name="branch" size={22} /></div>
        <div>
          <div className="about-name">Git Crackit</div>
          <div className="muted small">v{version} · Tauri 2 + Rust (git2) + React</div>
        </div>
      </div>
      <p className="hint">
        A fast, focused Git GUI — GitKraken-style power at a fraction of the weight,
        with a visual identity of its own.
      </p>
      <div className="about-cols">
        <div>
          <div className="settings-title">Repositories</div>
          <ul className="about-list">
            <li>Open, init &amp; clone — HTTPS and SSH remotes</li>
            <li>Live lane-graph history, search, commit details</li>
            <li>Drag-drop cherry-pick / move / squash (merges preserved)</li>
            <li>Stashes with browsable files &amp; diffs, tags, remotes</li>
            <li>Submodules, blame, image diffs, Git LFS detection</li>
          </ul>
        </div>
        <div>
          <div className="settings-title">GitHub &amp; extras</div>
          <ul className="about-list">
            <li>PRs + CI status + one-click checkout, issues, avatars</li>
            <li>OAuth device flow or token; OS-keychain credentials</li>
            <li>AI commit messages (Claude / OpenAI / Groq / Ollama) —
              auto-picks the cheapest model, no choosing needed</li>
            <li>Auto-updates, command palette, light &amp; dark themes</li>
          </ul>
        </div>
      </div>
      <p className="hint">
        AI is used <b>only</b> to draft commit messages from your staged diff — nothing else.
        API keys live in your OS keychain on this device; they are never sent anywhere except
        the chosen provider, and only when you click ✦.
      </p>
      <div className="modal-actions">
        <button className="btn" onClick={() => open("https://github.com/saimsajidirl/git-crackit")}>
          <Icon name="globe" size={13} /> Repository
        </button>
        <button className="btn" onClick={() => open("https://github.com/saimsajidirl/git-crackit/issues")}>
          <Icon name="alert" size={13} /> Report an issue
        </button>
        <span className="grow" />
        <button className="btn primary" onClick={onClose}>Close</button>
      </div>
    </Modal>
  );
}

/* ---------------- File history ---------------- */

export function FileHistoryDialog({
  path, onClose, onError,
}: { path: string; onClose: () => void; onError: (m: string) => void }) {
  const [commits, setCommits] = useState<CommitInfo[] | null>(null);
  const [selOid, setSelOid] = useState<string | null>(null);
  const [diff, setDiff] = useState<FileDiff | null>(null);

  useEffect(() => {
    setCommits(null);
    setDiff(null);
    setSelOid(null);
    api.fileHistory(path).then(setCommits).catch((e) => { onError(String(e)); onClose(); });
  }, [path]);

  const open = (oid: string) => {
    setSelOid(oid);
    api.getCommitFileDiff(oid, path).then(setDiff).catch((e) => { setDiff(null); onError(String(e)); });
  };

  return (
    <Modal title={`File history — ${path}`} onClose={onClose} width={840}>
      {commits === null ? (
        <p className="muted small">Loading…</p>
      ) : commits.length === 0 ? (
        <p className="muted small">No commits touch this file.</p>
      ) : (
        <div className="modal-list" style={{ maxHeight: "26vh" }}>
          {commits.map((c) => (
            <button
              key={c.oid}
              className={cx("list-row fh-row", selOid === c.oid && "selected")}
              onClick={() => open(c.oid)}
            >
              <code className="fh-oid">{c.short_id}</code>
              <span className="ellipsis grow">{c.summary}</span>
              <span className="muted small">{c.author_name}</span>
              <span className="muted small">{timeAgo(c.author_time)}</span>
            </button>
          ))}
        </div>
      )}
      {diff && (
        <div className="modal-diff">
          <DiffView diff={diff} />
        </div>
      )}
    </Modal>
  );
}

/* ---------------- Branch compare ---------------- */

export function CompareDialog({
  branches, head, onClose, onError,
}: {
  branches: BranchInfo[]; head: string;
  onClose: () => void; onError: (m: string) => void;
}) {
  const [base, setBase] = useState(head);
  const [compare, setCompare] = useState("");
  const [result, setResult] = useState<CompareResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [selPath, setSelPath] = useState<string | null>(null);
  const [diff, setDiff] = useState<FileDiff | null>(null);

  const runCompare = async () => {
    if (!base || !compare) return;
    setBusy(true);
    setDiff(null);
    setSelPath(null);
    try {
      setResult(await api.compareRefs(base, compare));
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const openFile = (p: string) => {
    setSelPath(p);
    api.compareFileDiff(base, compare, p).then(setDiff).catch((e) => { setDiff(null); onError(String(e)); });
  };

  const opts = (list: BranchInfo[]) => (
    <>
      <optgroup label="Local">
        {list.filter((b) => !b.is_remote).map((b) => <option key={b.name} value={b.name}>{b.name}</option>)}
      </optgroup>
      <optgroup label="Remote">
        {list.filter((b) => b.is_remote).map((b) => <option key={b.name} value={b.name}>{b.name}</option>)}
      </optgroup>
    </>
  );

  return (
    <Modal title="Compare branches" onClose={onClose} width={840}>
      <div className="compare-bar">
        <select className="input" value={base} onChange={(e) => { setBase(e.target.value); setResult(null); }}>
          {opts(branches)}
        </select>
        <span className="muted">…</span>
        <select className="input" value={compare} onChange={(e) => { setCompare(e.target.value); setResult(null); }}>
          <option value="">Compare against…</option>
          {opts(branches)}
        </select>
        <button className="btn primary" disabled={!base || !compare || base === compare || busy} onClick={runCompare}>
          {busy ? "Comparing…" : "Compare"}
        </button>
      </div>
      {result && (
        <>
          <p className="muted small">
            <code>{compare}</code> is {result.ahead} ahead, {result.behind} behind <code>{base}</code>
            {result.files.length === 0 ? " — no file changes" : ""}
          </p>
          {result.files.length > 0 && (
            <div className="modal-list" style={{ maxHeight: "26vh" }}>
              {result.files.map((f) => (
                <button
                  key={f.path}
                  className={cx("list-row fh-row", selPath === f.path && "selected")}
                  onClick={() => openFile(f.path)}
                >
                  <span className={`status-badge st-${f.status}`}>{statusLetter(f.status)}</span>
                  <span className="ellipsis grow" title={f.path}>
                    {fileName(f.path)} <span className="muted small">{dirName(f.path)}</span>
                  </span>
                  <span className="stat-add small">+{f.additions}</span>
                  <span className="stat-del small">−{f.deletions}</span>
                </button>
              ))}
            </div>
          )}
        </>
      )}
      {diff && (
        <div className="modal-diff">
          <DiffView diff={diff} />
        </div>
      )}
    </Modal>
  );
}
