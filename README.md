<div align="center">
  <img src="docs/logo.png" alt="Git Crackit logo" width="128" />
  <h1>Git Crackit</h1>
  <p>
    <b>A fast, native Git desktop client — GitKraken-style power,<br/>
    GitHub Desktop-style approachability, none of the Electron weight.</b>
  </p>
  <p>
    Tauri 2 &middot; Rust (git2/libgit2) &middot; React 18 &middot; TypeScript &middot; Vite
  </p>
</div>

---

## Download

Installers for all three platforms are built by GitHub Actions and attached to
each [GitHub Release](https://github.com/saimsajidirl/git-crackit/releases):

| Platform | Files | Notes |
|---|---|---|
| **Windows** | `.msi` / `.exe` (NSIS) | SmartScreen may warn — installers aren't EV-signed yet |
| **macOS** | `.dmg` + `.app.tar.gz` | Gatekeeper may warn — right-click → Open, or `xattr -d com.apple.quarantine` |
| **Linux** | `.AppImage`, `.deb`, `.rpm` | AppImage is portable; `.deb`/`.rpm` integrate with the desktop |

> **Unsigned builds**: until Apple/Windows code-signing secrets are configured
> (see [Code signing](#code-signing)), your OS will warn before installing.
> That's normal for unsigned OSS — verify you downloaded from this repo.

## Features

### Repository lifecycle
- **Open / init / clone** local, HTTPS, and SSH remotes; recent-repos list
- Live filesystem watcher — external changes (CLI checkouts, file edits)
  refresh the UI automatically

### Working tree
- Stage / unstage / discard per file, per hunk view, or in bulk
- Merge-conflict resolution flow: pick sides, stage, commit — or Abort to
  restore the pre-merge state
- Amend last commit, `.gitignore` editing, per-file **blame** view
- **AI commit messages** — click ✦ and the staged diff becomes a conventional
  commit message (see [AI commit generation](#ai-commit-generation))
- **Image diffs** — side-by-side before/after previews
  (png, jpg, gif, webp, bmp, ico, svg, avif) on a checkerboard backdrop
- **Stash browser** — sidebar section: expand a stash to see its file list
  (including untracked files), read-only diffs, apply / pop / drop

### History
- Real lane-graph visualization with virtualized scrolling, search,
  pagination, and ref labels (branches, tags, HEAD)
- Commit detail with full file diffs
- **Drag a commit onto another** → cherry-pick, move-after (reorder), or
  squash — powered by `git rebase -i --rebase-merges`, so merge topology in
  the rewritten range is preserved
- Cherry-pick, revert, checkout-detached, reset (soft / mixed / hard)

### Branches & remotes
- Branch create / checkout / rename / delete, merge, rebase
- Tags — annotated and lightweight, push support
- Fetch / pull / push over **HTTPS and SSH** — SSH ops route through the
  system `git` CLI, so `ssh-agent`, `~/.ssh/config`, `ProxyJump`, custom
  ports, and passphrase keys all behave exactly like your terminal
- **In-app SSH key loading** — gear menu → "SSH keys…" loads a
  passphrase-protected key into `ssh-agent` without a terminal; SSH auth
  failures auto-open it and retry the failed operation
- Submodule listing and update

### GitHub integration
- Sign in via **OAuth device flow** or paste a PAT; the token lives in your
  **OS keychain** (Secret Service / Keychain / Credential Manager)
- Pull-request list with CI/check status and author avatars;
  one-click PR checkout (`refs/pull/N/head` → local `pr-N` branch)
- Issues list with labels; real GitHub avatars inline in the commit graph

### Git LFS
- Detects `filter=lfs` in `.gitattributes`, runs `git lfs pull`/`checkout`
  hooks after clone/pull/checkout, and warns when `git-lfs` isn't installed

### AI commit generation
- Providers: **Claude** (Anthropic), **OpenAI**, **Groq**, **Ollama**
  (fully local — no key, diffs never leave your machine), or any
  **custom OpenAI-compatible** endpoint
- **Always the cheapest model** — the app lists the provider's live model
  catalog and auto-picks the least expensive chat model each time.
  There's no model selector for hosted providers; nothing to configure
  beyond the API key
- **Privacy**: only the staged diff is sent, only when you click ✦. API keys
  are stored in the OS keychain — never in the cloud, never in the repo
- Large diffs are truncated per-line and per-total; if a provider rejects an
  oversized request, the app automatically retries with a smaller payload
- Configure via gear menu → **AI keys…** (or Settings → Git settings)

### Polish
- Light & dark themes with a signature blue→sky gradient identity
- Command palette (Ctrl/⌘+K) covering every major action
- **UI zoom** — Ctrl/⌘+`=` / `-` / `0` scales the whole app (60–200%,
  persisted)
- Auto-updates via `tauri-plugin-updater` (minisign-signed artifacts)
- Resizable sidebar, accessible menus/dialogs (ARIA roles, keyboard nav,
  live-region toasts)

## Keyboard shortcuts

| Shortcut | Action |
|---|---|
| Ctrl/⌘+K | Command palette |
| Ctrl/⌘+Enter | Commit (from summary field: Enter also commits) |
| Ctrl/⌘+= / − / 0 | Zoom in / out / reset |
| Esc | Close dialog / menu |
| ↑ ↓ Enter | Navigate menus and the palette |

## Build from source

**Prerequisites:** Rust toolchain, Node 20+, system `git`, and the
[Tauri system deps](https://v2.tauri.app/start/prerequisites/)
(Linux: `libwebkit2gtk-4.1-dev librsvg2-dev libssl-dev patchelf`).

```bash
npm install            # frontend deps
npm run tauri dev      # dev mode with hot reload
npm run tauri build    # release build → src-tauri/target/release/bundle/
cd src-tauri && cargo test   # backend e2e tests (self-contained fixtures)
```

Outputs land in `src-tauri/target/release/bundle/` — `appimage/`, `deb/`,
`rpm/` on Linux; `dmg/` on macOS; `msi/` + `nsis/` on Windows.

## Releasing (downloadable installers via GitHub Actions)

`.github/workflows/release.yml` builds all three OSes and attaches every
installer + updater artifact to a GitHub Release. Two ways to trigger:

```bash
git tag v0.1.0 && git push origin v0.1.0   # option 1: push a tag
```

…or **Actions → Release → Run workflow** with a tag name (creates the tag at
the dispatched commit).

Required repo secrets for updater artifacts:
- `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
  (generate once with `npx tauri signer generate` — the pubkey is already
  baked into `tauri.conf.json`)

### Code signing

The workflow already forwards the standard cert env vars — signing activates
automatically once the secrets exist:

- **macOS**: `APPLE_CERTIFICATE` (base64 `.p12` of a *Developer ID
  Application* cert — `base64 -i cert.p12 | pbcopy`),
  `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, plus notarization
  creds `APPLE_ID` / `APPLE_PASSWORD` (app-specific password) /
  `APPLE_TEAM_ID`. Requires a paid Apple Developer account ($99/yr).
- **Windows**: `WINDOWS_CERTIFICATE` (base64 `.pfx`) +
  `WINDOWS_CERTIFICATE_PASSWORD`. OV/EV removes SmartScreen; Azure Trusted
  Signing also works (`AZURE_TENANT_ID`/`AZURE_CLIENT_ID`/
  `AZURE_CLIENT_SECRET` secrets + a `signCommand` under `bundle.windows`).
- **Linux**: no code signing exists — nothing to configure.

Optional: `GIT_CRACKIT_GH_CLIENT_ID` at build time enables the GitHub OAuth
device flow; without it the sign-in dialog falls back to token paste.

## Where things live

| What | Where |
|---|---|
| App state (recents, prefs) | `~/.config/git-crackit/state.json` |
| Credentials & AI keys | OS keychain (`git-crackit` service) |
| Updater signing key | `~/.tauri/git-crackit.key` (never committed) |

## Test fixture

`test_git_Crack/` builds a fully-loaded offline repo — merge lanes, tags,
stashes, submodule, a local bare remote with a waiting upstream commit,
image diffs, LFS markers, and a dirty working tree:

```bash
./test_git_Crack/setup.sh        # then open test_git_Crack/test_repo
./test_git_Crack/refresh_dirty.sh  # re-dirty after committing
./test_git_Crack/make_conflict.sh  # drop into a real merge conflict
```

## Known limitations

- Passphrase-protected SSH keys need a running `ssh-agent` — the app loads
  keys into it (`ssh-add`), but can't start the agent itself
- Merge commits can't be moved or squashed in drag-drop rewrites (Git can't
  reorder merge lines) — merges *inside* a rewritten range are preserved
- Installers are unsigned until code-signing secrets are configured

## Contributing

Issues and PRs welcome at
[github.com/saimsajidirl/git-crackit](https://github.com/saimsajidirl/git-crackit).
See `AGENTS.md` for architecture notes and the `test_git_Crack/` fixture for
repro setups.
