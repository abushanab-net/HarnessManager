// Screenshot a mockup (or any local HTML) at 1280x800 in Edge, in dark and light.
//   node shoot.mjs <file.html> [out-basename] [--full]
// Writes <out>-dark.png and <out>-light.png beside the HTML (or at the given basename). Needs e2e/node_modules (playwright-core).
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';
const here = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(path.resolve(here, '../../../e2e/package.json'));
const { chromium } = require('playwright-core');
const [file, outArg] = process.argv.slice(2).filter((a) => !a.startsWith('--'));
const full = process.argv.includes('--full');
if (!file) { console.error('usage: node shoot.mjs <file.html> [out-basename] [--full]'); process.exit(1); }
const abs = path.resolve(file);
const out = outArg ? path.resolve(outArg) : abs.replace(/\.html$/, '');
const browser = await chromium.launch({ channel: 'msedge', headless: true });
try {
  for (const theme of ['dark', 'light']) {
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1, colorScheme: theme });
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
    await page.goto(pathToFileURL(abs).href + '?theme=' + theme);
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(300);
    // facts a reviewer needs: does anything overflow the frame, and what is the smallest text
    const facts = await page.evaluate(() => {
      const sizes = {}; let over = 0;
      for (const el of document.querySelectorAll('body *')) {
        const r = el.getBoundingClientRect(); if (!r.width || !r.height) continue;
        if ([...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())) { const fs = parseFloat(getComputedStyle(el).fontSize); sizes[fs] = (sizes[fs] || 0) + 1; }
        if (el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflowX === 'visible' && el.children.length === 0) over++;
      }
      return { page: [document.documentElement.scrollWidth, document.documentElement.scrollHeight], textSizes: sizes, textOverflowing: over };
    });
    await page.screenshot({ path: `${out}-${theme}.png`, fullPage: full });
    console.log(theme, JSON.stringify(facts), errors.length ? 'ERRORS: ' + errors.join(' | ') : 'no console errors');
    await page.close();
  }
} finally { await browser.close(); }
