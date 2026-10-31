# Brevi

Brevi is a programming language for models to write. It reads like Python, compiles to Rust, and reports every error as one short JSON line with a suggested fix.

## Setup

Check that `brevi --version` works. If it does not, run the repository's `install.sh`: it fetches the Rust toolchain if `rustc` is missing (Brevi compiles through it), then a prebuilt `brevi` from the latest release, or builds it with `cargo install --git https://github.com/ets-public/brevi`.

## Before writing code

Run `brevi guide` once and read the whole guide it prints. It is short and matches the installed compiler. Rare details are not in it: `brevi guide WORD` prints the lines about one name (`brevi guide partition`) and `brevi guide Strings` the full brace and format rules.

## Loop

1. Write the whole program to `NAME.bv`. Programs read stdin and write stdout.
2. Run `brevi run NAME.bv < input.txt`, or `brevi check NAME.bv` to compile without running, or `brevi test NAME.bv` for `test` blocks.
3. Each error is `{"line":N,"msg":...,"fix":...}`. Change only the lines it names, then run again. Do not rewrite the whole file.
4. `brevi fix NAME.bv` writes every suggested rename into the file. Adding `--explain` to any command adds the guide lines for each error.
5. To replace a whole function, write just the new `fn` (or `type`, `enum`, `test`) to a patch file and run `brevi patch NAME.bv patch.bv`.

## Notes

- `brevi check NAME.bv --fast` runs only Brevi's own checks, in milliseconds, without rustc.
- `brevi build NAME.bv -o app --release` makes a native binary.
- When Brevi has no built-in for something (hashing, HTTP, a data science library), run `brevi guide Libraries`: write that one function as a `rust fn` (with `use CRATE "VERSION"`) or a `py fn`, and keep the rest in Brevi. Split big programs into files with `use "other.bv"`.
- Python habits are accepted (`True`, `.append()`, `range()`, `", ".join(xs)`), so write what comes naturally and let the errors guide you.
