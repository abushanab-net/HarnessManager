# Exercise the actual NSIS hooks and sidecar with a locked executable and an isolated user PATH value.
# Usage: pwsh scripts/installer-hooks-test.ps1 [-Sidecar <path-to-newly-built-hrm.exe>]
param(
    [string]$Hooks = (Join-Path $PSScriptRoot '..\app\src-tauri\installer-hooks.nsh'),
    [string]$Sidecar
)
$ErrorActionPreference = 'Stop'
$Hooks = (Resolve-Path -LiteralPath $Hooks).Path
$makensis = Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe'
if (-not (Test-Path $makensis)) { throw "makensis not found at $makensis; build the installer once (scripts/build.ps1) so Tauri downloads NSIS." }
if (-not $Sidecar) {
    Push-Location (Split-Path $PSScriptRoot)
    try {
        cargo build --locked -p hrm
        if ($LASTEXITCODE -ne 0) { throw "building the hrm sidecar failed ($LASTEXITCODE)" }
        $target = (cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).target_directory
        $Sidecar = Join-Path $target 'debug\hrm.exe'
    } finally { Pop-Location }
}
$Sidecar = (Resolve-Path -LiteralPath $Sidecar).Path
$id = [Guid]::NewGuid().ToString()
$testKey = "Software\HarnessManager-PathTest-$id"
$work = Join-Path ([IO.Path]::GetTempPath()) "hrm-installer-hooks-test-$id"
$inst = Join-Path $work 'install dir'
New-Item -Path $inst -ItemType Directory | Out-Null
$work = (Resolve-Path -LiteralPath $work).ProviderPath
$temp = (Resolve-Path -LiteralPath ([IO.Path]::GetTempPath())).ProviderPath.TrimEnd('\') + '\'
if (-not $work.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase)) { throw "test folder is outside $temp" }

# makensis is 32-bit. Keep fixture inputs outside paths subject to WOW64 redirection.
$payload = Join-Path $work 'payload.exe'
Copy-Item -LiteralPath $Sidecar -Destination $payload
$oldExe = "$env:WINDIR\System32\cmd.exe"
function Hash($p) { (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash }
function Build($out, [switch]$WithHooks) {
    $defs = @("/DPAYLOAD=$payload", "/DOUT=$out")
    if ($WithHooks) { $defs += "/DHOOKS=$Hooks" }
    $log = & $makensis /V2 @defs (Join-Path $PSScriptRoot 'installer-hooks-test.nsi') 2>&1
    if ($LASTEXITCODE -ne 0) { $log | Write-Host; throw "makensis failed ($LASTEXITCODE)" }
}
function Run-Installer($exe, $location) {
    $proc = Start-Process -FilePath $exe -ArgumentList '/S', $location -WindowStyle Hidden -Wait -PassThru
    if ($proc.ExitCode -ne 0) { throw "$exe exited with code $($proc.ExitCode)" }
}
function Install($exe) { Run-Installer $exe "/D=$inst" }
function Olds { @(Get-ChildItem -LiteralPath $inst -Filter 'hrm.old-*.exe') }
function RawPath { $key.GetValue('Path', $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) }
$fail = 0
function Check($name, $ok) { if ($ok) { "PASS  $name" } else { "FAIL  $name"; $script:fail++ } }

$priorHome = $env:HRM_HOME
$priorKey = $env:HRM_PATH_TEST_KEY
$env:HRM_HOME = Join-Path $work 'app-data'
$env:HRM_PATH_TEST_KEY = $testKey
$key = $null
$session = $null
try {
    $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($testKey)
    $original = '%USERPROFILE%\bin;C:\' + ('long;' * 500) + $inst + '-other;;'
    $expand = [Microsoft.Win32.RegistryValueKind]::ExpandString
    $key.SetValue('Path', $original, $expand)
    $added = "$original;$inst"

    $plain = Join-Path $work 'plain.exe'
    $hooked = Join-Path $work 'hooked.exe'
    Build $plain
    Build $hooked -WithHooks
    'built both installers (the hook file compiles)'

    Copy-Item $oldExe (Join-Path $inst 'hrm.exe')
    $session = Start-Process -FilePath (Join-Path $inst 'hrm.exe') -ArgumentList '/c', 'ping -n 900 127.0.0.1 >nul' -WindowStyle Hidden -PassThru
    Start-Sleep -Milliseconds 800
    try {
        Install $plain
        Check 'without the hook, the running hrm.exe is not replaced' ((Hash (Join-Path $inst 'hrm.exe')) -eq (Hash $oldExe))
        Check 'without the hook, PATH is untouched' ((RawPath) -ceq $original)

        Install $hooked
        Check 'with the hook, the new hrm.exe is written' ((Hash (Join-Path $inst 'hrm.exe')) -eq (Hash $payload))
        Check 'the running copy is moved aside, unchanged' ((Olds).Count -eq 1 -and (Hash (Olds)[0].FullName) -eq (Hash $oldExe))
        Check 'the open session keeps running' (-not $session.HasExited)
        Check 'install appends the folder without changing long PATH bytes or type' ((RawPath) -ceq $added -and $key.GetValueKind('Path') -eq $expand)

        Install $hooked
        Check 'a second update replaces hrm.exe in place and adds no copy' ((Olds).Count -eq 1 -and (Hash (Join-Path $inst 'hrm.exe')) -eq (Hash $payload))
        Check 'the session still runs after the second update' (-not $session.HasExited)
        Check 'a second update leaves PATH unchanged' ((RawPath) -ceq $added)
    } finally {
        # Only the processes this test started: the fake session and its ping.
        Get-CimInstance Win32_Process -Filter "ParentProcessId=$($session.Id)" | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
        Stop-Process -Id $session.Id -Force -ErrorAction SilentlyContinue
        $null = $session.WaitForExit(5000)
    }

    Install $hooked
    Check 'once the session is closed, the next update deletes the copy' ((Olds).Count -eq 0)
    Run-Installer (Join-Path $inst 'uninstall.exe') "_?=$inst"
    Check 'uninstall restores every byte of the original long PATH and its type' ((RawPath) -ceq $original -and $key.GetValueKind('Path') -eq $expand)

    $custom = "$original;$($inst.ToUpperInvariant())\"
    $string = [Microsoft.Win32.RegistryValueKind]::String
    $key.SetValue('Path', $custom, $string)
    Install $hooked
    Check 'a first install writes hrm.exe and leaves no old copy' ((Hash (Join-Path $inst 'hrm.exe')) -eq (Hash $payload) -and (Olds).Count -eq 0)
    Check 'case and trailing slash do not create a duplicate' ((RawPath) -ceq $custom -and $key.GetValueKind('Path') -eq $string)
    Run-Installer (Join-Path $inst 'uninstall.exe') "_?=$inst"
    Check 'uninstall leaves the pre-existing matching entry' ((RawPath) -ceq $custom -and $key.GetValueKind('Path') -eq $string)

    $key.DeleteValue('Path')
    Install $hooked
    Check 'install creates a missing PATH' ((RawPath) -ceq $inst -and $key.GetValueKind('Path') -eq $expand)
    Run-Installer (Join-Path $inst 'uninstall.exe') "_?=$inst"
    Check 'uninstall restores a missing PATH' ($null -eq (RawPath))
} finally {
    $env:HRM_HOME = $priorHome
    $env:HRM_PATH_TEST_KEY = $priorKey
    if ($key) { $key.Dispose() }
    [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKeyTree($testKey, $false)
    Remove-Item -LiteralPath $work -Recurse -Force
}
if ($fail) { "$fail check(s) failed"; exit 1 }
'all checks passed'
