<#
.SYNOPSIS
  (Tuỳ chọn) Tạo chứng chỉ TỰ KÝ riêng cho GitHub Actions và cất vào GitHub Secrets,
  để workflow Release tự ký bộ cài.

.DESCRIPTION
  Chứng chỉ trên máy bạn (new-signing-cert.ps1) có khoá riêng không xuất được, nên máy chủ
  GitHub không dùng được. Script này tạo một chứng chỉ KHÁC, xuất ra PFX với mật khẩu ngẫu
  nhiên, đưa thẳng vào 2 secret của repo bằng `gh secret set`, rồi xoá file PFX và gỡ chứng
  chỉ khỏi máy. Khoá riêng và mật khẩu không được in ra màn hình hay lưu lại trên máy.

  Yêu cầu: GitHub CLI đã đăng nhập (`gh auth status`), có quyền admin trên repo.
  Cùng giới hạn như mọi chứng chỉ tự ký: SmartScreen vẫn cảnh báo trên máy người dùng.

.EXAMPLE
  .\scripts\new-ci-signing-cert.ps1
#>
param([string] $Repo = "bachxuantrann/pc-manager")
$ErrorActionPreference = "Stop"
$subject = "CN=PC Manager (self-signed), O=bachxuantrann"   # phải khớp scripts/sign.ps1

$gh = (Get-Command gh -ErrorAction SilentlyContinue).Source
if (-not $gh) { $gh = "C:\Program Files\GitHub CLI\gh.exe" }
& $gh auth status *> $null
if ($LASTEXITCODE -ne 0) { throw "GitHub CLI chưa đăng nhập. Chạy: gh auth login" }

$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $subject `
    -CertStoreLocation Cert:\CurrentUser\My -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 `
    -KeyExportPolicy Exportable -NotAfter (Get-Date).AddYears(5) -FriendlyName "PC Manager CI signing"
$pfx = Join-Path ([IO.Path]::GetTempPath()) ("pcm-ci-" + [guid]::NewGuid() + ".pfx")
try {
    # Mật khẩu ngẫu nhiên 32 byte, chỉ tồn tại trong bộ nhớ của script.
    $bytes = New-Object byte[] 32
    [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    $password = [Convert]::ToBase64String($bytes)
    $secure = ConvertTo-SecureString $password -AsPlainText -Force
    Export-PfxCertificate -Cert $cert -FilePath $pfx -Password $secure | Out-Null

    [Convert]::ToBase64String([IO.File]::ReadAllBytes($pfx)) | & $gh secret set SIGNING_PFX_BASE64 --repo $Repo
    if ($LASTEXITCODE -ne 0) { throw "Không đặt được secret SIGNING_PFX_BASE64" }
    $password | & $gh secret set SIGNING_PFX_PASSWORD --repo $Repo
    if ($LASTEXITCODE -ne 0) { throw "Không đặt được secret SIGNING_PFX_PASSWORD" }
    Write-Host "Đã cất chứng chỉ CI vào secrets của $Repo (thumbprint $($cert.Thumbprint), hết hạn $($cert.NotAfter.ToString('yyyy-MM-dd')))." -ForegroundColor Green
} finally {
    Remove-Item $pfx -Force -ErrorAction SilentlyContinue
    # Gỡ khỏi máy: bản này chỉ dành cho CI. Máy bạn vẫn dùng chứng chỉ của new-signing-cert.ps1.
    Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey -ErrorAction SilentlyContinue
}
