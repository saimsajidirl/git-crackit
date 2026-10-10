import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open as openDirDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import type {
  RepoInfo, FileChange, CommitInfo, CommitDetail, FileDiff, BranchInfo,
  MergeResult, BlameHunkInfo, RecentRepos, StashInfo, StashFileInfo,
} from "./types";
import { Toolbar } from "./components/Toolbar";
import { Welcome } from "./components/Welcome";
import { ChangesPanel, FileSelection } from "./components/ChangesPanel";
import { HistoryPanel } from "./components/HistoryPanel";
import { CommitDetailView } from "./components/CommitDetail";
import { DiffView } from "./components/DiffView";
import { ContextMenu, MenuItem } from "./components/ContextMenu";
import { Icon } from "./icons";
import { applyTheme, cx, initialTheme, modKey, type Theme } from "./util";
import { CommandPalette, type Command } from "./components/CommandPalette";
import {
  CloneDialog, InitDialog, ConfirmDialog, NewBranchDialog, PickBranchDialog,
  RenameBranchDialog, StashDialog, TagsDialog, RemotesDialog, SettingsDialog,
  CredentialsDialog, SubmodulesDialog, PullRequestsDialog, IssuesDialog,
} from "./components/Dialogs";

type Dialog =
  | { kind: "clone" }
  | { kind: "init" }
  | { kind: "newBranch"; base?: string }
  | { kind: "merge" }
  | { kind: "rebase" }
  | { kind: "rename"; branch: string }
  | { kind: "stash" }
  | { kind: "tags"; target?: string }
  | { kind: "remotes" }
  | { kind: "submodules" }
  | { kind: "prs" }
  | { kind: "issues" }
  | { kind: "settings" }
  | { kind: "credentials" }
  | { kind: "confirm"; title: string; message: React.ReactNode; confirmLabel?: string; danger?: boolean; onConfirm: () => void };

const PAGE = 300;

function normalCount(changes: FileChange[]): number {
  return changes.filter((c) => !c.conflicted).length;
}

export default function App() {
  const [repo, setRepo] = useState<RepoInfo | null>(null);
  const [recent, setRecent] = useState<RecentRepos>({ recent: [], last: null });
  const [tab, setTab] = useState<"changes" | "history">("changes");

  const [changes, setChanges] = useState<FileChange[]>([]);
  const [selFile, setSelFile] = useState<FileSelection | null>(null);
  const [workDiff, setWorkDiff] = useState<FileDiff | null>(null);
  const [stashes, setStashes] = useState<StashInfo[]>([]);
  const [expandedStash, setExpandedStash] = useState<number | null>(null);
  const [stashFilesMap, setStashFilesMap] = useState<Record<number, StashFileInfo[]>>({});
  const [stashSel, setStashSel] = useState<{ index: number; file: string } | null>(null);
  const [stashDiff, setStashDiff] = useState<FileDiff | null>(null);

  const [commits, setCommits] = useState<CommitInfo[]>([]);
  const [query, setQuery] = useState("");
  const [hasMore, setHasMore] = useState(true);
  const [selCommitOid, setSelCommitOid] = useState<string | null>(null);
  const [detail, setDetail] = useState<CommitDetail | null>(null);

  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [blame, setBlame] = useState<{ path: string; hunks: BlameHunkInfo[] } | null>(null);

  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const pendingRetry = useRef<(() => void) | null>(null);
  const [theme, setTheme] = useState<Theme>(initialTheme);
  const [palette, setPalette] = useState(false);

  useEffect(() => applyTheme(theme), [theme]);
  useEffect(() => {
    if (!notice) return;
    const t = window.setTimeout(() => setNotice(null), 4000);
    return () => window.clearTimeout(t);
  }, [notice]);

  /* ---------- data loading ---------- */

  const loadRecent = useCallback(() => {
    api.getRecentRepositories().then(setRecent).catch(() => {});
  }, []);

  const refreshInfo = useCallback(async () => {
    setRepo(await api.repositoryInfo().catch(() => null));
  }, []);

  const refreshStatus = useCallback(async () => {
    try {
      setChanges(await api.getStatus());
    } catch {
      setChanges([]);
    }
  }, []);

  const refreshBranches = useCallback(async () => {
    try {
      setBranches(await api.listBranches());
    } catch {
      setBranches([]);
    }
  }, []);

  const loadHistory = useCallback(async (q: string, reset: boolean) => {
    try {
      const skip = reset ? 0 : undefined;
      const list = await api.getHistory(skip, PAGE, q || undefined);
      setHasMore(list.length >= PAGE);
      if (reset) setCommits(list);
      else setCommits((prev) => [...prev, ...list]);
    } catch {
      if (reset) setCommits([]);
    }
  }, []);

  const loadMore = useCallback(async () => {
    const list = await api.getHistory(commits.length, PAGE, query || undefined);
    setHasMore(list.length >= PAGE);
    setCommits((prev) => [...prev, ...list]);
  }, [commits.length, query]);

  const expandedStashRef = useRef<number | null>(null);
  expandedStashRef.current = expandedStash;
  const stashSelRef = useRef<{ index: number; file: string } | null>(null);
  stashSelRef.current = stashSel;

  const loadStashFiles = useCallback((index: number) => {
    api.stashFiles(index)
      .then((files) => setStashFilesMap((m) => ({ ...m, [index]: files })))
      .catch((e) => {
        // Keep the row expanded but stop the spinner and surface the real error.
        setStashFilesMap((m) => ({ ...m, [index]: [] }));
        setError(String(e));
      });
  }, []);

  const loadStashes = useCallback(async () => {
    let list: StashInfo[] = [];
    try {
      list = await api.listStashes();
    } catch { /* no repo */ }
    setStashes(list);
    setStashFilesMap({});
    // Collapse only when the stash actually went away; keep UI state otherwise.
    const exp = expandedStashRef.current;
    if (exp !== null && list.some((s) => s.index === exp)) loadStashFiles(exp);
    else if (exp !== null) setExpandedStash(null);
    const sel = stashSelRef.current;
    if (sel && !list.some((s) => s.index === sel.index)) {
      setStashSel(null);
      setStashDiff(null);
    }
  }, [loadStashFiles]);

  const refreshAll = useCallback(async () => {
    await Promise.all([refreshInfo(), refreshStatus(), refreshBranches(), loadHistory(query, true), loadStashes()]);
    if (selFile) {
      api.getWorkingDiff(selFile.path, selFile.staged).then(setWorkDiff).catch(() => setWorkDiff(null));
    }
  }, [refreshInfo, refreshStatus, refreshBranches, loadHistory, loadStashes, query, selFile]);

  /* ---------- op runner ---------- */

  const run = useCallback(
    async (label: string, fn: () => Promise<unknown>, opts?: { refresh?: boolean }) => {
      setBusy(label);
      setError(null);
      try {
        const r = await fn();
        if (r && typeof r === "object" && "status" in (r as object)) {
          const mr = r as MergeResult;
          if (mr.status === "conflicts") {
            setNotice(`Conflicts in ${mr.conflicts.length} file(s). Resolve them, stage the files, then commit — or abort the operation.`);
          } else if (mr.status === "up_to_date") {
            setNotice("Already up to date.");
          } else if (mr.status === "fast_forward") {
            setNotice("Fast-forwarded.");
          } else if (mr.status === "merged") {
            setNotice("Done — changes committed.");
          }
        }
        if (opts?.refresh !== false) await refreshAll();
      } catch (e) {
        const m = String(e);
        if (m.startsWith("AUTH:")) {
          pendingRetry.current = () => run(label, fn, opts);
          setDialog({ kind: "credentials" });
          setError(null);
        } else {
          setError(m);
        }
      } finally {
        setBusy(null);
      }
    },
    [refreshAll]
  );

  /* ---------- repo open ---------- */

  const openPath = useCallback(async (path: string) => {
    setError(null);
    try {
      const info = await api.openRepository(path);
      setRepo(info);
      setSelFile(null);
      setSelCommitOid(null);
      setDetail(null);
      setBlame(null);
      setTab("changes");
      setQuery("");
      await refreshAll();
      loadRecent();
    } catch (e) {
      setError(String(e));
    }
  }, [refreshAll, loadRecent]);

  const openViaDialog = useCallback(async () => {
    const p = await openDirDialog({ directory: true, title: "Open a Git repository" });
    if (typeof p === "string") openPath(p);
  }, [openPath]);

  /* ---------- live refresh: fs watcher events + window focus + F5 ---------- */

  const selFileRef = useRef<FileSelection | null>(null);
  selFileRef.current = selFile;

  useEffect(() => {
    let timer: number | undefined;
    const un = listen<string>("repo-changed", (e) => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        if (e.payload === "git") {
          refreshAll();
        } else {
          refreshStatus();
          const sf = selFileRef.current;
          if (sf) {
            api.getWorkingDiff(sf.path, sf.staged).then(setWorkDiff).catch(() => setWorkDiff(null));
          }
        }
      }, 60);
    });
    const onFocus = () => {
      refreshInfo();
      refreshStatus();
    };
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (e.key === "F5" || (mod && e.key.toLowerCase() === "r")) {
        e.preventDefault();
        refreshAll();
      } else if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette((v) => !v);
      } else if (mod && e.key === "1") {
        e.preventDefault();
        setTab("changes");
      } else if (mod && e.key === "2") {
        e.preventDefault();
        setTab("history");
      }
    };
    window.addEventListener("focus", onFocus);
    window.addEventListener("keydown", onKey);
    return () => {
      un.then((f) => f());
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("keydown", onKey);
    };
  }, [refreshAll, refreshStatus, refreshInfo]);

  /* ---------- boot ---------- */

  useEffect(() => {
    (async () => {
      const r = await api.getRecentRepositories().catch(() => null);
      if (r) {
        setRecent(r);
        if (r.last) {
          api.openRepository(r.last).then(async (info) => {
            setRepo(info);
            await refreshAll();
          }).catch(() => {});
        }
      }
      // No remembered credentials → offer sign-in (asked again each launch until saved).
      const creds = await api.getCredentials().catch(() => null);
      if (creds && !creds.has_password) setDialog({ kind: "credentials" });
      // Silent update check on launch.
      checkUpdates(false);
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /* ---------- selections ---------- */

  const selectFile = useCallback((sel: FileSelection) => {
    setSelFile(sel);
    setStashSel(null);
    setBlame(null);
    api.getWorkingDiff(sel.path, sel.staged).then(setWorkDiff).catch((e) => setError(String(e)));
  }, []);

  const toggleStash = useCallback((index: number) => {
    setExpandedStash((cur) => (cur === index ? null : index));
    loadStashFiles(index);
  }, [loadStashFiles]);

  const selectStashFile = useCallback((index: number, path: string) => {
    setSelFile(null);
    setBlame(null);
    setStashSel({ index, file: path });
    api.stashFileDiff(index, path).then(setStashDiff).catch((e) => {
      setStashDiff(null);
      setError(String(e));
    });
  }, []);

  const selectCommit = useCallback((oid: string) => {
    setSelCommitOid(oid);
    setBlame(null);
    api.getCommitDetail(oid).then(setDetail).catch((e) => setError(String(e)));
  }, []);

  /* ---------- actions ---------- */

  const toggleStage = (f: FileChange, stage: boolean) =>
    run(stage ? "Staging…" : "Unstaging…", () => (stage ? api.stageFiles([f.path]) : api.unstageFiles([f.path])));

  const toggleAll = (stage: boolean) =>
    run(stage ? "Staging all…" : "Unstaging all…", () => (stage ? api.stageAll() : api.unstageAll()));

  const commit = (message: string, _d: string, amend: boolean) =>
    run("Committing…", () => api.createCommit(message, amend));

  const checkout = (name: string) => run(`Checking out ${name}…`, () => api.checkoutBranch(name));

  const fetchOp = () => run("Fetching…", () => api.fetchRemote());
  const pullOp = () => run("Pulling…", () => api.pull());
  const pushOp = () => run("Pushing…", () => api.push(true));

  const headOid = repo?.head
    ? commits.find((c) => c.refs.some((r) => r.kind === "branch" && r.name === repo.head))?.oid ?? null
    : null;

  /* ---------- context menus ---------- */

  const fileMenu = (e: React.MouseEvent, f: FileChange) => {
    e.preventDefault();
    const items: MenuItem[] = [
      f.staged !== null
        ? { label: "Unstage", onClick: () => run("Unstaging…", () => api.unstageFiles([f.path])) }
        : { label: "Stage", onClick: () => run("Staging…", () => api.stageFiles([f.path])) },
      {
        label: "Discard changes…",
        danger: true,
        onClick: () =>
          setDialog({
            kind: "confirm",
            title: "Discard changes",
            message: <>Discard all changes to <code>{f.path}</code>? This cannot be undone.</>,
            confirmLabel: "Discard",
            danger: true,
            onConfirm: () => run("Discarding…", () => api.discardChanges([f.path])),
          }),
      },
      { label: "", separator: true },
      { label: "View blame", onClick: () => {
          api.getBlame(f.path).then((hunks) => setBlame({ path: f.path, hunks })).catch((err) => setError(String(err)));
          setSelFile({ path: f.path, staged: f.staged !== null });
        } },
      { label: "Add to .gitignore", onClick: () => run("Updating .gitignore…", () => api.ignoreFile(f.path)) },
      { label: "Copy path", onClick: () => navigator.clipboard.writeText(f.path) },
    ];
    setMenu({ x: e.clientX, y: e.clientY, items });
  };

  const stashMenu = (e: React.MouseEvent, s: StashInfo) => {
    e.preventDefault();
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: "Apply stash", onClick: () => run("Applying stash…", () => api.stashApply(s.index)) },
        { label: "Pop stash (apply + drop)", onClick: () => run("Popping stash…", () => api.stashPop(s.index)) },
        { label: "", separator: true },
        {
          label: "Drop stash…",
          danger: true,
          onClick: () =>
            setDialog({
              kind: "confirm",
              title: "Drop stash",
              message: <>Drop <code>{`stash@{${s.index}}`}</code> — "{s.message}"? This cannot be undone.</>,
              confirmLabel: "Drop",
              danger: true,
              onConfirm: () => run("Dropping stash…", () => api.stashDrop(s.index)),
            }),
        },
        { label: "", separator: true },
        { label: "Manage all stashes…", onClick: () => setDialog({ kind: "stash" }) },
      ],
    });
  };

  /** Check the update endpoint; quiet unless `manual`. */
  const checkUpdates = useCallback(async (manual: boolean) => {
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (!update) {
        if (manual) setNotice("You're on the latest version.");
        return;
      }
      setDialog({
        kind: "confirm",
        title: `Update available — v${update.version}`,
        message: (
          <>
            {update.body && <p className="muted small" style={{ whiteSpace: "pre-wrap", marginBottom: 8 }}>{update.body}</p>}
            Download and install v{update.version}? The app will restart.
          </>
        ),
        confirmLabel: "Update & restart",
        onConfirm: async () => {
          try {
            setBusy("Downloading update…");
            let downloaded = 0;
            let total = 0;
            await update.downloadAndInstall((e) => {
              if (e.event === "Started" && e.data.contentLength) total = e.data.contentLength;
              else if (e.event === "Progress") {
                downloaded += e.data.chunkLength;
                if (total) setBusy(`Downloading update… ${Math.round((downloaded / total) * 100)}%`);
              }
            });
            const { relaunch } = await import("@tauri-apps/plugin-process");
            await relaunch();
          } catch (e) {
            setError(`Update failed: ${e}`);
          } finally {
            setBusy(null);
          }
        },
      });
    } catch (e) {
      if (manual) setError(`Update check failed: ${e}`);
    }
  }, []);

  const commitMenu = (e: React.MouseEvent, c: CommitInfo) => {
    e.preventDefault();
    const resetItem = (mode: "soft" | "mixed" | "hard"): MenuItem => ({
      label: `Reset to this commit (${mode})${mode === "hard" ? "…" : ""}`,
      danger: mode === "hard",
      onClick: () =>
        mode === "hard"
          ? setDialog({
              kind: "confirm",
              title: "Hard reset",
              message: <>Hard reset <b>{repo?.head}</b> to <code>{c.short_id}</code>? Uncommitted changes will be lost.</>,
              confirmLabel: "Reset",
              danger: true,
              onConfirm: () => run("Resetting…", () => api.resetToCommit(c.oid, "hard")),
            })
          : run("Resetting…", () => api.resetToCommit(c.oid, mode)),
    });
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: "Create branch here…", onClick: () => setDialog({ kind: "newBranch", base: c.oid }) },
        { label: "Create tag here…", onClick: () => setDialog({ kind: "tags", target: c.oid }) },
        { label: "", separator: true },
        { label: "Cherry-pick", onClick: () => run("Cherry-picking…", () => api.cherryPick(c.oid)) },
        { label: "Revert commit", onClick: () => run("Reverting…", () => api.revertCommit(c.oid)) },
        { label: "Checkout commit (detached HEAD)", onClick: () =>
            setDialog({ kind: "confirm", title: "Checkout commit", message: <>Check out <code>{c.short_id}</code> as detached HEAD?</>, confirmLabel: "Checkout", onConfirm: () => run("Checking out…", () => api.checkoutCommit(c.oid)) }) },
        { label: "", separator: true },
        resetItem("soft"), resetItem("mixed"), resetItem("hard"),
        { label: "", separator: true },
        { label: "Copy SHA", onClick: () => navigator.clipboard.writeText(c.oid) },
      ],
    });
  };

  /** Dragging a commit onto another commit: cherry-pick / reorder / squash. */
  const commitDrop = (oid: string, target: string, x: number, y: number) => {
    const c = commits.find((v) => v.oid === oid);
    const t = commits.find((v) => v.oid === target);
    if (!c || !t) return;
    const rewrite = (action: "move_after" | "squash_into", label: string) =>
      setDialog({
        kind: "confirm",
        title: label,
        message: (
          <>
            {label} — this rewrites commits after <code>{t.short_id}</code>. If <b>{repo?.head}</b> is
            already pushed you will need to force-push. Continue?
          </>
        ),
        confirmLabel: label.split(" ")[0],
        onConfirm: () => run("Rewriting history…", () => api.rebaseCommitAction(oid, target, action)),
      });
    setMenu({
      x,
      y,
      items: [
        { label: `Cherry-pick ${c.short_id} onto ${repo?.head ?? "HEAD"}`, onClick: () => run("Cherry-picking…", () => api.cherryPick(oid)) },
        { label: `Move ${c.short_id} after ${t.short_id}…`, onClick: () => rewrite("move_after", `Move commit ${c.short_id}`) },
        { label: `Squash ${c.short_id} into ${t.short_id}…`, onClick: () => rewrite("squash_into", `Squash commit ${c.short_id}`) },
      ],
    });
  };

  const branchMenu = (e: React.MouseEvent, b: BranchInfo) => {
    e.preventDefault();
    const items: MenuItem[] = [];
    if (!b.is_head && !b.is_remote) items.push({ label: `Checkout ${b.name}`, onClick: () => checkout(b.name) });
    items.push({
      label: `Merge ${b.name} into ${repo?.head}`,
      onClick: () => run(`Merging ${b.name}…`, () => api.mergeBranch(b.name)),
    });
    if (!b.is_remote) {
      items.push({ label: `Rebase ${repo?.head} onto ${b.name}`, onClick: () => run("Rebasing…", () => api.rebaseBranch(b.name)) });
      items.push({ label: `Rename ${b.name}…`, onClick: () => setDialog({ kind: "rename", branch: b.name }) });
      items.push({
        label: `Delete ${b.name}…`, danger: true, disabled: b.is_head,
        onClick: () => setDialog({
          kind: "confirm", title: "Delete branch",
          message: <>Delete branch <b>{b.name}</b>? Commits not reachable elsewhere may be lost.</>,
          confirmLabel: "Delete", danger: true,
          onConfirm: () => run("Deleting…", () => api.deleteBranch(b.name)),
        }),
      });
    } else {
      items.push({
        label: `Checkout ${b.name} (create local)`, onClick: () => checkout(b.name),
      });
      items.push({
        label: `Delete remote branch ${b.name}…`, danger: true,
        onClick: () => setDialog({
          kind: "confirm", title: "Delete remote branch",
          message: <>Delete remote-tracking ref <b>{b.name}</b>? (This removes the local ref only.)</>,
          confirmLabel: "Delete", danger: true,
          onConfirm: () => run("Deleting…", () => api.deleteBranch(b.name)),
        }),
      });
    }
    setMenu({ x: e.clientX, y: e.clientY, items });
  };

  const commands: Command[] = useMemo(() => {
    const list: Command[] = [];
    const add = (group: string, label: string, run: () => void, icon?: string, hint?: string) =>
      list.push({ id: `${group}:${label}`, group, label, run, icon, hint });
    add("Repository", "Open local repository…", openViaDialog, "folder");
    add("Repository", "Clone repository…", () => setDialog({ kind: "clone" }), "download");
    add("Repository", "Create new repository…", () => setDialog({ kind: "init" }), "plus");
    for (const r of recent.recent) add("Recent", r, () => openPath(r), "repo");
    if (repo) {
      add("Navigate", "Show changes", () => setTab("changes"), "diff", `${modKey}1`);
      add("Navigate", "Show history", () => setTab("history"), "history", `${modKey}2`);
      add("Remote", "Fetch", fetchOp, "sync");
      add("Remote", "Fetch all remotes", () => run("Fetching all…", () => api.fetchAll()), "sync");
      add("Remote", "Pull", pullOp, "arrowDown");
      add("Remote", "Push", pushOp, "arrowUp");
      add("Branch", "New branch…", () => setDialog({ kind: "newBranch" }), "plus");
      add("Branch", `Merge into ${repo.head ?? "HEAD"}…`, () => setDialog({ kind: "merge" }), "merge");
      add("Branch", "Rebase current branch…", () => setDialog({ kind: "rebase" }), "merge");
      add("Branch", "Stashes…", () => setDialog({ kind: "stash" }), "stash");
      add("Branch", "Tags…", () => setDialog({ kind: "tags" }), "tag");
      for (const b of branches) {
        if (!b.is_head && !b.is_remote) add("Checkout", b.name, () => checkout(b.name), "branch");
      }
      add("Settings", "Remotes…", () => setDialog({ kind: "remotes" }), "globe");
      add("GitHub", "Pull requests…", () => setDialog({ kind: "prs" }), "merge");
      add("GitHub", "Issues…", () => setDialog({ kind: "issues" }), "alert");
      add("Settings", "Submodules…", () => setDialog({ kind: "submodules" }), "repo");
      add("Settings", "Repository settings…", () => setDialog({ kind: "settings" }), "gear");
      add("Settings", "HTTPS credentials…", () => setDialog({ kind: "credentials" }), "cloud");
      add("Settings", "Refresh", refreshAll, "sync", "F5");
      add("App", "Check for updates…", () => checkUpdates(true), "download");
    }
    add("Appearance", theme === "dark" ? "Switch to light theme" : "Switch to dark theme",
      () => setTheme((t) => (t === "dark" ? "light" : "dark")), theme === "dark" ? "sun" : "moon");
    return list;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repo, branches, recent.recent, theme, refreshAll]);

  /* ---------- render ---------- */

  const headName = repo?.is_detached ? "detached HEAD" : repo?.head ?? "main";

  return (
    <div className="app">
      <Toolbar
        repo={repo}
        branches={branches}
        recent={recent.recent}
        busy={busy}
        onOpenPath={openPath}
        onOpenDialog={openViaDialog}
        onClone={() => setDialog({ kind: "clone" })}
        onInit={() => setDialog({ kind: "init" })}
        onFetch={fetchOp}
        onPull={pullOp}
        onPush={pushOp}
        onCheckout={checkout}
        onNewBranch={() => setDialog({ kind: "newBranch" })}
        onMerge={() => setDialog({ kind: "merge" })}
        onRebase={() => setDialog({ kind: "rebase" })}
        onStashMenu={() => setDialog({ kind: "stash" })}
        onTags={() => setDialog({ kind: "tags" })}
        onRemotes={() => setDialog({ kind: "remotes" })}
        onSettings={() => setDialog({ kind: "settings" })}
        onCredentials={() => setDialog({ kind: "credentials" })}
        onCheckUpdates={() => checkUpdates(true)}
        onPullRequests={() => setDialog({ kind: "prs" })}
        onIssues={() => setDialog({ kind: "issues" })}
        onCloseRepo={() => run("Closing…", async () => { await api.closeRepository(); setRepo(null); setChanges([]); setCommits([]); setDetail(null); setSelFile(null); }, { refresh: false })}
        onRemoveRecent={(p) => { api.removeRecentRepository(p).then(loadRecent); }}
        onBranchContext={branchMenu}
        onFetchAll={() => run("Fetching all…", () => api.fetchAll())}
        onSubmodules={() => setDialog({ kind: "submodules" })}
        theme={theme}
        onToggleTheme={() => setTheme((t) => (t === "dark" ? "light" : "dark"))}
        onCommandPalette={() => setPalette(true)}
      />

      {(error || notice) && (
        <div className="toast-stack" aria-live="polite">
          {error && (
            <div className="toast error" role="alert">
              <Icon name="alert" size={15} />
              <span className="grow">{error}</span>
              <button className="icon-btn" onClick={() => setError(null)}><Icon name="x" size={12} /></button>
            </div>
          )}
          {notice && (
            <div className="toast info" role="status">
              <Icon name="check" size={15} />
              <span className="grow">{notice}</span>
              <button className="icon-btn" onClick={() => setNotice(null)}><Icon name="x" size={12} /></button>
            </div>
          )}
        </div>
      )}
      {repo && repo.uses_lfs && !repo.lfs_installed && (
        <div className="banner warn" role="status">
          <Icon name="alert" size={14} />
          <span className="grow">
            This repository uses <b>Git LFS</b> but <code>git-lfs</code> isn't installed — large files will appear as pointer text files. Install it to fetch real contents.
          </span>
        </div>
      )}
      {repo && repo.state !== "clean" && (
        <div className="banner warn" role="alert">
          <Icon name="alert" size={14} />
          <span className="grow">
            Repository is in <b>{repo.state.replace("_", " ")}</b> state. Resolve conflicts, stage files, and commit to finish.
          </span>
          <button className="btn small" onClick={() => run("Aborting…", () => api.abortMerge())}>Abort</button>
        </div>
      )}

      {!repo ? (
        <Welcome
          recent={recent.recent}
          onOpenPath={openPath}
          onOpenDialog={openViaDialog}
          onClone={() => setDialog({ kind: "clone" })}
          onInit={() => setDialog({ kind: "init" })}
          onRemoveRecent={(p) => { api.removeRecentRepository(p).then(loadRecent); }}
        />
      ) : (
        <div className="main">
          <div className="sidebar">
            <div className="seg" role="tablist">
              <span className={cx("seg-thumb", tab === "history" && "right")} />
              <button role="tab" aria-selected={tab === "changes"} className={cx("seg-btn", tab === "changes" && "active")} onClick={() => setTab("changes")}>
                <Icon name="diff" size={13} /> Changes {changes.length > 0 && <span className="badge blue">{changes.length}</span>}
              </button>
              <button role="tab" aria-selected={tab === "history"} className={cx("seg-btn", tab === "history" && "active")} onClick={() => setTab("history")}>
                <Icon name="history" size={13} /> History
              </button>
            </div>
            {tab === "changes" ? (
              <ChangesPanel
                changes={changes}
                selected={selFile}
                onSelect={selectFile}
                onToggleStage={toggleStage}
                onToggleAll={toggleAll}
                onCommit={commit}
                onDiscard={(paths) => run("Discarding…", () => api.discardChanges(paths))}
                onDiscardAll={() =>
                  setDialog({
                    kind: "confirm",
                    title: "Discard all changes",
                    message: <>Discard all changes in <b>{normalCount(changes)}</b> file(s)? This cannot be undone.</>,
                    confirmLabel: "Discard all",
                    danger: true,
                    onConfirm: () =>
                      run("Discarding…", () => api.discardChanges(changes.map((c) => c.path))),
                  })
                }
                onStashAll={() => setDialog({ kind: "stash" })}
                onContextMenu={fileMenu}
                stashes={stashes}
                expandedStash={expandedStash}
                stashFiles={stashFilesMap}
                stashSel={stashSel}
                onToggleStash={toggleStash}
                onSelectStashFile={selectStashFile}
                onStashContext={stashMenu}
                headName={headName}
                repoState={repo.state}
                isUnborn={repo.is_unborn}
                busy={busy}
              />
            ) : (
              <HistoryPanel
                commits={commits}
                selectedOid={selCommitOid}
                headOid={headOid}
                onSelect={selectCommit}
                onContextMenu={commitMenu}
                onCommitDrop={commitDrop}
                onSearch={(q) => { setQuery(q); loadHistory(q, true); }}
                onLoadMore={loadMore}
                hasMore={hasMore}
              />
            )}
          </div>

          <div className="content">
            {blame ? (
              <div className="blame-view">
                <div className="diff-header">
                  <span className="diff-path">Blame — {blame.path}</span>
                  <button className="btn small" onClick={() => setBlame(null)}>Back to diff</button>
                </div>
                <div className="blame-list">
                  {blame.hunks.map((h, i) => (
                    <div key={i} className="blame-row">
                      <span className="blame-color" style={{ background: `hsl(${(i * 47) % 360} 60% 55%)` }} />
                      <div className="grow">
                        <div className="ellipsis">{h.summary}</div>
                        <div className="muted small">{h.author} · {h.short_id} · lines {h.start_line}–{h.start_line + h.line_count - 1}</div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ) : tab === "changes" ? (
              stashSel ? (
                <div className="stash-diff-wrap">
                  <div className="stash-diff-banner">
                    <Icon name="stash" size={13} />
                    <span>{`stash@{${stashSel.index}} — stashed changes (read-only)`}</span>
                    <button className="btn small" onClick={() => setStashSel(null)}>Back to working tree</button>
                  </div>
                  <DiffView diff={stashDiff} staged={false} />
                </div>
              ) : (
                <DiffView diff={workDiff} staged={selFile?.staged ?? false} />
              )
            ) : (
              <CommitDetailView detail={detail} onError={setError} />
            )}
          </div>
        </div>
      )}

      <div className="statusbar">
        <span className={cx("status-dot", busy && "busy")} />
        {repo && (
          <span className="status-item">
            <Icon name="branch" size={11} /> {repo.is_detached ? "detached" : repo.head ?? "—"}
          </span>
        )}
        <span className="ellipsis faint">{repo ? repo.path : "No repository open"}</span>
        <span className="grow" />
        {busy && <span className="status-item">{busy}</span>}
        <button className="status-item status-btn" onClick={() => setPalette(true)}>
          <kbd>{modKey}</kbd><kbd>K</kbd> Commands
        </button>
      </div>

      {palette && <CommandPalette commands={commands} onClose={() => setPalette(false)} />}

      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}

      {dialog?.kind === "clone" && <CloneDialog onClose={() => setDialog(null)} onDone={() => { refreshAll(); loadRecent(); }} onError={setError} />}
      {dialog?.kind === "init" && <InitDialog onClose={() => setDialog(null)} onDone={() => { refreshAll(); loadRecent(); }} onError={setError} />}
      {dialog?.kind === "newBranch" && (
        <NewBranchDialog
          branches={branches}
          defaultBase={dialog.base ?? ""}
          onClose={() => setDialog(null)}
          onError={setError}
          onCreate={async (name, base, co) => {
            await run("Creating branch…", () => api.createBranch(name, base || undefined, co));
          }}
        />
      )}
      {dialog?.kind === "merge" && (
        <PickBranchDialog
          title={`Merge into ${headName}`}
          actionLabel="Merge"
          branches={branches}
          exclude={repo?.is_detached ? undefined : repo?.head ?? undefined}
          onClose={() => setDialog(null)}
          onError={setError}
          onPick={async (name) => run(`Merging ${name}…`, () => api.mergeBranch(name))}
        />
      )}
      {dialog?.kind === "rebase" && (
        <PickBranchDialog
          title={`Rebase ${headName} onto…`}
          actionLabel="Rebase"
          branches={branches}
          exclude={repo?.is_detached ? undefined : repo?.head ?? undefined}
          onClose={() => setDialog(null)}
          onError={setError}
          onPick={async (name) => run("Rebasing…", () => api.rebaseBranch(name))}
        />
      )}
      {dialog?.kind === "rename" && (
        <RenameBranchDialog
          branch={dialog.branch}
          onClose={() => setDialog(null)}
          onError={setError}
          onRename={async (old, n) => run("Renaming…", () => api.renameBranch(old, n))}
        />
      )}
      {dialog?.kind === "stash" && <StashDialog onClose={() => setDialog(null)} onChanged={refreshAll} onError={setError} />}
      {dialog?.kind === "tags" && <TagsDialog onClose={() => setDialog(null)} onChanged={refreshAll} onError={setError} initialTarget={dialog.target} />}
      {dialog?.kind === "submodules" && <SubmodulesDialog onClose={() => setDialog(null)} onChanged={refreshAll} onError={setError} />}
      {dialog?.kind === "prs" && (
        <PullRequestsDialog
          onClose={() => setDialog(null)}
          onError={setError}
          onCheckout={(b) => run(`Checking out ${b}…`, () => api.checkoutBranch(b))}
          onSignIn={() => setDialog({ kind: "credentials" })}
        />
      )}
      {dialog?.kind === "issues" && (
        <IssuesDialog
          onClose={() => setDialog(null)}
          onError={setError}
          onSignIn={() => setDialog({ kind: "credentials" })}
        />
      )}
      {dialog?.kind === "remotes" && <RemotesDialog onClose={() => setDialog(null)} onChanged={refreshAll} onError={setError} />}
      {dialog?.kind === "settings" && <SettingsDialog onClose={() => setDialog(null)} onError={setError} />}
      {dialog?.kind === "credentials" && (
        <CredentialsDialog
          onClose={() => setDialog(null)}
          onError={setError}
          onSaved={() => {
            const r = pendingRetry.current;
            pendingRetry.current = null;
            r?.();
          }}
        />
      )}
      {dialog?.kind === "confirm" && (
        <ConfirmDialog
          title={dialog.title}
          message={dialog.message}
          confirmLabel={dialog.confirmLabel}
          danger={dialog.danger}
          onClose={() => setDialog(null)}
          onConfirm={dialog.onConfirm}
        />
      )}
    </div>
  );
}
