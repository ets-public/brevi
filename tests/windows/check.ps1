# Windows platform checks: library bridge (typed and dynamic), output parity, and the
# resource limits (cpu/mem in a Job Object; file and fds refused). Run from the repository root after `cargo build --release`.
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
Check-Verify "print_end.py"
Check-Verify "print_csv.py"
Check-Verify "bridge_yaml.py"
Check-Verify "bridge_yaml_dynamic.py"
Check-Verify "generator.py"
Check-Verify "async.py"
Check-Verify "sqlite.py"
Check-Run "plain.py"
Check-Run "bridge_yaml.py"
Check-Run "sqlite.py"

$explain = (& $brevi check (Join-Path $root "tests\windows\bridge_yaml_dynamic.py") --explain 2>&1 | ForEach-Object { "$_" }) -join "`n"
if ($explain -notmatch '"path":"dynamic"') { throw "dynamic bridge program did not take the dynamic path`n$explain" }
Write-Host "PASS dynamic path"

# cpu and mem are held by a Windows Job Object: the program stops at the limit with the limit line and exit 124
Check-Limit "limit_cpu.py" "cpu=1" "cpu"
Check-Limit "limit_cpu_child.py" "cpu=2" "cpu"
Check-Limit "limit_mem.py" "mem=64" "mem"
# under its limits a program runs as usual
$raw = & $brevi run --limit cpu=10,mem=512 (Join-Path $root "tests\windows\plain.py") 2>&1
$out = Clean-Output $raw
if ($LASTEXITCODE -ne 0 -or $out -ne "45") { throw "plain.py under cpu/mem limits exited $LASTEXITCODE`n$out" }
Write-Host "PASS run under cpu/mem limits"
# file and fds have no OS limit on Windows, so they are refused, never half enforced
foreach ($lim in @("file=1", "fds=16")) {
    $raw = & $brevi run --limit $lim (Join-Path $root "tests\windows\plain.py") 2>&1
    $code = $LASTEXITCODE
    $out = ($raw | ForEach-Object { "$_" }) -join "`n"
    if ($code -eq 0 -or $out -notmatch 'not enforced on Windows') { throw "--limit $lim was not refused on Windows (exit $code)`n$out" }
    Write-Host "PASS limit $lim refused"
}
$duck = & python -c "import importlib.util,glob,os,sys; s=importlib.util.find_spec('duckdb'); d=os.path.dirname(list(s.submodule_search_locations)[0]) if s else ''; sys.exit(0 if d and glob.glob(os.path.join(d,'*duckdb*')) else 1)" 2>$null
if ($LASTEXITCODE -eq 0) { Check-Verify "duckdb_native.py"; Check-Run "duckdb_native.py" }
$ort = & python -c "import importlib.util,glob,os,sys; s=importlib.util.find_spec('onnxruntime'); d=os.path.dirname(list(s.submodule_search_locations)[0]) if s else ''; sys.exit(0 if d and glob.glob(os.path.join(d,'capi','onnxruntime*.dll')) else 1)" 2>$null
if ($LASTEXITCODE -eq 0) { Check-Verify "onnx_native.py"; Check-Run "onnx_native.py" }

# Brevi's own requests over native Windows sockets and TLS: net_server.py serves localhost over HTTP and
# HTTPS; net_requests.py must print what CPython's requests prints, built on the dynamic native path.
& python -c "import requests" 2>$null
if ($LASTEXITCODE -eq 0) {
    $srv = Start-Process -FilePath "python" -ArgumentList (Join-Path $root "tests\windows\net_server.py"), "18180", (Join-Path $root "tests\http") -PassThru -WindowStyle Hidden
    try {
        $ready = $false
        for ($i = 0; $i -lt 80 -and -not $ready; $i++) {
            try { $c = New-Object System.Net.Sockets.TcpClient("127.0.0.1", 18180); $c.Close(); $ready = $true } catch { Start-Sleep -Milliseconds 250 }
        }
        if (-not $ready) { throw "net_server.py did not start on 127.0.0.1:18180" }
        $raw = & $brevi verify (Join-Path $root "tests\windows\net_requests.py") -- 18180 (Join-Path $root "tests\http\cert.pem") 2>&1
        $code = $LASTEXITCODE
        $out = ($raw | ForEach-Object { "$_" }) -join "`n"
        if ($code -ne 0 -or $out -notmatch '"verify":"identical"') { throw "verify net_requests.py exited $code`n$out" }
        if ($out -notmatch '"path":"dynamic"' -or $out -match '"py":') { throw "net_requests.py was not built with Brevi's own requests`n$out" }
        Write-Host "PASS verify net_requests.py (native requests, HTTP and HTTPS)"
    } finally {
        Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue
    }
}
$env:BREVI = $brevi
& python (Join-Path $root "tests\windows\mcp_check.py")
if ($LASTEXITCODE -ne 0) { throw "mcp_check.py exited $LASTEXITCODE" }
Write-Host "all Windows checks passed"
