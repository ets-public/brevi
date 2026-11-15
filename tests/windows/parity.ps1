# Windows parity checks: command-line arguments, text-mode newlines, generators on fibers, the DLL search
# order, TLS against public roots and TLS against the Windows certificate store. Run from the repository root after `cargo build --release` (or set BREVI).
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
function Raw-Out([string]$file, [string[]]$argv, [hashtable]$envset = @{}) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $file
    foreach ($a in $argv) { [void]$psi.ArgumentList.Add($a) }
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.Environment["PYTHONIOENCODING"] = "utf-8"
    # $null removes a variable; the certificate variables are cleared for every run that sets $envset
    foreach ($k in $envset.Keys) { if ($null -eq $envset[$k]) { [void]$psi.Environment.Remove($k) } else { $psi.Environment[$k] = $envset[$k] } }
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

    Run-Check "certificate store tls" {
        # With no CA file, Brevi's requests must trust what CPython's default context trusts on Windows: the
        # "CA" and "ROOT" stores' certificates for server authentication (load_default_certs). Each case runs
        # parity_store_ref.py (CPython's ssl) and parity_store_tls.py (Brevi) and both must print the expected
        # word. The test certificates are made here, self-signed, CA:FALSE, valid for one hour, added to the
        # current user's "CA" store (no prompt, unlike "ROOT") and removed afterwards.
        $exe = Build "parity_store_tls.py"
        $ref = Join-Path $root "tests\windows\parity_store_ref.py"
        $clean = @{ SSL_CERT_FILE = $null; SSL_CERT_DIR = $null; REQUESTS_CA_BUNDLE = $null; CURL_CA_BUNDLE = $null }
        function New-TestCert([string]$dir, [string[]]$eku) {
            New-Item -ItemType Directory $dir -Force | Out-Null
            $rsa = [System.Security.Cryptography.RSA]::Create(2048)
            $req = [System.Security.Cryptography.X509Certificates.CertificateRequest]::new("CN=brevi store test $([guid]::NewGuid())", $rsa, [System.Security.Cryptography.HashAlgorithmName]::SHA256, [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
            $san = [System.Security.Cryptography.X509Certificates.SubjectAlternativeNameBuilder]::new()
            $san.AddDnsName("localhost")
            $req.CertificateExtensions.Add($san.Build())
            $req.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($false, $false, 0, $true))
            if ($eku) {
                $oids = [System.Security.Cryptography.OidCollection]::new()
                foreach ($o in $eku) { [void]$oids.Add([System.Security.Cryptography.Oid]::new($o)) }
                $req.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($oids, $false))
            }
            $now = [DateTimeOffset]::UtcNow
            $c = $req.CreateSelfSigned($now.AddMinutes(-5), $now.AddHours(1))
            Set-Content -Path (Join-Path $dir "cert.pem") -Value $c.ExportCertificatePem() -NoNewline
            Set-Content -Path (Join-Path $dir "key.pem") -Value $rsa.ExportPkcs8PrivateKeyPem() -NoNewline
            return [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($c.RawData)
        }
        function Expect([string]$what, [string]$want, [string[]]$refArgs, [string]$url, [hashtable]$envset) {
            $r = Raw-Out "python" (@($ref) + $refArgs) $envset
            $n = Raw-Out $exe @($url) $envset
            $a = ([System.Text.Encoding]::UTF8.GetString($r.out)).Trim(); $b = ([System.Text.Encoding]::UTF8.GetString($n.out)).Trim()
            $exp = "$want`r`none"
            if ($r.code -ne 0 -or $a -ne $exp) { throw "CPython's default context, $what, printed (exit $($r.code)):`n$a`n$($r.err)`nexpected $want" }
            if ($n.code -ne 0 -or $b -ne $exp) { throw "Brevi, $what, printed (exit $($n.code)):`n$b`n$($n.err)`nCPython: $want" }
            Write-Host "PASS $what`: $want, as CPython's default context"
        }
        $any = New-TestCert (Join-Path $work "store-any") @()
        $client = New-TestCert (Join-Path $work "store-client") @("1.3.6.1.5.5.7.3.2")
        $servers = @()
        $st = [System.Security.Cryptography.X509Certificates.X509Store]::new("CA", "CurrentUser")
        $st.Open("ReadWrite")
        try {
            foreach ($s in @(@{ port = 18190; dir = "store-any" }, @{ port = 18192; dir = "store-client" })) {
                $servers += Start-Process -FilePath "python" -ArgumentList (Join-Path $root "tests\windows\net_server.py"), "$($s.port)", (Join-Path $work $s.dir) -PassThru -WindowStyle Hidden
                $ready = $false
                for ($i = 0; $i -lt 80 -and -not $ready; $i++) {
                    try { $c = New-Object System.Net.Sockets.TcpClient("127.0.0.1", $s.port); $c.Close(); $ready = $true } catch { Start-Sleep -Milliseconds 250 }
                }
                if (-not $ready) { throw "net_server.py did not start on 127.0.0.1:$($s.port)" }
            }
            Expect "self-signed certificate outside the store" "verify-failed" @("localhost", "18191") "https://localhost:18191/text" $clean
            $st.Add($any); $st.Add($client)
            Expect "the same certificate in the CA store" "ok" @("localhost", "18191") "https://localhost:18191/text" $clean
            Expect "a CA-store certificate for client authentication only" "verify-failed" @("localhost", "18193") "https://localhost:18193/text" $clean
            # The ROOT store, with certifi out of the way: SSL_CERT_FILE names a file holding only the test
            # certificate, so a public site verifies only through the Windows ROOT store (for both).
            $only = $clean.Clone(); $only["SSL_CERT_FILE"] = (Join-Path $work "store-any\cert.pem")
            Expect "www.python.org through the ROOT store" "ok" @("www.python.org", "443") "https://www.python.org/" $only
        } finally {
            [void]$st.Remove($any); [void]$st.Remove($client); $st.Close()
            foreach ($p in $servers) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
        }
    }
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
if ($failed.Count -gt 0) { throw "Windows parity checks failed: $($failed -join ', ')" }
Write-Host "all Windows parity checks passed"
