# Known wrong results on Windows (`tools/conformance.py --slice stdlib`)

The Windows twin of `compat/conformance_stdlib_known_wrong/`, with the same rules: one file per entry, named
`test-id.mode` and holding `issue=#N owner=LOGIN expires=YYYY-MM-DD`. On Windows, `tools/conformance.py --slice
stdlib --check` reads this folder and `compat/conformance_windows_stdlib_floor.json` instead of the Linux ones, so a
result that is right on Linux but wrong on Windows is caught here and not hidden by the Linux ratchet.

A release runs `--slice stdlib --check --release-gate` on the `brevi.exe` it ships (`.github/workflows/release.yml`),
so any file here blocks a release: the release gate allows no known-wrong result. The weekly `conformance` job in
`.github/workflows/windows.yml` runs `--slice stdlib --check --raise-floor` on main and commits a raised Windows floor.
