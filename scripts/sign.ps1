<#
.SYNOPSIS
  Ký một file bằng chứng chỉ tự ký của PC Manager (tạo bằng new-signing-cert.ps1).

.DESCRIPTION
  Tauri gọi script này cho exe của app, bộ cài và trình gỡ cài khi build bằng
  `npm run build:signed` (src-tauri/tauri.sign.conf.json > bundle.windows.signCommand).
  Dùng signtool của Windows SDK; không gửi gì ra mạng (không dùng máy chủ timestamp).

.EXAMPLE
  .\scripts\sign.ps1 "src-tauri\target\release\pc-manager.exe"
#>
param([Parameter(Mandatory)] [string] $File)
$ErrorActionPreference = "Stop"
$subject = "CN=PC Manager (self-signed), O=bachxuantrann"

$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
    Where-Object { $_.Subject -eq $subject -and $_.NotAfter -gt (Get-Date) } |
    Sort-Object NotAfter -Descending | Select-Object -First 1
if (-not $cert) {
    Write-Error "Chưa có chứng chỉ ký. Chạy .\scripts\new-signing-cert.ps1 trước."
}

$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending | Select-Object -First 1
if (-not $signtool) {
    Write-Error "Không tìm thấy signtool.exe (cài Windows SDK qua VS Build Tools)."
}

& $signtool.FullName sign /sha1 $cert.Thumbprint /s My /fd SHA256 /d "PC Manager" $File
if ($LASTEXITCODE -ne 0) { Write-Error "signtool lỗi ($LASTEXITCODE) khi ký $File" }
Write-Host "Đã ký: $File"
