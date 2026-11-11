# Windows parity checks: command-line arguments, text-mode newlines, generators on fibers, the DLL search
# order and TLS against public roots. Run from the repository root after `cargo build --release` (or set BREVI).
# Every check compares with CPython or fails; none is skipped.
$ErrorActionPreference = "Continue"

$root = (Get-Location).Path
$brevi = if ($env:BREVI) { $env:BREVI } else { Join-Path $root "target\release\brevi.exe" }
if (-not (Test-Path $brevi)) { throw "$brevi not found; run cargo build --release first (or set BREVI)" }
$brevi = (Resolve-Path $brevi).Path
$work = Join-Path ([System.IO.Path]::GetTempPath()) ("brevi-parity-" + [guid]::NewGuid())
New-Item -ItemType Directory $work | Out-Null

function Build([string]$name) {
    $exe = Join-Path $work ([System.IO.Path]::GetFileNameWithoutExtension($name) + ".exe")
    $raw = & $brevi build (Join-Path $root "tests\windows\$name") -o $exe 2>&1
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path $exe)) { throw "build $name exited $LASTEXITCODE`n$(($raw | ForEach-Object { "$_" }) -join "`n")" }
    return $exe
}

# The raw bytes a process writes to stdout (no PowerShell decoding, so CRLF and UTF-8 stay as written).
function Raw-Out([string]$file, [string[]]$argv) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $file
    foreach ($a in $argv) { [void]$psi.ArgumentList.Add($a) }
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.Environment["PYTHONIOENCODING"] = "utf-8"
    $p = [System.Diagnostics.Process]::Start($psi)
    $ms = New-Object System.IO.MemoryStream
    $err = $p.StandardError.ReadToEndAsync()
    $p.StandardOutput.BaseStream.CopyTo($ms)
    $p.WaitForExit()
    return @{ code = $p.ExitCode; out = $ms.ToArray(); err = $err.Result }
}

function Check-Same([string]$name, [string[]]$argv) {
    $exe = Build $name
    $py = Raw-Out "python" (@((Join-Path $root "tests\windows\$name")) + $argv)
    $nat = Raw-Out $exe $argv
    if ($py.code -ne 0) { throw "python $name exited $($py.code)`n$($py.err)" }
    if ($nat.code -ne 0) { throw "$name exited $($nat.code)`n$($nat.err)" }
    $a = [System.Text.Encoding]::UTF8.GetString($py.out); $b = [System.Text.Encoding]::UTF8.GetString($nat.out)
    if ($a -ne $b) { throw "$name differs (bytes)`npython:`n$($a -replace "`r", '\r')`nbrevi:`n$($b -replace "`r", '\r')" }
    Write-Host "PASS $name (same bytes as CPython)"
    return $exe
}

# Every check runs and reports; the script fails at the end if any failed.
$failed = @()
function Run-Check([string]$name, [scriptblock]$check) {
    try { & $check } catch { $script:failed += $name; Write-Host "FAIL $name`n$_" }
}

try {
    # Quotes, backslashes before quotes, an empty argument, spaces and non-ASCII: the C runtime's splitting rules.
    Run-Check "argv" { Check-Same "parity_argv.py" @("plain", "with space", 'say "hi"', 'trail\', 'back\\"slash', "", "é ü 中", "--flag=1") | Out-Null }
    # print and text-mode files write CRLF; reads translate; newline="" and newline="\n" keep bytes
    Run-Check "newlines" { Check-Same "parity_newlines.py" @() | Out-Null }
    # 5000 live generators, deep recursion inside one, yield from
    Run-Check "generators" { Check-Same "parity_generators.py" @() | Out-Null }
    # os.name, os.sep, sys.platform and os.path on Windows paths
    Run-Check "paths" { Check-Same "parity_paths.py" @() | Out-Null }

    Run-Check "dll search order" {
        # DLL search order: a planted DLL in the program's folder must never be loaded in place of the real
        # CPython or SQLite library. Each planted file is not a valid DLL, so loading it would fail the program.
        $sql = Check-Same "sqlite.py" @()
        $bridge = Check-Same "bridge_yaml.py" @()
        $planted = @("python3.dll", "python313.dll", "python312.dll", "python311.dll", "sqlite3.dll", "libsqlite3.dll", "_sqlite3.pyd", "yaml.pyd")
        foreach ($d in $planted) { Set-Content -Path (Join-Path $work $d) -Value "not a library" }
        foreach ($exe in @($sql, $bridge)) {
            $name = [System.IO.Path]::GetFileNameWithoutExtension($exe) + ".py"
            $py = Raw-Out "python" @((Join-Path $root "tests\windows\$name"))
            $nat = Raw-Out $exe @()
            $a = [System.Text.Encoding]::UTF8.GetString($py.out); $b = [System.Text.Encoding]::UTF8.GetString($nat.out)
            if ($nat.code -ne 0 -or $a -ne $b) { throw "$name with planted DLLs beside it exited $($nat.code)`n$($nat.err)`n$b" }
            Write-Host "PASS $name ignores DLLs planted in its folder"
        }
        # Also from the program's folder as the working directory (the current directory is not searched either).
        Push-Location $work
        try {
            $nat = Raw-Out $sql @()
            if ($nat.code -ne 0) { throw "sqlite.py run from the planted folder exited $($nat.code)`n$($nat.err)" }
            Write-Host "PASS sqlite.py ignores DLLs planted in the working directory"
        } finally { Pop-Location }
    }

    Run-Check "public tls" {
        # TLS against public roots with no CA file (the custom-CA case is net_requests.py in check.ps1).
        $raw = & $brevi verify (Join-Path $root "tests\windows\parity_public_tls.py") 2>&1
        $code = $LASTEXITCODE
        $out = ($raw | ForEach-Object { "$_" }) -join "`n"
        if ($code -ne 0 -or $out -notmatch '"verify":"identical"') { throw "verify parity_public_tls.py exited $code`n$out" }
        if ($out -notmatch '"path":"dynamic"' -or $out -match '"py":') { throw "parity_public_tls.py was not built with Brevi's own requests`n$out" }
        Write-Host "PASS verify parity_public_tls.py (public roots)"
    }
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
if ($failed.Count -gt 0) { throw "Windows parity checks failed: $($failed -join ', ')" }
Write-Host "all Windows parity checks passed"
