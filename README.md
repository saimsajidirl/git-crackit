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

### Code signing (optional — activates automatically once secrets exist)

The workflow already passes the standard Tauri signing env vars; add the
secrets and installers get signed on the next tag:

- **macOS**: `APPLE_CERTIFICATE` (base64 `.p12` of a *Developer ID Application*
  cert — `base64 -i cert.p12 | pbcopy`), `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_SIGNING_IDENTITY`, plus notarization creds `APPLE_ID` /
  `APPLE_PASSWORD` (app-specific password) / `APPLE_TEAM_ID`.
  Requires a paid Apple Developer account ($99/yr).
- **Windows**: `WINDOWS_CERTIFICATE` (base64 `.pfx`) +
  `WINDOWS_CERTIFICATE_PASSWORD`. An OV/EV cert removes SmartScreen;
  a self-signed cert signs but still warns. Azure Trusted Signing also works —
  add `AZURE_TENANT_ID`/`AZURE_CLIENT_ID`/`AZURE_CLIENT_SECRET` secrets and a
  `signCommand` under `bundle.windows` in `tauri.conf.json`.
- **Linux**: no code signing exists; nothing to configure.

Until certs are added, installers are unsigned — expect Gatekeeper/SmartScreen
warnings (normal for unsigned OSS releases).

Optional: `GIT_CRACKIT_GH_CLIENT_ID` at build time enables the GitHub OAuth
device flow; without it the sign-in dialog falls back to token entry.

## Known limitations

- Passphrase-protected SSH keys must be loaded into a running `ssh-agent` —
  the "Load SSH key into agent" dialog (gear menu → SSH keys…, auto-opens on
  SSH auth failures) does this in-app via `ssh-add`. If no agent is running,
  start one first (`eval $(ssh-agent)` / OpenSSH Authentication Agent service).
- Drag-drop history rewrites use `git rebase -i --rebase-merges` — merge
  commits in the range are preserved, but you can't move or squash a merge
  commit itself (git can't reorder merge lines).
