Convert a small subset of Markdown on stdin to HTML on stdout.

Blocks (separated by blank lines or by a change of block type):
- `# Title` to `###### Title`: a heading `<h1>`..`<h6>`. Its id is the text lowercased, with every run of characters other than a-z and 0-9 replaced by one `-`, and leading or trailing `-` removed. Output `<h2 id="the-id">Text</h2>`. If an id was already used, add `-2`, `-3`, and so on.
- Lines starting with `- `: items of one `<ul>`. Lines starting with `1. ` (any number then `. `): items of one `<ol>`. Each item is `<li>...</li>` on its own line, between `<ul>`/`</ul>` lines.
- A line of exactly three backticks starts a code block, which ends at the next such line. Output `<pre><code>`, the lines in between (HTML-escaped, nothing else changed) joined by newlines, then `</code></pre>`, all as written: `<pre><code>` directly followed by the first line, and `</code></pre>` directly after the last line.
- Any other consecutive non-blank lines form a paragraph: `<p>` + the lines joined with a single space + `</p>`.

Inline (in headings, items and paragraphs): first HTML-escape `&`, `<`, `>` (as `&amp;` `&lt;` `&gt;`), then `` `code` `` becomes `<code>code</code>` (its content is not further formatted), `**bold**` becomes `<strong>bold</strong>`, `*em*` becomes `<em>em</em>`, and `[text](url)` becomes `<a href="url">text</a>`.

Finally, if there are at least two headings, print a table of contents before everything else: `<nav>` on a line, then for each heading a line `<a href="#id">Text</a>` (Text with inline formatting removed, that is the raw heading text with the markers `` ` `` and `*` deleted, HTML-escaped), then `</nav>`. Each block is printed on its own line(s), with nothing between blocks.
