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
# the tests of several computers run here too: a seeded sync folder, a second computer paired through hrm, and a hub
# started from target/debug/hrm-hub.exe
$env:HRM_E2E_SYNC = '1'; $env:HRM_E2E_PEERS = '1'; $env:HRM_E2E_HUB = '1'
node e2e/app.e2e.mjs
if ($LASTEXITCODE -or -not $Installed) { exit $LASTEXITCODE }

& "$PSScriptRoot/build.ps1"
if ($LASTEXITCODE) { exit $LASTEXITCODE }
$setup = Get-ChildItem target/release/bundle/nsis/*setup.exe | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$dir = Join-Path $env:TEMP "hrm-installed-$PID"
$pathKey = "Software\HarnessManager-PathTest-$([Guid]::NewGuid())"
$priorHome = $env:HRM_HOME
$priorKey = $env:HRM_PATH_TEST_KEY
$env:HRM_HOME = Join-Path $dir 'test-data'
$env:HRM_PATH_TEST_KEY = $pathKey
try {
  [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($pathKey).Dispose()
  Start-Process $setup.FullName -ArgumentList '/S', "/D=$dir" -WindowStyle Hidden -Wait
  $env:HRM_E2E_APP_DIR = $dir
  Remove-Item Env:HRM_E2E_HUB # the installer carries no hrm-hub.exe: the hub runs on a server
  node e2e/app.e2e.mjs
  $code = $LASTEXITCODE
} finally {
  Remove-Item Env:HRM_E2E_APP_DIR -ErrorAction SilentlyContinue
  try {
    $uninstall = Join-Path $dir 'uninstall.exe'
    if (Test-Path -LiteralPath $uninstall) { Start-Process $uninstall -ArgumentList '/S' -WindowStyle Hidden -Wait }
  } finally {
    $env:HRM_HOME = $priorHome
    $env:HRM_PATH_TEST_KEY = $priorKey
    [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKeyTree($pathKey, $false)
  }
}
exit $code
