Stdin is a web server access log. Each line looks like
`203.0.113.9 - - [10/Oct/2026:13:55:36 +0000] "GET /index.html HTTP/1.1" 200 2326 "-" "Mozilla/5.0"`:
IP, two dashes, a bracketed timestamp, a quoted request line `METHOD PATH PROTOCOL`, the status code, the response size in bytes (or `-` meaning 0), then a quoted referrer and a quoted user agent. Lines that do not match this shape are malformed: count them and skip them. Query strings (from `?` on) are not part of the path.

Print a report:
1. `requests N` (well-formed lines), `malformed N`, `bytes N` (total size), `unique_ips N`, each on its own line.
2. `status` then one line per status class that appears, in order (`2xx`, `3xx`, `4xx`, `5xx`), as `  2xx N`.
3. `top_paths` then the 3 most requested paths as `  N PATH`, by count descending, then path ascending.
4. `hours` then one line per hour (00-23, only hours with requests, ascending) as `  HH N BAR` where BAR is `#` repeated round(N * 20 / max) times (round half up, at least 1), max being the busiest hour's count.
5. `bots N`: requests whose user agent contains `bot` (any case).
6. `errors_by_ip` then, for IPs with at least 2 responses of status 400 or more, `  IP N`, by N descending then IP ascending (compare IPs as strings).
