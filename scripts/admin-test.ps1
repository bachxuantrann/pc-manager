<#
.SYNOPSIS
  Chạy test (hoặc app dev) với quyền Administrator, có hộp UAC xác nhận mỗi lần.

.DESCRIPTION
  Nếu chưa elevated: tự mở lại chính nó bằng "Run as administrator" (Windows hiện
  UAC, người dùng bấm Yes), chờ chạy xong rồi in log ra màn hình.
  Không lưu quyền, không tạo scheduled task — mỗi lần chạy đều phải xác nhận.

.EXAMPLE
  .\scripts\admin-test.ps1                 # cargo test --include-ignored (toàn bộ test, kể cả test cần admin)
  .\scripts\admin-test.ps1 -Filter memory  # chỉ test có "memory" trong tên
  .\scripts\admin-test.ps1 -Mode dev       # mở `npm run tauri dev` trong cửa sổ admin riêng
  .\scripts\admin-test.ps1 -Mode ram       # chạy bản release ẩn ở tray 30s rồi đo RAM khi rảnh
#>
param(
    [ValidateSet("test", "dev", "ram")] [string] $Mode = "test",
    [string] $Filter = "",
    [switch] $Elevated  # dùng nội bộ khi script tự mở lại chính nó
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$tauri = Join-Path $root "src-tauri"
$log = Join-Path $tauri "target\admin-test.log"
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (($env:Path -split ";") -notcontains $cargoBin) { $env:Path += ";$cargoBin" }

function Test-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    ([Security.Principal.WindowsPrincipal]$id).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# ---------------------------------------------------------------------------
# Phần chạy bên trong cửa sổ admin.
if (Test-Admin) {
    if ($Mode -eq "dev") {
        Set-Location $root
        Write-Host "PC Manager dev (Administrator) — đóng cửa sổ này để dừng." -ForegroundColor Green
        npm run tauri dev
        return
    }

    if ($Mode -eq "ram") {
        # Đo RAM bản release ở 2 trạng thái:
        #   1) cửa sổ đang mở (Rust + WebView2)  2) chạy ngầm ở tray (--tray, không có WebView).
        $exe = Join-Path $tauri "target\release\pc-manager.exe"
        if (-not (Test-Path $exe)) { "Chưa có $exe — chạy 'npm run tauri build' trước." | Out-File $log -Encoding utf8; exit 2 }
        $mb = { param($v) [math]::Round($v / 1MB, 1) }
        # WebView2 của app: msedgewebview2.exe dùng thư mục dữ liệu của app (identifier trong tauri.conf.json).
        $webviews = {
            Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" |
                Where-Object { $_.CommandLine -like "*com.pcmanager.app*" } |
                ForEach-Object { Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue }
        }
        $measure = {
            param([string] $title, [string[]] $appArgs, [int] $wait, [switch] $closeWindow)
            if ($appArgs) { $p = Start-Process $exe -ArgumentList $appArgs -PassThru } else { $p = Start-Process $exe -PassThru }
            if ($closeWindow) {
                # Giống người dùng bấm X: WM_CLOSE tới cửa sổ chính.
                Start-Sleep -Seconds 15
                $p.Refresh()
                [void] $p.CloseMainWindow()
            }
            Start-Sleep -Seconds $wait
            $p.Refresh()
            if ($p.HasExited) { return @("--- $title ---", "  LỖI: app đã thoát (mã $($p.ExitCode)) — lẽ ra phải chạy tiếp ở tray") }
            $wvCim = @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -like "*com.pcmanager.app*" })
            $types = ($wvCim | ForEach-Object { if ($_.CommandLine -match "--type=([\w-]+)") { $Matches[1] } else { "browser" } }) -join ", "
            $gpuOff = [bool]($wvCim | Where-Object { $_.CommandLine -like "*--disable-gpu*" })
            $wv = @(& $webviews)
            $wvWs = ($wv | Measure-Object WorkingSet64 -Sum).Sum
            $wvPriv = ($wv | Measure-Object PrivateMemorySize64 -Sum).Sum
            $out = @(
                "--- $title (sau $wait giây) ---",
                ("  Rust pc-manager.exe : Working set {0} MB, Private {1} MB" -f (& $mb $p.WorkingSet64), (& $mb $p.PrivateMemorySize64)),
                ("  WebView2 ({0} tiến trình): Working set {1} MB, Private {2} MB" -f $wv.Count, (& $mb $wvWs), (& $mb $wvPriv)),
                ("  WebView2 gồm: [{0}] · --disable-gpu: {1}" -f $types, $gpuOff),
                ("  TỔNG               : Working set {0} MB, Private {1} MB" -f (& $mb ($p.WorkingSet64 + $wvWs)), (& $mb ($p.PrivateMemorySize64 + $wvPriv)))
            )
            Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 3
            & $webviews | Stop-Process -Force -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 2
            $out
        }
        $lines = @("PC Manager release — đo RAM")
        $lines += & $measure "Cửa sổ đang mở (trang Tổng quan)" @() 20
        $lines += & $measure "Chạy ngầm ở tray (--tray)" @("--tray") 30
        $lines += & $measure "Mở cửa sổ rồi bấm X (xuống tray)" @() 20 -closeWindow
        $lines += "`n=== ADMIN-TEST EXIT CODE: 0 ==="
        $lines | Out-File $log -Encoding utf8
        if (-not $Elevated) { Get-Content $log -Encoding utf8 }
        exit 0
    }

    Set-Location $tauri
    New-Item -ItemType Directory -Force (Split-Path $log) | Out-Null
    $cargoArgs = "test -- --include-ignored --nocapture"
    if ($Filter) { $cargoArgs = "test $Filter -- --include-ignored --nocapture" }
    # cmd /c để cargo ghi thẳng UTF-8 ra file (PowerShell 5.1 sẽ đổi sang UTF-16).
    cmd /c "cargo $cargoArgs > `"$log`" 2>&1"
    $code = $LASTEXITCODE
    Add-Content -Path $log -Value "`n=== ADMIN-TEST EXIT CODE: $code ===" -Encoding utf8
    if (-not $Elevated) { Get-Content $log -Encoding utf8 }
    exit $code
}

# ---------------------------------------------------------------------------
# Chưa có quyền: mở lại chính mình với UAC.
if ($Mode -eq "dev") {
    Write-Host "Mở cửa sổ admin cho tauri dev — hãy bấm Yes ở hộp UAC." -ForegroundColor Yellow
    Start-Process powershell -Verb RunAs -ArgumentList @(
        "-NoProfile", "-ExecutionPolicy", "Bypass", "-NoExit",
        "-File", "`"$PSCommandPath`"", "-Mode", "dev"
    )
    return
}

Write-Host "Cần quyền Administrator — hãy bấm Yes ở hộp UAC…" -ForegroundColor Yellow
if (Test-Path $log) { Remove-Item $log -Force }
$argList = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "`"$PSCommandPath`"", "-Mode", $Mode, "-Elevated")
if ($Filter) { $argList += @("-Filter", $Filter) }
try {
    $p = Start-Process powershell -Verb RunAs -WindowStyle Minimized -ArgumentList $argList -Wait -PassThru
} catch {
    Write-Host "Đã huỷ: không được cấp quyền Administrator." -ForegroundColor Red
    exit 1223  # ERROR_CANCELLED
}

if (Test-Path $log) {
    Get-Content $log -Encoding utf8
} else {
    Write-Host "Không thấy log tại $log" -ForegroundColor Red
}
exit $p.ExitCode
