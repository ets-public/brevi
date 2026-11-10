// Renders the link-preview card (Open Graph / Twitter) to site/icons/social.png and social-run.png, 1200x630.
// Run with: node site/icons/social.js   (needs Playwright with Chromium)
const { chromium } = require('playwright');
const fs = require('fs'), path = require('path');
const dir = __dirname, fonts = path.join(dir, '..', 'fonts');
const svg = fs.readFileSync(path.join(dir, 'favicon.svg'), 'utf8').replace('<svg', '<svg width="96" height="96"');
const font = (f) => 'data:font/woff2;base64,' + fs.readFileSync(path.join(fonts, f)).toString('base64');
const card = (name, line, sub) => `<style>
@font-face{font-family:Sans;src:url(${font('plex-sans.woff2')})}
@font-face{font-family:Mono;src:url(${font('plex-mono-400.woff2')})}
html,body{margin:0}
.c{width:1200px;height:630px;box-sizing:border-box;padding:72px 84px;background:#0b100e;color:#e8ecea;display:flex;flex-direction:column;justify-content:space-between;font-family:Sans;border-bottom:18px solid #1d5b4f}
.top{display:flex;align-items:center;gap:24px;font:500 44px Sans;letter-spacing:-0.01em}
h1{font:500 70px/1.08 Sans;margin:0;max-width:980px;letter-spacing:-0.03em}
.sub{font:400 30px/1.4 Sans;color:#a9b2af;max-width:960px;margin-top:22px}
.foot{font:400 26px Mono;color:#7cc4b1}
</style><div class="c"><div class="top">${svg}${name}</div><div><h1>${line}</h1><div class="sub">${sub}</div></div><div class="foot">${name === 'Brevi' ? 'brevi-lang.org' : 'brevi.run'}</div></div>`;
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1200, height: 630 } });
  for (const [file, html] of [
    ['social.png', card('Brevi', 'Run the Python your agent writes as a native binary', 'Plain Python in, a fast, memory-safe Rust binary out, with the same output as CPython. Free and open source.')],
    ['social-run.png', card('brevi.run', 'Hosted Brevi for AI agents', 'Compile, run and test Python as native binaries, with no toolchain on the agent’s machine. Coming soon.')],
  ]) {
    await p.setContent(html);
    await p.evaluate(async () => { await Promise.all(['500 70px Sans', '400 30px Sans', '400 26px Mono'].map((f) => document.fonts.load(f))); await document.fonts.ready; });
    await p.screenshot({ path: path.join(dir, file) });
  }
  await b.close();
})();
