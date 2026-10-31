Stdin has one request per line: `OP TEXT`, where OP is `sha256`, `b64` or `unb64`, then one space, then TEXT (the rest of the line, which may contain spaces or be empty).
- `sha256`: print the SHA-256 digest of TEXT's UTF-8 bytes in lowercase hex.
- `b64`: print TEXT's UTF-8 bytes in standard Base64 with `=` padding.
- `unb64`: decode TEXT as standard Base64 and print the result as UTF-8 text; if TEXT is not valid Base64 or the bytes are not valid UTF-8, print `error`.
Skip blank lines. For any other OP print `unknown OP`.
At the end print `lines N` where N is the number of requests handled (not counting blank lines).
