# Safety and administrator rights

[English](SAFETY.md) · [Tiếng Việt](AN-TOAN.md)

PC Manager works with files, processes, the registry and Windows services. This document describes
**what the app may and may not do**, and the safeguards implemented in code.

## 1. Permission model

| How it runs | Rights | UAC prompt |
|---|---|---|
| Normal launch | **Standard user** (manifest `asInvoker`) | No |
| **Run as administrator** button | Administrator — the app relaunches itself with `ShellExecuteExW("runas")` and the old instance exits | Yes, every time |
| Start with Windows (if enabled) | Administrator — Task Scheduler task with "run with highest privileges" at sign-in | Once, when enabling |

- Without the required rights, a feature reports "administrator rights required" instead of failing.
- **Start with Windows can only be enabled when the app is installed under `Program Files`.**
  The task runs the executable as administrator without asking again; if it lived somewhere a
  standard user can write to (Desktop, Downloads…), malware could replace it to gain administrator
  rights. Symlinks and junctions are resolved before the path is checked.
- Turning this option on or off requires administrator rights. Uninstalling with the installer
  removes the task.
- The app does **not** install a Windows service and never runs anything as SYSTEM.

## 2. Junk cleaner

The cleaner **deletes only files matching fixed rules** in [`rules/cleaner.toml`](../rules/cleaner.toml),
compiled into the app at build time. No command accepts an arbitrary path from the UI for deletion.

| Rule | Location | Risk | Only files older than |
|---|---|---|---|
| Your temporary files | `%LOCALAPPDATA%\Temp` | low | 24 hours |
| Windows temporary files | `%SystemRoot%\Temp` | low | 24 hours |
| Thumbnail cache | `%LOCALAPPDATA%\Microsoft\Windows\Explorer` (`thumbcache_*`) | low | — |
| DirectX shader cache | `%LOCALAPPDATA%\D3DSCache` | low | 24 hours |
| App crash dumps | `%LOCALAPPDATA%\CrashDumps` | low | 24 hours |
| Blue-screen minidumps | `%SystemRoot%\Minidump` | medium | 7 days |
| `MEMORY.DMP` | `%SystemRoot%` (that file only) | medium | 7 days |
| Windows Error Reporting | `WER` (user and system) | low | 24 hours |
| Old Windows logs | `%SystemRoot%\Logs` | medium | 7 days |
| Chrome / Edge / Firefox cache | browser cache folders | low | — (skipped while the browser is running) |
| Windows Update cache | `SoftwareDistribution\Download` | medium | stops `wuauserv` first, restarts it after |
| Delivery Optimization | `DoSvc` cache | medium | stops and restarts the service |
| Recycle Bin | `SHEmptyRecycleBin` | high | **not** selected by default |

**Checks applied to every file, both while scanning and right before deletion:**

1. **Scan → preview → you choose → confirm → delete.** A **dry-run** mode reports what would be
   deleted without deleting anything.
2. **Never follows symlinks, junctions or reparse points**, and never touches OneDrive
   online-only files.
3. **Skips locked files** and **files newer than the rule's age threshold** (age is measured from
   the newer of the creation and modification times).
4. **Skips files changed after the scan** (different size or timestamp).
5. **Hard block list**, never deleted whatever a rule says: `System32`, `SysWOW64`, `WinSxS`,
   `Prefetch`, `Boot`, the user-profile root, Documents / Desktop / Pictures / Videos / Music /
   Downloads (including when redirected to another drive), `Program Files`, every drive root
   `X:\`, network paths, and paths containing `..`.
6. **Protected folders** you add in Settings: nothing inside them is ever deleted (applies to the
   cleaner, the disk tools and leftover cleanup).
7. Scan results stay in the Rust backend and expire after 30 minutes; the UI only sends **rule IDs**.

**Scheduled cleaning** (off by default) may only use rules that are **low risk, do not stop a
service, and do not empty the Recycle Bin**. Enabling it and choosing rules in Settings is your
confirmation; every check above still applies.

**Deep system cleanup** uses only official Windows tools:

- WinSxS: `DISM /Online /Cleanup-Image /StartComponentCleanup`. **No** `/ResetBase`, so installed
  Windows updates can still be uninstalled.
- `Windows.old`: Disk Cleanup (`cleanmgr /sagerun`) with only "Previous Installations" enabled.
  The app never deletes files in `Windows.old` itself.

## 3. Disk — large and duplicate files

- You pick files from the scan results; they are **only moved to the Recycle Bin**
  (`SHFileOperationW` with `FOF_ALLOWUNDO`) and can be restored. If a file is too large for the
  Recycle Bin, Windows asks before deleting it permanently.
- **Blocked:** Windows, `Program Files`, `ProgramData`, files at a drive root (`pagefile.sys`,
  `hiberfil.sys`…), user-profile files (`ntuser.dat`), `$Recycle.Bin`, `System Volume Information`.
- **Warned:** files under AppData and virtual disks (`.vhdx`, `.vmdk`… used by WSL, Docker or
  virtual machines). The "Hide risky files" filter is on by default.
- Duplicates: at least **one copy per group is always kept**; hard links (one file, several paths)
  are not treated as duplicates; size and modification time are re-checked before moving.

## 4. Apps — uninstall and leftover cleanup

- Uninstall runs **the app's own uninstaller** (`UninstallString`, re-read from the registry, absolute
  paths only). When it finishes, PC Manager looks for leftovers.
- Apps installed for the current user only (`HKCU`) are always uninstalled **as the signed-in user
  without administrator rights**, even when PC Manager runs as administrator — any standard program can
  change their uninstall command, so running it elevated would be a privilege-escalation path. If such an
  uninstaller cannot start that way, PC Manager reports an error instead of falling back to admin rights.
- Microsoft Store apps are listed read-only; they are uninstalled from Windows Settings.
- **Leftover cleanup:** the backend finds the install folder, AppData/ProgramData folders matching the
  app name, Start-menu shortcuts pointing into the install folder, and the `Uninstall` registry key.
  Partial name matches ("possible") are not selected by default. It never selects:
  - shared folders (Microsoft, Common Files, Packages, `.ssh`…);
  - folders that contain, or sit inside, **another app's** install folder;
  - folders with a running program.
- If the app still has a **service, driver or scheduled task** pointing into a folder, PC Manager
  warns and does not pre-select it.
- Files go to the **Recycle Bin**; registry keys are **exported to a `.reg` backup** before removal
  (`%APPDATA%\com.pcmanager.app\backups`).

## 5. Processes and startup

- **Cannot end** critical system processes (`csrss`, `wininit`, `winlogon`, `lsass`, `smss`,
  `services`, System, PID 0/4), processes marked critical, or PC Manager itself. Every action
  re-verifies PID and creation time so a new process that reuses a PID is never affected.
- Startup: an entry is disabled through `StartupApproved` (the same mechanism Task Manager uses);
  the original `Run` entry is **never deleted**. Scheduled tasks and services are shown read-only.

## 6. Activity log

Every system change (deletion, move to Recycle Bin, ending a process, enabling or disabling startup,
uninstalling, changing settings, elevating…) is recorded in a local SQLite database: time, action,
target, result. See **Settings → Activity log**.

## 7. What memory Boost actually does

- **Boost (light):** writes the modified page list to disk and drops **low-priority** standby pages.
  Cache of frequently used apps is kept.
- **Deep Boost:** drops the **entire** standby list. "Free" memory rises sharply but **available memory
  barely changes** (standby pages already count as available), and apps start **more slowly** next time.
- **Smart Boost:** runs only when available memory is below a threshold (15% by default), and uses the light Boost (deep Boost would only discard cache).
- Each report separates "available memory gained" (the real benefit) from "cache cleared".
- **Trim working set** applies only to processes you choose, never in bulk. Windows manages memory
  well; trimming helps only occasionally, and pages are reloaded when the app needs them.

## 8. Known limitations

- The installer is **not signed by a trusted certificate authority**; SmartScreen and antivirus
  software may warn. See [Installation](../README.md#installation).
- Windows 10/11 x64 only.
- Tests that need administrator rights run through `scripts/admin-test.ps1` (UAC prompt every time).
