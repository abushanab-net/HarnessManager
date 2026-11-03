# Every test on this computer: unit and CLI end-to-end tests, the router, then the desktop app driven through WebView2.
#   -Installed  also builds the installer, installs it silently into a temp folder, runs the UI suite against
#               the installed app and its bundled hrm.exe, and uninstalls it.
param([switch]$Installed)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot)
cargo build --workspace
if ($LASTEXITCODE) { exit $LASTEXITCODE }
cargo test --workspace
if ($LASTEXITCODE) { exit $LASTEXITCODE }
if (-not (Test-Path e2e/node_modules)) {
  Push-Location e2e; npm install; $npm = $LASTEXITCODE; Pop-Location
  if ($npm) { Remove-Item -Recurse -Force e2e/node_modules -ErrorAction SilentlyContinue; exit $npm }
}
node e2e/app.e2e.mjs
if ($LASTEXITCODE -or -not $Installed) { exit $LASTEXITCODE }

& "$PSScriptRoot/build.ps1"
if ($LASTEXITCODE) { exit $LASTEXITCODE }
$setup = Get-ChildItem target/release/bundle/nsis/*setup.exe | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$dir = Join-Path $env:TEMP "hrm-installed-$PID"
Start-Process $setup.FullName -ArgumentList '/S', "/D=$dir" -Wait
try {
  $env:HRM_E2E_APP_DIR = $dir
  node e2e/app.e2e.mjs
  $code = $LASTEXITCODE
} finally {
  Remove-Item Env:HRM_E2E_APP_DIR -ErrorAction SilentlyContinue
  Start-Process (Join-Path $dir 'uninstall.exe') -ArgumentList '/S' -Wait
}
exit $code
