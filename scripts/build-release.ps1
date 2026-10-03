# Сборка релиза с подписью обновлений. Запускает ПОЛЬЗОВАТЕЛЬ:
#   powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 "что нового"
# Пароль ключа вводится скрыто и живёт только в переменной этого процесса.
param([string]$Notes = "")
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

$keyPath = Join-Path $env:USERPROFILE ".tauri\onecfork.key"
if (-not (Test-Path $keyPath)) { throw "Нет ключа подписи: $keyPath" }

$secure = Read-Host "Пароль ключа подписи" -AsSecureString
$bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
try {
    $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
    npm run check
    if ($LASTEXITCODE -ne 0) { throw "Проверки не прошли — релиз не собирается" }
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw "Сборка не удалась" }
    node scripts/make-latest-json.mjs $Notes
    if ($LASTEXITCODE -ne 0) { throw "Не удалось записать latest.json" }
    Write-Host "Готово. Файлы релиза: src-tauri\target\release\bundle\nsis\" -ForegroundColor Green
}
finally {
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
}
