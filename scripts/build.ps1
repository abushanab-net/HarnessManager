# Release build: hrm.exe, the router inside it, and the Windows installer for the desktop app.
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot)
cargo build --release -p hrm
if ($LASTEXITCODE) { exit $LASTEXITCODE }
$triple = (rustc -vV | Select-String '^host: (.+)$').Matches[0].Groups[1].Value
New-Item -ItemType Directory -Force app/src-tauri/binaries | Out-Null
# the installer ships hrm.exe next to the app, so Start and the CLI use the same core
Copy-Item target/release/hrm.exe "app/src-tauri/binaries/hrm-$triple.exe" -Force
Push-Location app
if (-not (Test-Path node_modules)) { npm install; if ($LASTEXITCODE) { Remove-Item -Recurse -Force node_modules -ErrorAction SilentlyContinue; Pop-Location; exit 1 } }
npx tauri build --config src-tauri/tauri.release.conf.json
$code = $LASTEXITCODE
Pop-Location
if ($code) { exit $code }
Get-ChildItem target/release/bundle -Recurse -Include *.exe | Select-Object FullName, Length
exit $code
