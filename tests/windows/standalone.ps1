# brevi build --standalone on Windows: one .exe that carries its own CPython (src/pybundle.py) and runs where
# no Python is installed. Each program is built with --standalone and run with Python out of reach (PATH holds
# only Windows' own folders, BREVI_PYTHON names nothing, no PYTHONHOME or PYTHONPATH, a fresh cache): its output
# must equal CPython's (tests/standalone/*.expected, or `python FILE` here). Then: a second run reuses the
# cache, two first runs racing on an empty cache both succeed, and a file with bytes appended after the build
# (as code signing does) is refused, never run on the installed Python.
# Run from the repository root (check.ps1 runs it); BREVI names the brevi.exe to check.
$ErrorActionPreference = "Continue"
$root = (Get-Location).Path
$brevi = if ($env:BREVI) { $env:BREVI } else { Join-Path $root "target\release\brevi.exe" }
$brevi = (Resolve-Path $brevi).Path
$work = Join-Path ([System.IO.Path]::GetTempPath()) ("brevi-sa-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Path $work | Out-Null

function Lines($raw) { return (($raw | ForEach-Object { "$_" }) -join "`n").Trim() }

# Runs $exe with Python out of reach and the cache at $cache; returns (exit code, output).
function Run-Bare([string]$exe, [string]$cache, [string[]]$exeArgs = @()) {
    $keep = @{}
    foreach ($n in @("PATH", "PYTHONHOME", "PYTHONPATH", "BREVI_PYTHON", "BREVI_PY_CACHE", "VIRTUAL_ENV")) { $keep[$n] = [Environment]::GetEnvironmentVariable($n) }
    try {
        $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
        Remove-Item Env:PYTHONHOME, Env:PYTHONPATH, Env:VIRTUAL_ENV -ErrorAction SilentlyContinue
        $env:BREVI_PYTHON = Join-Path $work "no-python\python.exe"
        $env:BREVI_PY_CACHE = $cache
        $raw = & $exe @exeArgs 2>&1
        return @($LASTEXITCODE, (Lines $raw))
    } finally {
        foreach ($n in $keep.Keys) { [Environment]::SetEnvironmentVariable($n, $keep[$n]) }
    }
}

function Check-Standalone([string]$file, [string]$want, [string]$dyn) {
    $name = [System.IO.Path]::GetFileNameWithoutExtension($file) + $(if ($dyn) { "-dyn" } else { "" })
    $exe = Join-Path $work "$name.exe"
    $old = $env:BREVI_DYN
    if ($dyn) { $env:BREVI_DYN = "1" }
    # Typed as an array: a one-item array from an if expression would unroll to a string.
    [string[]]$flags = if ($dyn) { @("--standalone") } else { @("--native", "--standalone") }
    $b = Lines (& $brevi build $file @flags -o $exe 2>&1)
    $code = $LASTEXITCODE
    $env:BREVI_DYN = $old
    if ($code -ne 0 -or $b -notmatch 'note: bundled Python') { throw "standalone build of $name exited $code`n$b" }
    $cache = Join-Path $work "cache-$name"
    $r = Run-Bare $exe $cache
    if ($r[0] -ne 0 -or $r[1] -ne $want) { throw "standalone $name exited $($r[0])`nwant:`n$want`ngot:`n$($r[1])" }
    $ok = @(Get-ChildItem -Path $cache -Filter ".brevi-ok" -Recurse -Force -ErrorAction SilentlyContinue)
    if ($ok.Count -ne 1) { throw "standalone $name did not unpack its Python into $cache" }
    $r = Run-Bare $exe $cache
    if ($r[0] -ne 0 -or $r[1] -ne $want) { throw "standalone $name (second run, cache reused) exited $($r[0])`n$($r[1])" }
    Write-Host "PASS standalone $name"
    return $exe
}

try {
    $sa = Join-Path $root "tests\standalone\stdlib.py"
    $want = (Get-Content (Join-Path $root "tests\standalone\stdlib.expected") -Raw).Replace("`r`n", "`n").Trim()
    $exe = Check-Standalone $sa $want ""
    Check-Standalone $sa $want "1" | Out-Null
    foreach ($t in @("bridge_yaml.py", "duckdb_native.py", "onnx_native.py")) {
        $f = Join-Path $root "tests\windows\$t"
        Check-Standalone $f (Lines (& python $f 2>&1)) "" | Out-Null
    }
    $f = Join-Path $root "tests\windows\bridge_yaml_dynamic.py"
    Check-Standalone $f (Lines (& python $f 2>&1)) "1" | Out-Null

    # Two first runs at once on an empty cache: one unpacks, the other uses its copy (or its own); both answer.
    $cache = Join-Path $work "cache-race"
    $keep = $env:BREVI_PY_CACHE, $env:PATH, $env:BREVI_PYTHON
    $env:BREVI_PY_CACHE = $cache; $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"; $env:BREVI_PYTHON = Join-Path $work "no-python\python.exe"
    $ps = 1..3 | ForEach-Object { Start-Process -FilePath $exe -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $work "race$_.out") -RedirectStandardError (Join-Path $work "race$_.err") }
    $env:BREVI_PY_CACHE, $env:PATH, $env:BREVI_PYTHON = $keep
    $ps | ForEach-Object { $_.WaitForExit() }
    foreach ($i in 1..3) {
        $got = (Get-Content (Join-Path $work "race$i.out") -Raw).Replace("`r`n", "`n").Trim()
        if ($ps[$i - 1].ExitCode -ne 0 -or $got -ne $want) { throw "racing first run $i exited $($ps[$i - 1].ExitCode)`n$got`n$(Get-Content (Join-Path $work "race$i.err") -Raw)" }
    }
    Write-Host "PASS standalone first runs racing on an empty cache"

    # Bytes appended after the build hide the bundle: refused, with Python on PATH and nothing in the cache.
    $signed = Join-Path $work "signed.exe"
    Copy-Item $exe $signed
    [System.IO.File]::AppendAllText($signed, "appended after the build")
    $keep = $env:BREVI_PY_CACHE
    $env:BREVI_PY_CACHE = Join-Path $work "cache-signed"
    $out = Lines (& $signed 2>&1)
    $code = $LASTEXITCODE
    $env:BREVI_PY_CACHE = $keep
    if ($code -eq 0 -or $out -notmatch 'built with --standalone, but its bundled Python is missing') { throw "a standalone exe changed after the build was not refused (exit $code)`n$out" }
    Write-Host "PASS standalone exe changed after the build is refused"

    # A cache folder too deep for Windows' 260-character paths: refused with what to do, never a half-unpacked run.
    $deep = Join-Path $work ("d" * 200)
    $r = Run-Bare $exe $deep
    if ($r[0] -eq 0 -or $r[1] -notmatch 'set BREVI_PY_CACHE to a shorter folder') { throw "a cache past 260 characters was not refused (exit $($r[0]))`n$($r[1])" }
    Write-Host "PASS standalone cache past 260 characters refused"
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
Write-Host "all standalone checks passed"
