// Renders site/icons/favicon.svg to the PNG and ICO sizes browsers and phones ask for.
// Run with: node site/icons/make.js   (needs Playwright with Chromium, and Python Pillow for the .ico)
const { chromium } = require('playwright');
const fs = require('fs'), path = require('path'), { execFileSync } = require('child_process');
const dir = __dirname, svg = fs.readFileSync(path.join(dir, 'favicon.svg'), 'utf8');
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage();
  const sizes = { 'apple-touch-icon.png': 180, 'icon-192.png': 192, 'icon-512.png': 512, 'ico-16.png': 16, 'ico-32.png': 32, 'ico-48.png': 48 };
  for (const [name, s] of Object.entries(sizes)) {
    // The apple icon is square with no transparent corners; iOS rounds it itself.
    const src = name.startsWith('apple') ? svg.replace(/rx="13"/, 'rx="0"') : svg;
    await p.setViewportSize({ width: s, height: s });
    await p.setContent(`<style>html,body{margin:0;background:transparent}svg{display:block;width:${s}px;height:${s}px}</style>${src}`);
    await p.screenshot({ path: path.join(dir, name), omitBackground: true });
  }
  await b.close();
  execFileSync('python3', ['-c', `
from PIL import Image; import os; d=${JSON.stringify(dir)}
ims=[Image.open(os.path.join(d,f'ico-{s}.png')) for s in (16,32,48)]
ims[2].save(os.path.join(d,'favicon.ico'), sizes=[(16,16),(32,32),(48,48)], append_images=ims[:2])
for s in (16,32,48): os.remove(os.path.join(d,f'ico-{s}.png'))`]);
})();
