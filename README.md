<div align="center">

<img src="app-icon.svg" width="96" alt="PC Manager logo">

# PC Manager

**A lightweight, safety-first maintenance tool for Windows 10 and 11.**

Monitor memory, clean junk files, manage processes and startup apps, find unused apps,<br>
large files and duplicates — in a 2.5 MB installer that idles at a few megabytes of RAM.

[![Release](https://img.shields.io/github/v/release/bachxuantrann/pc-manager?label=release)](https://github.com/bachxuantrann/pc-manager/releases/latest)
[![CI](https://github.com/bachxuantrann/pc-manager/actions/workflows/ci.yml/badge.svg)](https://github.com/bachxuantrann/pc-manager/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
![Platform: Windows 10/11 x64](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011%20x64-0078D4)

**English** · [Tiếng Việt](README.vi.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

</div>

![PC Manager dashboard](docs/screenshots/dashboard.png)

> The app's interface is currently in Vietnamese. This documentation is available in several languages.

## Contents

- [Highlights](#highlights)
- [Features](#features)
- [Installation](#installation)
- [Administrator rights](#administrator-rights)
- [Safety](#safety)
- [How memory Boost works](#how-memory-boost-works)
- [FAQ](#faq)
- [Building from source](#building-from-source)
- [License](#license)

## Highlights

- **Lightweight.** ~2.5 MB installer. In the system tray the app uses roughly 2–15 MB of RAM;
  closing the window releases the UI engine entirely.
- **Safe by design.** Deletes only what fixed rules allow, always shows a preview first,
  sends anything you pick by hand to the Recycle Bin, and logs every change.
- **No admin prompt on launch.** Runs as a standard user and asks for elevation only when a
  feature actually needs it.
- **Honest.** Reports the real gain of every action instead of inflated "freed" numbers.

## Features

| Area | What it does |
|---|---|
| **Dashboard and memory** | Live RAM usage, available and standby memory, 60-second history; Boost, Smart Boost and Deep Boost |
| **Junk cleaner** | Temp files, browser caches, crash dumps, logs, Windows Update cache, Recycle Bin. Preview before cleaning, dry-run mode. Deep clean of WinSxS (via DISM) and `Windows.old` (via Disk Cleanup) |
| **Processes** | CPU and memory per process, publisher (digital signature), end, suspend and resume, priority, trim working set, open file location |
| **Startup apps** | Registry, Startup folders, scheduled tasks and services; enable or disable the same way Task Manager does |
| **Installed apps** | Size, install date and last-used date; filter apps you have not used for months; uninstall, then clean up leftovers |
| **Disk** | Drive usage, largest files, content-identical duplicates (BLAKE3); remove to the Recycle Bin |
| **Settings** | Smart Boost threshold, scheduled cleaning, start with Windows, protected folders, activity log |

<table>
  <tr>
    <td><img src="docs/screenshots/cleaner.png" alt="Junk cleaner with preview"></td>
    <td><img src="docs/screenshots/disk.png" alt="Duplicate finder"></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/apps.png" alt="Installed apps with last-used date"></td>
    <td><img src="docs/screenshots/settings.png" alt="Settings and activity log"></td>
  </tr>
</table>

<sub>Screenshots use demo data, not data from a real machine.</sub>

## Installation

1. Download `PC-Manager_<version>_x64-setup.exe` from the
   [latest release](https://github.com/bachxuantrann/pc-manager/releases/latest).
2. Run the installer. It installs to `Program Files` and asks for administrator approval once.
3. The installer is **self-signed**, not signed by a certificate authority that Windows trusts,
   so Microsoft Defender SmartScreen may show *"Windows protected your PC"*.
   Choose **More info → Run anyway**. Each release lists the installer's SHA-256 checksum so you
   can verify the download, or you can [build it yourself](#building-from-source).

**Requirements:** Windows 10 or 11 (x64) and the Microsoft Edge WebView2 Runtime, which ships
with Windows 11 and is downloaded automatically by the installer if missing.

**Updating:** run the new installer over the existing one; settings and the activity log are kept.
**Uninstalling:** use *Settings → Apps* like any other app. The start-with-Windows task, if
enabled, is removed as well.

## Administrator rights

| How it runs | Rights |
|---|---|
| Normal launch | Standard user, no UAC prompt |
| **Run as administrator** button (sidebar, or next to a feature that needs it) | Administrator, after a UAC prompt; reopens on the same page |
| Start with Windows (optional) | Administrator without prompting — can only be enabled when the app is installed in `Program Files` |

Features that need administrator rights: memory Boost, cleaning system locations
(`Windows\Temp`, system crash dumps, Windows Update cache), reading Prefetch, changing
machine-wide startup entries, acting on system processes, uninstalling machine-wide apps,
and WinSxS cleanup. Everything else works without elevation.

## Safety

PC Manager touches files, processes, the registry and services, so every destructive path is
guarded in code. Summary below; full details in **[docs/SAFETY.md](docs/SAFETY.md)**.

- **Rule-based cleaning only.** The cleaner deletes files matching fixed rules in
  [`rules/cleaner.toml`](rules/cleaner.toml), compiled into the app. No command accepts an
  arbitrary path from the UI for deletion.
- **Scan → preview → confirm → delete**, with a dry-run mode.
- **Never follows** symlinks, junctions or reparse points; skips locked files, files newer than
  the rule's age threshold, and files changed after the scan.
- **Hard block list:** `System32`, `SysWOW64`, `WinSxS` (cleaned only through DISM), `Prefetch`,
  the user-profile root, personal folders (Documents, Desktop, Pictures…), `Program Files`,
  and every drive root. You can add your own **protected folders**.
- **Recycle Bin, not permanent deletion,** for large and duplicate files and for leftovers
  after uninstalling an app. Registry keys are exported to a `.reg` backup before removal.
- **Critical processes are protected** (`csrss`, `wininit`, `winlogon`, `lsass`, `smss`,
  `services`, System). Disabling a startup entry never deletes the original.
- **Every change is logged** locally and visible under *Settings → Activity log*.
- **Tested.** 140+ automated tests, including sandboxed tests of the file-selection logic
  (junctions, locked files, young files, blocked folders, files modified after a scan),
  run on every push.

To report a security issue, see [SECURITY.md](SECURITY.md).

## How memory Boost works

Windows already manages memory well, so PC Manager does not promise miracles:

- **Boost (recommended)** writes pending modified pages to disk and drops *low-priority*
  standby cache. Cache of apps you use often is kept.
- **Deep Boost** drops the *entire* standby cache. "Free" memory jumps, but **available memory
  barely changes** (standby cache already counts as available) and apps load more slowly the
  next time. Use it only when you have a specific reason.
- **Smart Boost** runs only when available memory falls below a threshold (15% by default).
- Every result separates **"available memory gained"** (the real benefit) from
  **"cache cleared"**.
- **Trim working set** applies only to processes you select, never in bulk.

## FAQ

<details>
<summary><b>Why doesn't PC Manager appear under Task Manager → Startup apps?</b></summary>

Start with Windows uses a Task Scheduler task with highest privileges, because a regular
`Run` registry entry cannot start an app with administrator rights without a UAC prompt.
Task Manager does not list scheduled tasks. You will find it as **PC Manager Autostart** in
Task Scheduler, and on PC Manager's own *Startup* page.
</details>

<details>
<summary><b>Why does SmartScreen or my antivirus warn about the installer?</b></summary>

The installer is self-signed. Removing that warning requires a commercial code-signing
certificate. Compare the SHA-256 checksum on the release page, or build from source.
</details>

<details>
<summary><b>How much memory does PC Manager itself use?</b></summary>

About 2–15 MB while running in the tray. With the window open, about 75–80 MB, most of it the
WebView2 engine that renders the UI; it releases memory when the window loses focus and is
shut down completely when you close the window.
</details>

<details>
<summary><b>Does it work on macOS or Linux?</b></summary>

Not currently. The system features rely on Windows APIs.
</details>

## Building from source

Requires Node.js 22+, Rust (stable, `x86_64-pc-windows-msvc`), Visual Studio 2022 Build Tools
(C++ workload and Windows SDK) and WebView2.

```powershell
git clone https://github.com/bachxuantrann/pc-manager.git
cd pc-manager
npm install
npm run tauri build   # installer: src-tauri/target/release/bundle/nsis/
```

See **[CONTRIBUTING.md](CONTRIBUTING.md)** for development, tests, CI and releases.

Built with [Tauri 2](https://tauri.app), [React](https://react.dev), [Rust](https://www.rust-lang.org)
and [windows-rs](https://github.com/microsoft/windows-rs).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
