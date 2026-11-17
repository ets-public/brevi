# Security policy

Brevi is in public preview and still 0.x. The latest minor release gets every fix. The previous minor gets fixes for Critical and High security issues only, for 4 weeks after its successor ships. Older releases get nothing. See [docs/SUPPORT.md](docs/SUPPORT.md).

## Where Brevi stands

- **No external security audit yet.** We have not had an external audit; we are seeking funding for one. Until then, do not run hostile code under the sandbox.
- **The sandbox is a policy layer, not an isolation boundary.** `--sandbox` (the default in the `brevi mcp` tools and the SDKs, except their `verify`, which runs CPython) checks at build time what the program may touch, and, without `--allow-py` or `--allow native`, the program reaches files and the network only through Brevi's own code, which checks the caller's grants. There is no namespace, container or VM around the program, and it runs as the same OS user as `brevi`. Only a program that loads CPython gets kernel holds on files and the network as well, on Linux only: Landlock for files (5.13 or later, with Landlock on; where it is missing, named folders are refused and otherwise CPython keeps full file access) and, on x86-64 and arm64, a seccomp filter against the network when none is granted. A `rust fn` or crate under `--allow native` gets no kernel hold at all. The sandbox stops agent code from touching what the caller did not grant; it is not built to hold code that is trying to break out, and a bug in Brevi's checks or runtime can be a way out.
- **Run untrusted code in a container or VM.** Run `brevi` itself inside one, with no credentials in it, and use the sandbox as a second layer inside it.
- **`--allow-py` and `--allow native` are full trust.** Once CPython or native code is loaded, the program can do anything the user running `brevi` can, apart from the kernel holds and limits the threat model lists. Grant them only to code you would run without a sandbox.

The details, and what each layer does and does not cover, are in [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md). How we test it is in [docs/SECURITY_TESTING.md](docs/SECURITY_TESTING.md).

### Who should use Brevi today

| Use | What it needs first |
| --- | --- |
| A design partner running its own trusted agent code inside its existing container or VM | Nothing more: this is a fair use today, with the four points above understood. No audit is needed, because nobody relies on the sandbox as the boundary. |
| A public beta | An independent security review of a fixed scope (the sandbox checks, the MCP server, DLL loading on Windows) is strongly recommended. |
| Offering the sandbox as a boundary for hostile code (not before an audit) | A paid external audit. We will not make that claim before one is done. |

The external audit is on the [roadmap](docs/ROADMAP.md#ws7-safety-at-scale-gates-the-hosted-service); it does not hold up the pilot use above.

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

Every release has a `SHA256SUMS` file. `install.sh` checks the archive against it, and also checks the archive's build-provenance attestation when `gh` is installed and signed in; it installs nothing if either check fails. To check by hand:

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

See [Where Brevi stands](#where-brevi-stands) at the top: no external audit yet, the sandbox is a policy layer and not an isolation boundary, and code you have reason to distrust belongs in a container or VM with no credentials. See [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).
