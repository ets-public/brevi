Print a line diff of two texts. Stdin holds the old text, then a line that is exactly `=====`, then the new text. Split each text into lines (a final newline does not make an extra empty line; an empty text has no lines).

Use a longest common subsequence of lines. Walk both texts from the start with positions i (old) and j (new):
- if old[i] equals new[j], print `  ` + the line and move both on;
- otherwise, if the LCS length of (old from i+1, new from j) is at least that of (old from i, new from j+1), print `- ` + old[i] and move i on;
- otherwise print `+ ` + new[j] and move j on.
When one text runs out, print the rest of the other with `- ` or `+ `.

Then print `@@ -A +B @@` where A is the number of removed lines and B the number of added lines. If the texts are identical, print only `no changes`.
