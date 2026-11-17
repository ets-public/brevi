# OSS-Fuzz application (draft)

Draft files for applying Brevi to [OSS-Fuzz](https://google.github.io/oss-fuzz/getting-started/accepting-new-projects/), which runs fuzz targets continuously on Google's machines for free and reports crashes privately. Nothing here runs in Brevi's own CI.

To apply, a maintainer opens a pull request on google/oss-fuzz adding `projects/brevi/` with these three files, and a `primary_contact` that can read private bug reports. OSS-Fuzz accepts projects that are widely used or critical; Brevi is new, so the application may be declined until it has more users. If it is, [ClusterFuzzLite](https://google.github.io/clusterfuzzlite/) runs the same `build.sh` and Dockerfile in GitHub Actions, but costs Actions minutes, so it stays off until there is budget for it.

Before applying: `cargo +nightly fuzz build -O` passes (tools/security_checks.sh), and docs/SECURITY_TESTING.md lists every target.
