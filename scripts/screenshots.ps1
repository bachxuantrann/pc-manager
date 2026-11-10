<#
.SYNOPSIS
  Chụp ảnh màn hình các trang cho README bằng Edge headless, dùng chế độ demo (dữ liệu giả).

.DESCRIPTION
  Cần server dev đang chạy:  npm run dev   (http://localhost:1420)
  Mỗi trang mở http://localhost:1420/?demo&page=<trang> — dữ liệu giả trong src/dev/demo.ts,
  không đọc gì từ máy thật. Ảnh lưu vào docs/screenshots/<trang>.png (giao diện tối).

.EXAMPLE
  .\scripts\screenshots.ps1
  .\scripts\screenshots.ps1 -Light      # giao diện sáng
#>
param([switch] $Light, [string] $Url = "http://localhost:1420")
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root "docs\screenshots"
New-Item -ItemType Directory -Force $out | Out-Null

$edge = @("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe", "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe") |
    Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { throw "Không tìm thấy Microsoft Edge." }
try { Invoke-WebRequest $Url -UseBasicParsing -TimeoutSec 5 | Out-Null } catch { throw "Server dev chưa chạy ở $Url (chạy: npm run dev)." }

$profile = Join-Path ([IO.Path]::GetTempPath()) "pcm-screenshot-profile"
$pages = "dashboard", "cleaner", "processes", "apps", "disk", "settings"
foreach ($p in $pages) {
    $file = Join-Path $out "$p.png"
    $args = @(
        "--headless=new", "--disable-gpu", "--hide-scrollbars", "--no-first-run",
        "--user-data-dir=$profile", "--window-size=1280,800", "--force-device-scale-factor=1",
        # Tổng quan: chạy 62 giây thời gian ảo để biểu đồ "60 giây gần nhất" có đủ dữ liệu.
        "--virtual-time-budget=$(if ($p -eq 'dashboard') { 62000 } else { 6000 })", "--screenshot=$file"
    )
    if (-not $Light) { $args += "--force-dark-mode", "--blink-settings=preferredColorScheme=0" }
    $args += "$Url/?demo&page=$p"
    Start-Process $edge -ArgumentList $args -Wait -WindowStyle Hidden
    Write-Host "Đã chụp: $file"
}
Remove-Item $profile -Recurse -Force -ErrorAction SilentlyContinue
