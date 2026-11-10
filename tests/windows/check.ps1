# Windows platform checks: library bridge (typed and dynamic), output parity, and the
# cpu/mem resource limits. Run from the repository root after `cargo build --release`.
$ErrorActionPreference = "Continue"

$root = (Get-Location).Path
$brevi = Join-Path $root "target\release\brevi.exe"
if (-not (Test-Path $brevi)) { throw "brevi.exe not built; run cargo build --release first" }

function Clean-Output($lines) {
    return (($lines | ForEach-Object { "$_" } | Where-Object { $_ -notmatch '^\{"path":' }) -join "`n").Trim()
}

function Check-Verify([string]$name) {
    $file = Join-Path $root "tests\windows\$name"
    $raw = & $brevi verify $file 2>&1
    $code = $LASTEXITCODE
    $out = ($raw | ForEach-Object { "$_" }) -join "`n"
    if ($code -ne 0) { throw "verify $name exited $code`n$out" }
    if ($out -notmatch '"verify":"identical"') { throw "verify $name did not match CPython`n$out" }
    Write-Host "PASS verify $name"
}

function Check-Run([string]$name) {
    $file = Join-Path $root "tests\windows\$name"
    $py = (& python $file 2>&1 | ForEach-Object { "$_" }) -join "`n"
    $raw = & $brevi run $file 2>&1
    $code = $LASTEXITCODE
    $nat = Clean-Output $raw
    if ($code -ne 0) { throw "run $name exited $code`n$nat" }
    if ($py.Trim() -ne $nat) { throw "run $name differs`npython:`n$py`nbrevi:`n$nat" }
    Write-Host "PASS run $name"
}

function Check-Limit([string]$name, [string]$limit, [string]$on) {
    $file = Join-Path $root "tests\windows\$name"
    $raw = & $brevi run --limit $limit $file 2>&1
    $code = $LASTEXITCODE
    $out = ($raw | ForEach-Object { "$_" }) -join "`n"
    if ($code -ne 124) { throw "limit $limit on $name exited $code`n$out" }
    if ($out -notmatch ('"on":"' + $on + '"')) { throw "limit $limit on $name did not name $on`n$out" }
    Write-Host "PASS limit $limit ($on)"
}

Check-Verify "plain.py"
Check-Verify "bridge_yaml.py"
Check-Verify "bridge_yaml_dynamic.py"
Check-Verify "generator.py"
Check-Verify "async.py"
Check-Run "plain.py"
Check-Run "bridge_yaml.py"

$explain = (& $brevi check (Join-Path $root "tests\windows\bridge_yaml_dynamic.py") --explain 2>&1 | ForEach-Object { "$_" }) -join "`n"
if ($explain -notmatch '"path":"dynamic"') { throw "dynamic bridge program did not take the dynamic path`n$explain" }
Write-Host "PASS dynamic path"

Check-Limit "limit_cpu.py" "cpu=1" "cpu"
Check-Limit "limit_mem.py" "mem=64" "mem"
Check-Limit "limit_file.py" "file=1" "file"
Write-Host "all Windows checks passed"
