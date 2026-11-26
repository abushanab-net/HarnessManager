// node tools/page.mjs out.html "Title" body.html : wraps a body fragment in the shared head and the shared scripts
import fs from 'fs';
const [out, title, body] = process.argv.slice(2);
const head = fs.readFileSync('tools/head.html', 'utf8').replace(/<title>.*<\/title>/, `<title>${title} · Calibre · Harness Manager mobile</title>`);
const scripts = ['calibre.js', 'screens-onboarding.js', 'screens-requests.js', 'screens-sessions.js', 'screens-accounts.js', 'screens-you.js', 'screens-states.js', 'screens-extra.js', 'screens-platforms.js', 'widgets.js', 'prototype.js']
  .filter(f => fs.existsSync(f)).map(f => `<script src="${f}"></script>`).join('\n');
fs.writeFileSync(out, `${head}<body>\n<header data-topbar></header>\n<main class="page">\n${fs.readFileSync(body, 'utf8')}\n</main>\n${scripts}\n</body>\n</html>\n`);
console.log('wrote', out);
