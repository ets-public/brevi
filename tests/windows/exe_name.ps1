# brevi build without -o on Windows writes FILE.exe, which a shell or Explorer can start (a program named
# without .exe fails with "%1 is not a valid Win32 application"), on the typed build and both dynamic paths,
# through the result cache and without it. An -o name is used as given, as rustc does. Throws on a failure.
$ErrorActionPreference = "Continue"
$root = (Get-Location).Path
$brevi = if ($env:BREVI) { $env:BREVI } else { Join-Path $root "target\release\brevi.exe" }
$brevi = (Resolve-Path $brevi).Path

# Every build prints its {"path":...} line, the typed one too.
$env:BREVI_PATH = "1"

function Lines($raw) { return (($raw | ForEach-Object { "$_" }) -join "`n") }

# name: the test file; path: the build it must take; envs: set for the build only; flags: extra build flags
function Check-Default([string]$name, [string]$path, [hashtable]$envs, [string[]]$flags, [string]$label) {
    $file = Join-Path $root "tests\windows\$name"
    $stem = [System.IO.Path]::GetFileNameWithoutExtension($name)
    $want = Lines (& python $file 2>&1)
    $dir = Join-Path ([System.IO.Path]::GetTempPath()) ("brevi-exe-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $dir | Out-Null
    Push-Location $dir
    try {
        foreach ($k in $envs.Keys) { Set-Item "env:$k" $envs[$k] }
        $b = Lines (& $brevi build $file @flags 2>&1)
        $code = $LASTEXITCODE
        foreach ($k in $envs.Keys) { Remove-Item "env:$k" -ErrorAction SilentlyContinue }
        if ($code -ne 0) { throw "build of $name ($label) exited $code`n$b" }
        if ($b -notmatch ('"path":"' + $path + '"')) { throw "build of $name ($label) did not take the $path build`n$b" }
        if (-not (Test-Path (Join-Path $dir "$stem.exe"))) { throw "build of $name ($label) wrote no $stem.exe: $((Get-ChildItem $dir).Name -join ', ')" }
        if (Test-Path (Join-Path $dir $stem)) { throw "build of $name ($label) also wrote $stem without .exe" }
        # Started by name, as a person types it: cmd finds plain.exe through PATHEXT.
        $got = Lines (& cmd /c $stem 2>&1)
        if ($LASTEXITCODE -ne 0 -or $got.Trim() -ne $want.Trim()) { throw "$stem ($label) started from cmd exited $LASTEXITCODE`n$got`npython:`n$want" }
        $got = Lines (& (Join-Path $dir "$stem.exe") 2>&1)
        if ($LASTEXITCODE -ne 0 -or $got.Trim() -ne $want.Trim()) { throw "$stem.exe ($label) exited $LASTEXITCODE`n$got" }
    } finally {
        Pop-Location
        Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
    }
    Write-Host "PASS build $name writes $stem.exe ($label)"
}

Check-Default "plain.py" "typed" @{} @() "typed, cached"
Check-Default "plain.py" "typed" @{} @("--no-cache") "typed, no cache"
Check-Default "exe_name_dynamic.py" "dynamic" @{} @() "sent to the dynamic build"
Check-Default "exe_name_dynamic.py" "dynamic" @{} @("--no-cache") "sent to the dynamic build, no cache"
Check-Default "plain.py" "dynamic" @{ BREVI_DYN = "1" } @("--no-cache") "BREVI_DYN=1"

# -o is used as given (as rustc does): -o app writes app, -o app.exe writes app.exe.
$dir = Join-Path ([System.IO.Path]::GetTempPath()) ("brevi-exe-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $dir | Out-Null
try {
    $file = Join-Path $root "tests\windows\plain.py"
    foreach ($o in @("app", "app.exe")) {
        $b = Lines (& $brevi build $file -o (Join-Path $dir $o) 2>&1)
        if ($LASTEXITCODE -ne 0) { throw "build -o $o exited $LASTEXITCODE`n$b" }
    }
    $names = (Get-ChildItem $dir -File | Where-Object { $_.Extension -ne ".pdb" }).Name | Sort-Object
    if (($names -join ",") -ne "app,app.exe") { throw "-o app and -o app.exe wrote: $($names -join ', ')" }
    $got = Lines (& (Join-Path $dir "app.exe") 2>&1)
    if ($LASTEXITCODE -ne 0 -or $got.Trim() -ne "45") { throw "app.exe exited $LASTEXITCODE`n$got" }
} finally {
    Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
}
Write-Host "PASS build -o uses the name as given"
Remove-Item env:BREVI_PATH -ErrorAction SilentlyContinue
