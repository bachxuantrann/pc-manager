<#
.SYNOPSIS
  Tạo cặp khoá ký cập nhật (minisign) cho tính năng tự cập nhật — làm MỘT lần.

.DESCRIPTION
  - Khoá riêng: %USERPROFILE%\.tauri\pc-manager-updater.key (NGOÀI repo). Mật khẩu do bạn gõ khi được hỏi.
  - Khoá công khai: ghi vào src-tauri/tauri.conf.json (plugins.updater.pubkey) — commit file này.
  - Nếu có GitHub CLI đã đăng nhập: đưa khoá riêng + mật khẩu vào secret TAURI_SIGNING_PRIVATE_KEY và
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD của repo. Không có thì in hướng dẫn thêm secret trên web.

  GIỮ KỸ khoá riêng + mật khẩu (vd. trình quản lý mật khẩu). Mất khoá => các bản đã cài không cập nhật được
  lên bản ký bằng khoá mới nữa (phải cài tay một lần). Lộ khoá => người khác ký được bản cập nhật giả.

.EXAMPLE
  .\scripts\new-updater-key.ps1
#>
param([string] $Repo = "bachxuantrann/pc-manager")
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$conf = Join-Path $root "src-tauri\tauri.conf.json"
$keyDir = Join-Path $env:USERPROFILE ".tauri"
$key = Join-Path $keyDir "pc-manager-updater.key"

if (Test-Path $key) {
    throw "Đã có khoá ở $key — không tạo đè (tạo khoá mới làm các bản đã cài không cập nhật được nữa). Xoá tay nếu thật sự muốn đổi khoá."
}
New-Item -ItemType Directory -Force $keyDir | Out-Null

Write-Host "Tạo khoá ký cập nhật — hãy đặt mật khẩu khi được hỏi (gõ 2 lần)." -ForegroundColor Yellow
Push-Location $root
try {
    npx tauri signer generate -w "$key"
    if ($LASTEXITCODE) { throw "tauri signer generate lỗi (mã $LASTEXITCODE)" }
} finally {
    Pop-Location
}
$pub = (Get-Content "$key.pub" -Raw).Trim()

# Ghi khoá công khai vào tauri.conf.json, giữ nguyên định dạng file (chỉ thay đúng dòng pubkey).
$text = [IO.File]::ReadAllText($conf)
$re = [regex]'("pubkey":\s*")[^"]*(")'
$new = $re.Replace($text, [Text.RegularExpressions.MatchEvaluator] { param($mm) $mm.Groups[1].Value + $pub + $mm.Groups[2].Value }, 1)
if ($new -eq $text) { throw "Không tìm thấy plugins.updater.pubkey trong $conf" }
[IO.File]::WriteAllText($conf, $new, (New-Object Text.UTF8Encoding $false))
Write-Host "Đã ghi khoá công khai vào src-tauri/tauri.conf.json — nhớ commit file này." -ForegroundColor Green

$gh = (Get-Command gh -ErrorAction SilentlyContinue).Source
if (-not $gh -and (Test-Path "C:\Program Files\GitHub CLI\gh.exe")) { $gh = "C:\Program Files\GitHub CLI\gh.exe" }
$ghOk = $false
if ($gh) { & $gh auth status *> $null; $ghOk = ($LASTEXITCODE -eq 0) }

if ($ghOk) {
    Get-Content $key -Raw | & $gh secret set TAURI_SIGNING_PRIVATE_KEY --repo $Repo
    if ($LASTEXITCODE) { throw "Không đặt được secret TAURI_SIGNING_PRIVATE_KEY" }
    $sec = Read-Host "Gõ lại mật khẩu khoá để lưu vào secret TAURI_SIGNING_PRIVATE_KEY_PASSWORD" -AsSecureString
    $plain = [Runtime.InteropServices.Marshal]::PtrToStringBSTR([Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec))
    $plain | & $gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo $Repo
    if ($LASTEXITCODE) { throw "Không đặt được secret TAURI_SIGNING_PRIVATE_KEY_PASSWORD" }
    Write-Host "Đã đặt 2 secret cho repo $Repo." -ForegroundColor Green
} else {
    Write-Host ""
    Write-Host "Chưa có GitHub CLI — thêm 2 secret bằng tay:" -ForegroundColor Yellow
    Write-Host "  1. Mở https://github.com/$Repo/settings/secrets/actions/new"
    Write-Host "  2. Name: TAURI_SIGNING_PRIVATE_KEY   Secret: toàn bộ nội dung file $key"
    Write-Host "     (đang mở file này trong Notepad — Ctrl+A, Ctrl+C rồi dán)"
    Write-Host "  3. New repository secret lần nữa: Name: TAURI_SIGNING_PRIVATE_KEY_PASSWORD   Secret: mật khẩu vừa đặt"
    Start-Process notepad.exe -ArgumentList "`"$key`""
}
Write-Host ""
Write-Host "Sao lưu file $key và mật khẩu ở nơi an toàn. KHÔNG commit file .key." -ForegroundColor Yellow
