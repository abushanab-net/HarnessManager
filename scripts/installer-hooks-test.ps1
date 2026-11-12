# Tests app/src-tauri/installer-hooks.nsh with a really locked hrm.exe: a copy of cmd.exe that keeps running while the
# installer runs. Needs the NSIS that a Tauri build downloads (%LOCALAPPDATA%\tauri\NSIS).
# Usage: pwsh scripts/installer-hooks-test.ps1
param([string]$Hooks = (Join-Path $PSScriptRoot '..\app\src-tauri\installer-hooks.nsh'))
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$Hooks = (Resolve-Path -LiteralPath $Hooks).Path
$makensis = Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe'
if (-not (Test-Path $makensis)) { throw "makensis not found at $makensis; build the installer once (scripts/build.ps1) so Tauri downloads NSIS." }
$work = Join-Path ([IO.Path]::GetTempPath()) "hrm-installer-hooks-test-$PID"
$inst = Join-Path $work 'install dir'   # a space, like "Harness Manager"
Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
New-Item $inst -ItemType Directory | Out-Null

# Copied out of System32 first: makensis is 32-bit, and Windows would hand it the SysWOW64 file under the same path.
$payload = Join-Path $work 'payload.exe'
Copy-Item "$env:WINDIR\System32\where.exe" $payload
$oldExe = "$env:WINDIR\System32\cmd.exe"
function Hash($p) { (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash }
function Build($out, [switch]$WithHooks) {
    $defs = @("/DPAYLOAD=$payload", "/DOUT=$out")
    if ($WithHooks) { $defs += "/DHOOKS=$Hooks" }
    $log = & $makensis /V2 @defs (Join-Path $here 'installer-hooks-test.nsi') 2>&1
    if ($LASTEXITCODE -ne 0) { $log | Write-Host; throw "makensis failed ($LASTEXITCODE)" }
}
function Install($exe) { Start-Process -FilePath $exe -ArgumentList "/S", "/D=$inst" -Wait }
function Olds { @(Get-ChildItem -LiteralPath $inst -Filter 'hrm.old-*.exe') }
$fail = 0
function Check($name, $ok) { if ($ok) { "PASS  $name" } else { "FAIL  $name"; $script:fail++ } }

$plain = Join-Path $work 'plain.exe'; $hooked = Join-Path $work 'hooked.exe'
Build $plain
Build $hooked -WithHooks
"built both installers (the hook file compiles)"

# A running hrm.exe: cmd.exe under that name, waiting on a long ping.
Copy-Item $oldExe (Join-Path $inst 'hrm.exe')
$session = Start-Process -FilePath (Join-Path $inst 'hrm.exe') -ArgumentList '/c', 'ping -n 900 127.0.0.1 >nul' -WindowStyle Hidden -PassThru
Start-Sleep -Milliseconds 800
try {
    Install $plain
    Check 'without the hook, a running hrm.exe is not replaced (the reported failure)' ((Hash (Join-Path $inst 'hrm.exe')) -eq (Hash $oldExe))

    Install $hooked
    Check 'with the hook, the new hrm.exe is written' ((Hash (Join-Path $inst 'hrm.exe')) -eq (Hash $payload))
    Check 'the running copy is moved aside, unchanged' ((Olds).Count -eq 1 -and (Hash (Olds)[0].FullName) -eq (Hash $oldExe))
    Check 'the open session keeps running' (-not $session.HasExited)

    Install $hooked
    Check 'a second update replaces hrm.exe in place and adds no copy' ((Olds).Count -eq 1 -and (Hash (Join-Path $inst 'hrm.exe')) -eq (Hash $payload))
    Check 'the session still runs after the second update' (-not $session.HasExited)
}
finally {
    # Only the processes this test started: the fake session and its ping.
    Get-CimInstance Win32_Process -Filter "ParentProcessId=$($session.Id)" | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Stop-Process -Id $session.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 800
}

Install $hooked
Check 'once the session is closed, the next update deletes the copy' ((Olds).Count -eq 0)

# Nothing running and no hrm.exe yet: a first install.
Remove-Item -LiteralPath (Join-Path $inst 'hrm.exe') -Force
Install $hooked
Check 'a first install writes hrm.exe and leaves no copy' ((Hash (Join-Path $inst 'hrm.exe')) -eq (Hash $payload) -and (Olds).Count -eq 0)

Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
if ($fail) { "$fail check(s) failed"; exit 1 } else { 'all checks passed' }
