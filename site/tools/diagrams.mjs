// Draws the Mermaid diagrams in docs/ARCHITECTURE.md into site/docs/img/mmd-HASH.svg, which site/gen.py
// shows on the Architecture page. Run it after changing a diagram (the site build itself needs no Node):
//   npm install --no-save mermaid@11 && node site/tools/diagrams.mjs
// Uses the preinstalled Playwright Chromium. HASH is the first 12 hex digits of the SHA-1 of the diagram source.
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync, readdirSync, unlinkSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
const require = createRequire(import.meta.url);
const { chromium } = require("playwright");
const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const out = join(here, "..", "docs", "img");
const mermaidJs = require.resolve("mermaid/dist/mermaid.min.js", { paths: [process.cwd(), root, ...(process.env.NODE_PATH || "").split(":")] });
const src = readFileSync(join(root, "docs", "ARCHITECTURE.md"), "utf8");
const blocks = [...src.matchAll(/```mermaid\n([\s\S]*?)```/g)].map((m) => m[1].replace(/\n$/, ""));
const browser = await chromium.launch();
const page = await browser.newPage();
await page.setContent("<!doctype html><body></body>");
await page.addScriptTag({ path: mermaidJs });
await page.evaluate(() => window.mermaid.initialize({ startOnLoad: false, theme: "base", fontFamily: "IBM Plex Sans, system-ui, sans-serif",
  themeVariables: { fontSize: "14px", primaryColor: "#f1f2f0", primaryBorderColor: "#8a948f", lineColor: "#5d6462", textColor: "#1b1e1d" },
  htmlLabels: false, flowchart: { htmlLabels: false, curve: "basis" } }));
const keep = new Set();
for (const [i, code] of blocks.entries()) {
  const h = createHash("sha1").update(code.trim()).digest("hex").slice(0, 12);
  keep.add(`mmd-${h}.svg`);
  // Re-serialise as XML: mermaid's string has HTML-style <br> that an <img> (strict XML) refuses.
  const svg = await page.evaluate(async ([id, c]) => {
    const div = document.createElement("div");
    div.innerHTML = (await window.mermaid.render(id, c)).svg;
    return new XMLSerializer().serializeToString(div.firstElementChild);
  }, [`d${i}`, code]);
  writeFileSync(join(out, `mmd-${h}.svg`), svg);
  console.log(`mmd-${h}.svg`);
}
for (const f of readdirSync(out)) if (f.startsWith("mmd-") && !keep.has(f)) unlinkSync(join(out, f));
await browser.close();
