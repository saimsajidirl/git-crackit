# Git Crackit

A fast, lightweight Git GUI — GitKraken-style feature set with a clean
light/dark interface. Built with **Tauri 2 + Rust (git2/libgit2) + React 18 +
TypeScript + Vite**, so it ships as a tiny native binary instead of an Electron
bundle.

## Features

**Repository lifecycle**
- Open, init, and clone repos (HTTPS + SSH), recent-repos list, live fs-watcher refresh

**Working tree**
- Stage/unstage/discard per file or in bulk, conflict resolution flow, amend,
  .gitignore editing, per-file blame view
- **Image diffs** — side-by-side before/after previews for png/jpg/gif/webp/bmp/ico/svg/avif
- **Stashes** — a persistent sidebar section: expand a stash to browse its files
  (including untracked), view read-only diffs, apply / pop / drop

**History**
- Real lane-graph commit visualization with virtualized scrolling, search,
  pagination, ref labels (branches/tags/HEAD)
- Commit detail + file diffs; **drag a commit onto another** to cherry-pick,
  reorder (move-after), or squash — powered by scripted `git rebase -i`
- Cherry-pick, revert, checkout-detached, reset soft/mixed/hard

**Branches & remotes**
- Branch create/checkout/rename/delete, merge with conflict handling, rebase,
  tags (annotated + lightweight, push)
- Fetch / pull / push over **HTTPS and SSH** — SSH remotes route through the
  system `git` CLI so ssh-agent, `~/.ssh/config`, ProxyJump, and custom ports
  all work
- Submodule listing and update

**GitHub integration**
- Sign in via OAuth device flow (or paste a token); credentials persist in the
  **OS keychain** (Secret Service / Keychain / Credential Manager)
- Pull requests list with CI status + author avatars, one-click PR checkout
- Issues list with labels; real GitHub avatars in the commit graph

**Polish**
- Git LFS detection + hooks with a warning when `git-lfs` isn't installed
- Auto-updates via `tauri-plugin-updater` (signed minisign artifacts)
- Command palette (Ctrl/⌘+K), keyboard shortcuts, light & dark themes,
  resizable sidebar, accessible menus/dialogs (ARIA roles, keyboard nav)

## Getting started

```bash
npm install          # frontend deps
npm run tauri dev    # run the app in dev mode (hot reload)
npm run tauri build  # release build + installers
cd src-tauri && cargo test   # backend tests
```

Requirements: Rust toolchain, Node 20+, and system `git` (used for SSH remotes
and drag-drop history rewrites). `git-lfs` is optional — the app warns when a
repo needs it.

## Test fixture

`test_git_Crack/` contains a generator that builds a fully-loaded offline
fixture repo (branches, merges, stashes, submodule, local remote, images, LFS
markers, dirty tree) plus a feature-by-feature checklist:

```bash
./test_git_Crack/setup.sh   # then open test_git_Crack/test_repo in the app
```

## Releasing

Tag `v*` → `.github/workflows/release.yml` builds all three OSes and publishes a
draft GitHub release with signed updater artifacts (`latest.json` + `.sig`),
which the in-app updater consumes.

Required repo secrets: `TAURI_SIGNING_PRIVATE_KEY`,
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (generate locally with
`npx tauri signer generate`).

Optional: `GIT_CRACKIT_GH_CLIENT_ID` at build time enables the GitHub OAuth
device flow; without it the sign-in dialog falls back to token entry.

## Known limitations

- Installers are **unsigned** (no Apple Developer / EV certs yet) — expect
  Gatekeeper/SmartScreen warnings.
- Passphrase-protected SSH keys must be loaded into `ssh-agent` (`ssh-add`) —
  BatchMode never prompts interactively.
- Drag-drop history rewrites run through `git rebase -i` — merge commits inside
  the rewritten range are not preserved.
