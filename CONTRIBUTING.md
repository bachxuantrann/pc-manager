# Contributing to PC Manager

Thanks for your interest in improving PC Manager. This guide covers the development setup,
checks, project layout and the release process.

## Prerequisites

- Windows 10 or 11 (x64)
- [Node.js](https://nodejs.org) 22+
- [Rust](https://rustup.rs) stable, toolchain `x86_64-pc-windows-msvc`
- Visual Studio 2022 Build Tools with the *Desktop development with C++* workload and a Windows SDK
- Microsoft Edge WebView2 Runtime

## Getting started

```powershell
git clone https://github.com/bachxuantrann/pc-manager.git
cd pc-manager
npm install
npm run tauri dev
```

The app runs as a standard user. To try features that need administrator rights, click
**Run as administrator** in the app, or start an elevated dev session with
`.\scripts\admin-test.ps1 -Mode dev` (UAC prompt).

## Project layout

```
pc-manager/
├── src/                   React + TypeScript UI (Vite, Tailwind)
│   ├── pages/             one file per page
│   ├── components/  hooks/
│   ├── lib/api.ts         every backend call, typed
│   └── dev/demo.ts        demo data for screenshots (dev builds only)
├── src-tauri/
│   └── src/
│       ├── commands/      thin Tauri commands: validate input → call module → map errors
│       ├── sys/           all unsafe / Win32 code, wrapped in safe functions
│       ├── memory/  cleaner/  process/  startup/  usage/  disk/
│       ├── settings.rs    protected folders, scheduled cleaning, start with Windows
│       └── store/         SQLite (settings, activity log, usage samples)
├── rules/cleaner.toml     cleaning rules, compiled into the app
├── docs/                  safety documentation, screenshots
└── scripts/               admin tests, signing, screenshots
```

## Code guidelines

- `unsafe` is allowed only in `src-tauri/src/sys/`; every `unsafe` block has a `// SAFETY:` comment.
  The crate enforces this with `#![deny(unsafe_code)]` and Clippy.
- Every Win32 handle is owned by a type that closes it on drop.
- No `unwrap()` / `expect()` outside tests. Errors reach the UI as `{ code, message }`.
- Long-running commands are `async`, run on a blocking thread, report progress through a channel,
  and can be cancelled. Disk-scanning threads run in background priority mode.
- Do not relax the rules in [docs/SAFETY.md](docs/SAFETY.md). Anything that deletes data must go
  through a fixed rule or the Recycle Bin, with a preview and confirmation.
- Pure logic (rule parsing, glob matching, file age, ROT13 decoding…) gets unit tests.

## Checks

All three must pass before a change is merged (CI runs them on every push and pull request):

```powershell
cd src-tauri
cargo clippy --all-targets -- -D warnings
cargo test
cd ..
npm run build
```

### Tests that need administrator rights

These are marked `#[ignore = "…Administrator…"]` and run through a script that relaunches itself
elevated (Windows asks for UAC confirmation every time; nothing is stored):

```powershell
.\scripts\admin-test.ps1                 # all tests, including ignored admin tests
.\scripts\admin-test.ps1 -Filter memory  # only tests whose name contains "memory"
.\scripts\admin-test.ps1 -Mode ram       # measure memory use of the release build
```

The log is written to `src-tauri/target/admin-test.log`.

## Building

```powershell
npm run tauri build    # unsigned installer: src-tauri/target/release/bundle/nsis/
npm run build:signed   # same, signed with your local self-signed certificate
```

To sign locally, create a code-signing certificate once with `.\scripts\new-signing-cert.ps1`.
It is stored in your user certificate store with a non-exportable private key and is not added
to Trusted Root.

The app icon is generated from `app-icon.svg`:

```powershell
npx tauri icon app-icon.svg -o src-tauri/icons   # then delete the generated android/ and ios/ folders
```

## Screenshots

Run `npm run dev`, then `.\scripts\screenshots.ps1`. Each page opens in demo mode
(`?demo&page=…`, fake data from `src/dev/demo.ts`, excluded from release builds) and is captured
with headless Edge into `docs/screenshots/`.

## Continuous integration and releases

- **[CI](.github/workflows/ci.yml)** — on every push to `main` and every pull request: UI build,
  Clippy, tests and an unsigned installer, uploaded as a workflow artifact for 14 days.
- **[Release](.github/workflows/release.yml)** — on every `v*` tag: the same checks, then a GitHub
  Release with the installer and its SHA-256 checksum.

To publish a release:

1. Update `version` in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
2. Commit, then tag and push:

   ```powershell
   git tag -a v1.0.1 -m "PC Manager v1.0.1"
   git push origin main v1.0.1
   ```

The workflow fails if the tag does not match the version in `package.json`.

**Signing in CI (optional):** run `.\scripts\new-ci-signing-cert.ps1` once (requires
`gh auth login`). It creates a separate self-signed certificate, stores it in the repository
secrets `SIGNING_PFX_BASE64` and `SIGNING_PFX_PASSWORD`, then deletes the local copy. Without
these secrets, releases are published unsigned.

## Pull requests

- Keep each pull request focused on one change and describe what it changes and why.
- Include tests for new logic, and screenshots for UI changes.
- Make sure CI passes.

By contributing, you agree that your contributions are licensed under the same terms as the
project: MIT OR Apache-2.0.
