# Git Crackit

A Git GUI client — GitKraken-style feature set with a GitHub Desktop-style UI
(light theme, blue accent widgets). Built with Tauri 2 + Rust (git2/libgit2) +
React 18 + TypeScript + Vite.

## Commands

- `npm install` — install frontend deps
- `npm run dev` — Vite dev server on :1420 (UI only; tauri IPC needs the app)
- `npm run tauri dev` — run the desktop app in dev mode (hot reload)
- `npm run tauri build` — release build + installers
- `cd src-tauri && cargo check` / `cargo test` — backend checks

## Architecture

- `src-tauri/src/` — Rust backend. `main.rs` registers ~45 `#[tauri::command]`
  functions organized by domain: `cmds_repo.rs` (open/init/clone/recent/creds),
  `cmds_files.rs` (status/stage/discard/commit/diff/blame/config),
  `cmds_history.rs` (log/detail/cherry-pick/revert/reset),
  `cmds_branches.rs` (branch CRUD/checkout/merge/rebase/abort),
  `cmds_remotes.rs` (remotes/fetch/pull/push/tags push),
  `cmds_stash_tags.rs` (stash ops, tag CRUD),
  `helpers.rs` (diff→struct conversion, merge driver, conflict list),
  `state.rs` (current repo path, HTTPS credentials, persisted recents in
  `~/.config/git-crackit/state.json`).
- `src/` — React frontend. `api.ts` wraps `invoke`, `App.tsx` orchestrates,
  `components/` holds Toolbar, ChangesPanel, HistoryPanel (with lane-graph),
  CommitDetail, DiffView, Dialogs, ContextMenu. `styles.css` is the theme.
- GitKraken parity notes: covers clone/init/open, commit graph, stage/commit/
  amend, branches, merge, rebase (auto-aborts on conflict), cherry-pick, revert,
  reset, stash, tags, remotes, fetch/pull/push, blame, submodules (list),
  conflict resolution flow (resolve → stage → commit; Abort restores state).
- Transport: HTTPS via `git2` (built without ssh feature — no libssh2/cmake on
  the build host); SSH (`ssh://` and `git@host:` scp-style) remotes are routed
  to the system `git` CLI — see `is_ssh_url`/`git_net`/`git_clone` in
  `helpers.rs`. CLI ops use the user's ssh-agent/`~/.ssh/config` with
  `BatchMode=yes`, inject stored HTTPS creds via a `credential.helper`, and map
  stderr (`Permission denied` → SSH hint, `401/403`/auth → `AUTH:` dialog,
  `non-fast-forward` → pull-first hint). SSH clone parses `Receiving objects:`
  progress → `clone-progress` events.
- HTTPS auth = username + token via Credentials dialog; `Remember me` stores
  creds in the OS keychain via `keyring` (state.json plaintext fallback,
  auto-migrated); `AppState::default` reloads; `AUTH:`-prefixed backend errors
  trigger the dialog and auto-retry; `clear_credentials` forgets.
- GitHub integration lives in `gh.rs` (`ureq` + serde_json): `github_repo`
  (owner/repo parsed from remote URL), `github_user`, `github_prs`,
  `github_issues` (filters out PR entries), `github_check_runs` (check-runs
  first, legacy commit status fallback → success/failure/pending/none),
  OAuth device flow (`github_device_start`/`github_device_poll` — needs a
  registered OAuth App client id via `GIT_CRACKIT_GH_CLIENT_ID` env or
  `GITHUB_CLIENT_ID` at compile time; without it the dialog falls back to the
  token-paste page). `fetch_pr_branch` fetches `refs/pull/N/head` into local
  `pr-N` (SSH/HTTPS aware). Frontend: PullRequests/Issues dialogs (gear menu +
  palette), `Avatar` renders real GitHub avatars via `ID+login@users.noreply`
  email parsing or `avatar_url`; CSP `img-src` allows avatars.githubusercontent.com.
