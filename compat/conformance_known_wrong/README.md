# Known wrong results and crashes (`tools/conformance.py`)

One file per entry, named `test-id.mode` (`mode` is `normal` or `dyn`) and holding one line,
`issue=#N owner=LOGIN expires=YYYY-MM-DD`. One file per entry means two pull requests that add or remove
different entries never touch the same file, so they never conflict.

`--check` fails on a wrong result with no file here, on a file whose test is no longer wrong (delete it: the
folder only shrinks), and on a file without a tracking issue, an owner and an expiry date, or past its expiry.
`--write-baseline` keeps the metadata of entries already here and writes new ones with `?`, which `--check`
refuses until they are filled in.
