<#
.SYNOPSIS
  Tạo chứng chỉ ký mã TỰ KÝ (self-signed) cho PC Manager — chạy một lần trên máy build.

.DESCRIPTION
  Chứng chỉ được tạo trong kho của người dùng hiện tại (Cert:\CurrentUser\My), KHÔNG cài vào
  "Trusted Root". Vì vậy:
    - File được ký có chữ ký + tên nhà phát hành, và sẽ báo hỏng nếu bị sửa sau khi ký.
    - Windows SmartScreen vẫn cảnh báo trên máy khác: chỉ chứng chỉ do nhà cung cấp được
      Windows tin cậy mới bỏ được cảnh báo đó.
  Không cần quyền Administrator. Chạy lại khi đã có chứng chỉ thì không tạo thêm.

.EXAMPLE
  .\scripts\new-signing-cert.ps1
#>
$ErrorActionPreference = "Stop"
$subject = "CN=PC Manager (self-signed), O=bachxuantrann"

$existing = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object { $_.Subject -eq $subject -and $_.NotAfter -gt (Get-Date) }
if ($existing) {
    Write-Host "Đã có chứng chỉ: $($existing[0].Thumbprint) (hết hạn $($existing[0].NotAfter.ToString('yyyy-MM-dd')))" -ForegroundColor Green
    exit 0
}

$cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $subject `
    -CertStoreLocation Cert:\CurrentUser\My `
    -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 `
    -KeyExportPolicy NonExportable `
    -NotAfter (Get-Date).AddYears(5)

Write-Host "Đã tạo chứng chỉ: $($cert.Thumbprint) (hết hạn $($cert.NotAfter.ToString('yyyy-MM-dd')))" -ForegroundColor Green
Write-Host "Khoá riêng không xuất được ra ngoài (NonExportable) — chỉ máy này ký được."
