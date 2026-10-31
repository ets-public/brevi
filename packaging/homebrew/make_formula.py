"""Writes a Homebrew formula for a Brevi release. usage: make_formula.py SHA256SUMS > brevi.rb

SHA256SUMS is the file attached to the GitHub release (release.yml writes it). The version comes from Cargo.toml.
The formula installs the prebuilt binary and depends on rust, since brevi builds programs with rustc.
It goes in a tap repo (for example ets-public/homebrew-brevi as Formula/brevi.rb); nothing is published by this script.
"""
import re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
version = re.search(r'^version = "([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M)[1]
sums = {}
for line in Path(sys.argv[1]).read_text().splitlines():
    if line.strip():
        digest, name = line.split()
        sums[name.lstrip("*")] = digest
url = f"https://github.com/ets-public/brevi/releases/download/v{version}"


def asset(target):
    name = f"brevi-{target}.tar.gz"
    if name not in sums:
        sys.exit(f"{name} is not in {sys.argv[1]}")
    return f'url "{url}/{name}"\n      sha256 "{sums[name]}"'


print(f'''class Brevi < Formula
  desc "Programming language for AI models that compiles to Rust"
  homepage "https://brevi-lang.org"
  version "{version}"
  license "Apache-2.0"

  on_macos do
    on_arm do
      {asset("aarch64-apple-darwin")}
    end
    on_intel do
      {asset("x86_64-apple-darwin")}
    end
  end

  on_linux do
    on_arm do
      {asset("aarch64-unknown-linux-gnu")}
    end
    on_intel do
      {asset("x86_64-unknown-linux-gnu")}
    end
  end

  depends_on "rust"

  def install
    bin.install "brevi"
  end

  test do
    (testpath/"hi.bv").write("n = 2\\nprint(\\"hi {{n}}\\")\\n")
    assert_equal "hi 2", shell_output("#{{bin}}/brevi run hi.bv").strip
  end
end''')
