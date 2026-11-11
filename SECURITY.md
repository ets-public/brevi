# Security policy

Brevi is in public preview and still 0.x. The latest minor release gets every fix. The previous minor gets fixes for Critical and High security issues only, for 4 weeks after its successor ships. Older releases get nothing. See [docs/SUPPORT.md](docs/SUPPORT.md).

## Reporting a vulnerability

Report it privately. Never open a public issue for a vulnerability.

- On the repository's **Security** tab, choose **Report a vulnerability** (GitHub private reporting), or
- email support@brevi-lang.org with "security" in the subject.

Include what you ran, what happened, and the Brevi version (`brevi --version`).

## What you can expect

| Step | Target |
| --- | --- |
| First reply | within 3 days |
| Severity and a fix plan | within 7 days |
| Fix, Critical | 7 days |
| Fix, High | 30 days |
| Fix, Medium | 90 days |
| Fix, Low | best effort |

Severity is Critical, High, Medium or Low. We use CVSS v4 as a guide, not a formula.

Critical and High fixes ship as a patch release as soon as they are ready, even outside the usual Monday release. Medium and Low fixes ship in the next weekly release.

Disclosure is coordinated. We publish the advisory when the fixed release ships, and at most 90 days after your report, unless you agree to a different date. We request CVEs through GitHub Security Advisories (GitHub is a CNA). We credit reporters by GitHub handle unless they ask us not to.

## What counts

Brevi compiles programs into native binaries, so the most important cases are:

- a program declared with `allow` lines, or run with `brevi run --sandbox` or through the `brevi mcp` tools (sandboxed by default), that reads or writes files, reaches the network or the environment without being granted it;
- an MCP call that widens its own access: turns the sandbox off or grants itself more than the person who started `brevi mcp` allowed (without `--trust-calls`);
- a `--limit` (MCP `limits`) that does not hold where the [threat model](docs/THREAT_MODEL.md) says it does;
- generated Rust or runtime code that is memory-unsafe for a program Brevi accepted;
- a request that gets past the token or origin checks of `brevi mcp --http`;
- `install.sh` or a release download fetching or running something it should not, or a release artifact that does not match its checksum.

A wrong answer that is not a sandbox escape is a normal bug: please open an issue ([CONTRIBUTING.md](CONTRIBUTING.md)). What the sandbox does and does not protect against is in [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md). Known limits that are documented there are not vulnerabilities.

## Verifying a download

Every release has a `SHA256SUMS` file. `install.sh` checks the archive against it and installs nothing if the check fails. To check by hand:

```
sha256sum -c SHA256SUMS --ignore-missing
```

From v0.28.0, each release also carries:

- a CycloneDX SBOM, `brevi-sbom.cdx.json`;
- GitHub build-provenance attestations for every archive. Verify one with `gh attestation verify FILE -R ets-public/brevi`;
- a reproducible-build check: the Linux x86-64 binary is rebuilt in a second folder and must be byte-identical.

Earlier releases have only `SHA256SUMS`.

## If a release or credential is compromised

This covers a vulnerability being exploited, or a compromised release artifact, registry package, workflow or credential. The steps:

1. **Contain.** Revoke the credential. Delete the affected release assets. Yank the version on PyPI and crates.io, deprecate it on npm, and delete the ghcr.io image tag.
2. **Fix and ship** a patch release.
3. **Publish an advisory** that says which versions and artifacts were affected and what users should do.
4. **Write a short post-mortem** in the advisory.

The target is containment within 24 hours of confirmation.

## Scope notes

No outside security review has been done yet. The sandbox is a compile-time capability check plus a runtime with nothing in it, not an operating-system jail. For code you have reason to distrust, run `brevi` itself in a container or VM with no credentials. See [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).
