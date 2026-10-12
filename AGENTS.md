# Git Crackit

A Git GUI client — GitKraken-style feature set with a unique violet/indigo
theme (light + dark). Built with Tauri 2 + Rust (git2/libgit2) +
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
  `cmds_stash_tags.rs` (stash ops + stash file list/diff, tag CRUD),
  `helpers.rs` (diff→struct conversion, merge driver, conflict list,
  SSH transport helpers, LFS helpers, git CLI runner),
  `state.rs` (current repo path, HTTPS credentials, persisted recents in
  `~/.config/git-crackit/state.json`).
- `src/` — React frontend. `api.ts` wraps `invoke`, `App.tsx` orchestrates,
  `components/` holds Toolbar, ChangesPanel, HistoryPanel (with lane-graph),
  CommitDetail, DiffView, Dialogs, ContextMenu. `styles.css` is the theme.
  Perf conventions: the four heavy panels (ChangesPanel, HistoryPanel,
  DiffView, CommitDetailView) are `React.memo` — every prop App passes them
  must be a stable `useCallback`/state ref (`refreshAll`/`run` read `query`/
  `selFile` via refs so they never churn). History + diff lists are
  windowed via `useVirtual`; rows use `contain`/`content-visibility`.
  Motion: `--fast`/`--med` + `--ease`/`--ease-spring` tokens; transform/
  opacity only for animated state changes; theme switch cross-fades via a
  scoped transition list (elements with their own `transition` are excluded —
  a second declaration would replace it).
- GitKraken parity notes: covers clone/init/open, commit graph, stage/commit/
  amend, branches, merge, rebase (auto-aborts on conflict), cherry-pick, revert,
  reset, stash, tags, remotes, fetch/pull/push, blame, submodules (list),
  conflict resolution flow (resolve → stage → commit; Abort restores state),
  undo/redo (Ctrl/⌘+Z, ⇧+Z), force-push with lease, hunk-level staging,
  file history (--follow-style rename tracking), branch compare (three-dot),
  commit signing (commit.gpgsign → gpg/ssh-keygen), file-list multi-select.
- Commit signing: `commit_maybe_signed` in helpers.rs is THE commit entry —
  honor `commit.gpgsign` by piping `commit_create_buffer` output through the
  configured signer (`gpg --detach-sign --armor` for openpgp,
  `ssh-keygen -Y sign -n git` for gpg.format=ssh) then `commit_signed` +
  manual ref update (unborn-aware: symbolic HEAD target → `repo.reference`).
  `create_commit`/`finish_in_progress_commit`/amend all route through it.
- File history: `file_history` diffs each walked commit vs its FIRST parent
  with the FULL diff (never pathspec — it strips the rename's old-side delta
  before find_similar pairs it); a Renamed delta landing on the tracked path
  re-points the walk at the old name. `compare_refs`/`compare_file_diff`
  use merge-base→compare (three-dot) semantics.
- File list multi-select is ChangesPanel-local state (Set<path> + anchor
  index): ctrl/⌘ toggles, shift ranges, bulk ops via the `.multi-bar`; a
  `useEffect` prunes selections whose paths left `changes`.
- Undo: every HEAD-moving command records an `UndoPoint` (refname + oid +
  soft flag) in `AppState.undo_stack` BEFORE mutating but only commits it
  AFTER success (never on "dirty"/"up_to_date" no-ops). `restore_point` moves
  HEAD back then `git reset --merge` (same CLI trick as abort_merge — handles
  conflicted indexes, refuses on intersecting dirty work); `soft` points
  (commit/amend) only move the ref so changes stay staged. Deleted branches
  are recreated on restore; stacks clear on open/close/init/clone;
  `undo_last`/`redo_last` in cmds_history.rs; frontend `keyOps` ref feeds the
  Ctrl+Z keybind (skipped inside inputs).
- Force-push: `push(set_upstream, force)` — HTTPS path checks `remote.ls()`
  tip == `refs/remotes/<r>/<b>` before pushing `+refspec` (manual lease);
  SSH path uses `--force-with-lease`. git2 + CLI non-ff errors normalize to
  "non-fast-forward" — App.tsx `pushOp` intercepts that phrase to open the
  force-push confirm.
- Hunk staging: `hunk_op` in cmds_files builds the file's patch via
  `Patch::to_buf()`, slices at `@@` boundaries for exactly one hunk, and
  pipes it to `git apply` — `--cached` (stage), `--cached --reverse`
  (unstage), `--reverse` (discard). FileDiff hunk index == patch hunk order.
  DiffView shows hover buttons when `onHunkAction` is passed (working diff
  only); discard confirms.
  Dirty-worktree ops: `checkout_branch`, `checkout_commit`, `create_branch`,
  `merge_branch`, `pull`, `rebase_branch` all take a `dirty` mode and return
  `CheckoutOutcome` — "ask" (default) returns `status:"dirty"` so the frontend
  opens `DirtyCheckoutDialog`; "carry" = stash → op → re-apply
  (reinstantiate_index; on apply conflict the op completes with conflicts and
  the stash is kept as backup); "stash" leaves the changes in stash@{0}.
  Checkout-style ops prompt on ANY dirty file (`dirty_checkout`); merge-style
  ops prompt only when the merge would touch a dirty file (`merge_dirty_op` →
  `merge_blocked_paths` = merge-base→theirs diff ∩ dirty/untracked paths);
  rebase prompts on any dirty file (git refuses on all). `create_branch`
  skips the prompt entirely when the target tip == HEAD (can't clobber).
  Frontend `dirtyRun` (App.tsx) is the single entry point — the dialog's
  `retry` closure re-invokes the op with the chosen mode.
  Branch/detached checkouts do `checkout_tree(target)` → `set_head` — NEVER
  set_head + checkout_head (the workdir then looks dirty vs the new HEAD and
  safe checkout silently skips). Same rule in `merge_annotated`: fast-forward/
  unborn paths do `checkout_tree(target, safe)` → move ref — NEVER
  move-ref-then-force, which clobbered uncommitted work. `abort_merge`
  similarly must not force-reset: rebase states → `git rebase --abort`;
  everything else → `git reset --merge` (git2 lacks ResetType::Merge) so
  unrelated dirty work survives.
- Transport: HTTPS via `git2` (built without ssh feature — no libssh2/cmake on
  the build host); SSH (`ssh://` and `git@host:` scp-style) remotes are routed
  to the system `git` CLI — see `is_ssh_url`/`git_net`/`git_clone` in
  `helpers.rs`. CLI ops use the user's ssh-agent/`~/.ssh/config` with
  `BatchMode=yes`, inject stored HTTPS creds via a `credential.helper`, and map
  stderr (`Permission denied` → `SSH:` dialog, `401/403`/auth → `AUTH:` dialog,
  `non-fast-forward` → pull-first hint). SSH clone parses `Receiving objects:`
  progress → `clone-progress` events. `SSH:` errors (and gear menu → SSH keys…)
  open `SshKeyDialog`, which runs `ssh-add` via an `SSH_ASKPASS` echo script
  (`load_ssh_key`) — passphrase-protected keys load into the agent in-app;
  `list_ssh_keys` scans ~/.ssh for private-key files. Agent itself must be
  running already.
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
- Stashes: `list_stashes` + `stash_files`/`stash_file_diff` (diffs base
  commit→stash commit, third parent = untracked files). Sidebar "Stashes"
  section in ChangesPanel expands each stash to its file list; clicking a file
  shows a read-only diff with a "stash@{n}" banner; context menu apply/pop/drop.
- AI commit messages (only use of AI — nothing else): `ai.rs` — sparkle
  button in the commit box sends the staged diff (12 KB cap). Providers
  (persisted `AiSettings` in state.json): `auto` (first saved key in
  claude→openai→groq order, else local Ollama), `anthropic` (Messages API,
  `x-api-key` + `anthropic-version` headers), `openai`, `groq`, `ollama`
  (localhost:11434, model from `/api/tags`), `custom` (base URL + key).
  Key providers always auto-pick the cheapest chat model via the provider's
  `/models` list (cheap_model tier keywords; embed/tts/image skipped) — users
  never choose a model (Settings hides the field for them).
  Per-provider keys live in the OS keychain as `git-crackit/ai-{provider}`
  (state.json `keys` map fallback when no keychain). Generated text splits
  on the first blank line → summary + description. Retired/unusable:
  GitHub Models (EOL 2026-07-30), OpenCode Zen free tier (client-locked),
  Pollinations anonymous (flaky/402s — removed). AboutDialog = mini README
  in-app (gear menu / palette).
- Git LFS: `RepoInfo.uses_lfs`/`lfs_installed` flag repos whose `.gitattributes`
  contains `filter=lfs` and whether `git-lfs` is on PATH. Clone/pull/checkout
  run `git lfs pull|checkout` hooks when available; a warning banner shows when
  the repo uses LFS but the binary is missing (defensive — host may lack it).
- Image diffs: `get_image_diff(path, staged)` returns base64 old/new blobs;
  `DiffView` renders image files (png/jpg/gif/webp/bmp/ico/svg/avif)
  side-by-side on a checkerboard, "(absent)" for added/deleted sides.
- Drag-drop history ops: commit rows are draggable (`application/x-gc-commit`);
  dropping on a commit opens a menu — cherry-pick, move-after, squash-into.
  `rebase_commit_action` scripts a `git rebase -i --rebase-merges`
  (GIT_SEQUENCE_EDITOR awk script rewrites the todo in place — merge commits
  preserved; moving/squashing a merge commit is rejected). Move/squash confirm
  first since they rewrite history.
- Auto-updates: `tauri-plugin-updater` + `tauri-plugin-process` (relaunch).
  Endpoint = `releases/latest/download/latest.json` on the project GitHub
  repo. Signing: minisign keypair generated via `npx tauri signer generate`
  (`~/.tauri/git-crackit.key` — private, never commit); pubkey lives in
  `tauri.conf.json` plugins.updater. Releases: `.github/workflows/release.yml`
  builds all 3 OSes on `v*` tags via tauri-action (needs repo secrets
  `TAURI_SIGNING_PRIVATE_KEY[_PASSWORD]`). The workflow also forwards the
  standard signing env vars (`APPLE_CERTIFICATE[_PASSWORD]`,
  `APPLE_SIGNING_IDENTITY`, `APPLE_ID/PASSWORD/TEAM_ID` for notarization,
  `WINDOWS_CERTIFICATE[_PASSWORD]` for a .pfx) — installers become signed
  automatically once those secrets exist; until then they're unsigned.
- Accessibility: `role="menu/menuitem/separator"` + arrow-key nav in context
  menus, `aria-modal`+`aria-labelledby` dialogs, `role="tab"` seg control,
  `aria-live` toast stack, `role="status/alert"` banners, image `alt` text.
