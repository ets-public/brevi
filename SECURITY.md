# Security policy

Brevi is in public preview. Fixes go into the latest release only.

## Reporting a vulnerability

Report it privately through GitHub: on the repository's **Security** tab, choose **Report a vulnerability**. Please do not open a public issue. If you cannot use GitHub, email support@brevi-lang.org with "security" in the subject.

Include what you ran, what happened, and the Brevi version (`brevi --version`). You should get a first reply within a week.

## What counts

Brevi compiles programs into native binaries, so the most important cases are:

- a program declared with `allow` lines, or run with `brevi run --sandbox` or through the `brevi mcp` tools (sandboxed by default), that reads or writes files, reaches the network or the environment without declaring it;
- generated Rust that is memory-unsafe for a program Brevi accepted;
- `install.sh` or a release download fetching or running something it should not.

A wrong answer that is not a sandbox escape is a normal bug: please open an issue.
