# Brevi dev container feature

Adds Brevi to any dev container (VS Code, Codespaces, the devcontainer CLI): the release binary, checked against the release's SHA256SUMS, in `/usr/local/bin`, `brevi-python` beside it, and the minimal Rust toolchain in `/usr/local/cargo` if the image has no `rustc`. Debian, Ubuntu and Fedora based images work (glibc; not Alpine); it installs `curl` and a C compiler if they are missing.

```json
{
  "image": "mcr.microsoft.com/devcontainers/python:3.13",
  "features": {
    "ghcr.io/ets-public/features/brevi:1": { "version": "latest" }
  }
}
```

`version` is a Brevi release such as `0.27.0`, or `latest` (the default). Each release publishes the feature to `ghcr.io/ets-public/features/brevi` (release.yml), from v0.27.0. The feature's source is [src/brevi](src/brevi).
