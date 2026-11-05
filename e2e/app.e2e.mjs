// End-to-end tests of the Harness Manager desktop app: the real window, driven by Playwright (playwright-core,
// `npm install` in e2e/) over the WebView2 DevTools protocol, in a sandbox with fake tools, fixture sessions
// and a private credential prefix.
//
//   node e2e/app.e2e.mjs            (after `cargo build --workspace`)
//
// Screenshots of every screen land in e2e/.work/shots for review.

import { spawn, execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import zlib from 'node:zlib';
import http from 'node:http';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const target = path.join(root, 'target', 'debug');
// HRM_E2E_APP_DIR=<install folder> drives the installed app and its bundled hrm.exe instead of the debug build
const appDir = process.env.HRM_E2E_APP_DIR || target;
const work = path.join(root, 'e2e', '.work');
const shots = path.join(work, 'shots');
fs.rmSync(work, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });

// ---------- sandbox ----------
const sb = fs.mkdtempSync(path.join(os.tmpdir(), 'hrm-e2e-'));
const home = path.join(sb, 'home'), data = path.join(sb, 'data'), bin = path.join(sb, 'bin');
for (const d of [home, data, bin]) fs.mkdirSync(d, { recursive: true });
for (const t of ['claude', 'codex', 'opencode', 'agy', 'pi', 'omp']) fs.copyFileSync(path.join(target, 'fakeharness.exe'), path.join(bin, `${t}.exe`));
const prefix = `HRMTEST-app-${process.pid}`;
const runLog = path.join(sb, 'runs.jsonl'), termLog = path.join(sb, 'terminal.jsonl');
const pricing = path.join(sb, 'pricing.json');
fs.writeFileSync(pricing, JSON.stringify({ anthropic: { models: { 'claude-opus-5-5': { cost: { input: 4, output: 20, cache_read: 0.2, cache_write: 5 } } } }, openai: { models: { 'gpt-6': { cost: { input: 1, output: 10, cache_read: 0.1 } } } } }));
const write = (f, s) => { fs.mkdirSync(path.dirname(f), { recursive: true }); fs.writeFileSync(f, s); };
// the design checks are of the light theme, whatever Windows is set to
write(path.join(data, 'settings.json'), JSON.stringify({ theme: 'light' }));
// GitHub's archive downloads (Discover, installs from a link) answer with this sample repository
const repoDir = path.join(sb, 'repo-src', 'demo-skills-main');
write(path.join(repoDir, 'skills', 'hello', 'SKILL.md'), '---\nname: hello\ndescription: Says hello in every language\n---\nHello.');
write(path.join(repoDir, 'skills', 'review', 'SKILL.md'), '---\nname: review\ndescription: >\n  Reviews a change\n  line by line\n---\nReview.');
write(path.join(repoDir, 'README.md'), 'not a skill');
const repoZip = path.join(sb, 'repo.zip');
execFileSync('pwsh', ['-NoProfile', '-Command', `Compress-Archive -Path '${repoDir}' -DestinationPath '${repoZip}' -Force`]);
const codeloadHits = [];
let latestVersion = '9.9.9';
const codeload = http.createServer((req, res) => {
  // a newer Harness Manager release, for the update check
  if (req.url === '/latest.json') { res.writeHead(200, { 'content-type': 'application/json' }); return res.end(JSON.stringify({ version: latestVersion, notes: 'Faster limits.', pub_date: '2026-11-01T00:00:00Z', platforms: { 'windows-x86_64': { signature: 'x', url: 'https://example.invalid/setup.exe' } } })); }
  codeloadHits.push(req.url); res.writeHead(200, { 'content-type': 'application/zip' }); res.end(fs.readFileSync(repoZip)); });
await new Promise((r) => codeload.listen(0, '127.0.0.1', r));
const env = {
  ...process.env,
  HRM_GITHUB_CODELOAD: `http://127.0.0.1:${codeload.address().port}`,
  // the debug build accepts a plain-http release; the installed (release) build does not, so it skips that test
  ...(process.env.HRM_E2E_APP_DIR ? {} : { HRM_UPDATE_URL: `http://127.0.0.1:${codeload.address().port}/latest.json` }),
  // Launch at sign-in writes here, never to the real Run key
  HRM_AUTOSTART_KEY: `Software\\HarnessManagerTest-${process.pid}\\Run`,
  HRM_HOME: data, HRM_PROFILES_HOME: home, HRM_CRED_PREFIX: prefix, HRM_AGY_CRED: `${prefix}-agylive`,
  HRM_TERMINAL_LOG: termLog, HRM_PRICING_FILE: pricing, FAKE_RUN_LOG: runLog, FAKE_WAIT_CODE: '1', FAKE_EMAIL: 'alex@example.com',
  ...(process.env.HRM_E2E_APP_DIR ? {} : { HRM_EXE: path.join(target, 'hrm.exe') }),
  HRM_BIN_CLAUDE: path.join(bin, 'claude.exe'), HRM_BIN_CODEX: path.join(bin, 'codex.exe'), HRM_BIN_OPENCODE: path.join(bin, 'opencode.exe'), HRM_BIN_AGY: path.join(bin, 'agy.exe'), HRM_BIN_PI: path.join(bin, 'pi.exe'), HRM_BIN_OMP: path.join(bin, 'omp.exe'),
};
delete env.CLAUDE_CONFIG_DIR; delete env.ANTHROPIC_API_KEY; delete env.CODEX_HOME;
const hrm = (...args) => execFileSync(path.join(appDir, 'hrm.exe'), args, { env, encoding: 'utf8' });
const readLines = (f) => (fs.existsSync(f) ? fs.readFileSync(f, 'utf8').trim().split('\n').filter(Boolean).map((l) => JSON.parse(l)) : []);

// fixtures: a main Claude profile with a session in a git repository, a main Codex profile with a rollout
const now = new Date().toISOString();
const repo = path.join(home, 'Projects', 'platform-gitops');
write(path.join(repo, '.git', 'config'), '[remote "origin"]\n\turl = git@github.com:example/platform-gitops.git\n');
fs.mkdirSync(path.join(repo, 'apps'), { recursive: true });
write(path.join(home, '.claude', 'CLAUDE.md'), '# main instructions');
write(path.join(home, '.claude', 'skills', 'pdf', 'SKILL.md'), 'pdf');
const jl = (a) => a.map((x) => JSON.stringify(x)).join('\n') + '\n';
write(path.join(home, '.claude', 'projects', 'p', 's-main-1.jsonl'), jl([
  { type: 'user', sessionId: 's-main-1', cwd: repo, timestamp: now, message: { role: 'user', content: 'Add a Longhorn backup target for staging' } },
  { type: 'assistant', sessionId: 's-main-1', cwd: repo, timestamp: now, message: { id: 'm1', model: 'claude-opus-5-5', usage: { input_tokens: 1200, output_tokens: 300 }, content: [{ type: 'thinking', thinking: '' }, { type: 'tool_use', id: 't1', name: 'Edit', input: { file_path: 'apps/longhorn/values-staging.yaml', old_string: 'a', new_string: 'b' } }] } },
  { type: 'user', sessionId: 's-main-1', cwd: repo, timestamp: now, message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: 't1', content: 'ok' }] }, toolUseResult: { filePath: 'apps/longhorn/values-staging.yaml', structuredPatch: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 2, lines: ['-backupTarget: ""', '+backupTarget: s3://longhorn-backups@us-east-1/', '+backupTargetCredentialSecret: minio'] }] } },
  { type: 'assistant', sessionId: 's-main-1', cwd: repo, timestamp: now, message: { id: 'm2', model: 'claude-opus-5-5', usage: { input_tokens: 10, output_tokens: 5 }, content: [{ type: 'tool_use', id: 't2', name: 'Bash', input: { command: 'argocd app sync longhorn-staging' } }] } },
  { type: 'user', sessionId: 's-main-1', cwd: repo, timestamp: now, message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: 't2', is_error: true, content: 'rpc error: code = Unavailable desc = connection closed' }] } },
  { type: 'assistant', sessionId: 's-main-1', cwd: repo, timestamp: now, message: { id: 'm3', model: 'claude-opus-5-5', usage: { input_tokens: 1, output_tokens: 40 }, content: [{ type: 'text', text: 'The staging backup target now points at the `longhorn-backups` bucket.\n\n## What changed\n\n- `values-staging.yaml`: new backup target\n- ArgoCD synced after a retry' }] } },
  { type: 'ai-title', aiTitle: 'Longhorn backup target for staging', sessionId: 's-main-1' },
]));
// a server Claude Code already has: in the list from launch, without an Import click
write(path.join(home, '.claude.json'), JSON.stringify({ mcpServers: { pre: { type: 'stdio', command: 'mcp-pre' } } }));
const rollout = jl([
  { type: 'session_meta', timestamp: now, payload: { id: 'cx-1', cwd: path.join(repo, 'apps') } },
  { type: 'turn_context', timestamp: now, payload: { model: 'gpt-6' } },
  { type: 'event_msg', timestamp: now, payload: { type: 'user_message', message: 'Bump ArgoCD and fix the sync waves' } },
  { type: 'response_item', timestamp: now, payload: { type: 'message', role: 'assistant', content: [{ type: 'output_text', text: 'Bumped.' }] } },
  { type: 'token_usage_record', timestamp: now, payload: { response_id: 'r1', usage: { input_tokens: 900, cached_input_tokens: 400, output_tokens: 80 } } },
  { type: 'event_msg', timestamp: now, payload: { type: 'token_count', info: { total_token_usage: { total_tokens: 980 } }, rate_limits: { primary: { used_percent: 21, window_minutes: 10080, resets_at: Math.floor(Date.now() / 1000) + 3 * 86400 } } } },
]);
write(path.join(home, '.codex', 'sessions', '2026', '11', '01', 'rollout-2026-11-01T10-00-00-cx-1.jsonl.zst'), zlib.zstdCompressSync(Buffer.from(rollout)));
const b64 = (o) => Buffer.from(JSON.stringify(o)).toString('base64url');
write(path.join(home, '.codex', 'auth.json'), JSON.stringify({ auth_mode: 'chatgpt', tokens: { id_token: `${b64({ alg: 'none' })}.${b64({ email: 'alex@example.org', 'https://api.openai.com/auth': { chatgpt_plan_type: 'pro' } })}.sig` } }));
// OpenCode: its SQLite database with one session
{
  const { DatabaseSync } = await import('node:sqlite');
  fs.mkdirSync(path.join(home, '.config', 'opencode'), { recursive: true });
  const ocDir = path.join(home, '.local', 'share', 'opencode');
  fs.mkdirSync(ocDir, { recursive: true });
  const db = new DatabaseSync(path.join(ocDir, 'opencode.db'));
  db.exec(`create table session(id text, parent_id text, title text, directory text, time_created int, time_updated int, time_archived int, tokens_input int, tokens_output int, model text);
    create table message(id text, session_id text, time_created int, time_updated int, data text);
    create table part(id text, message_id text, session_id text, time_created int, time_updated int, data text);`);
  const t = Date.now();
  db.prepare('insert into session values(?,?,?,?,?,?,?,?,?,?)').run('ses_oc1', null, 'Fix the flaky build', repo, t, t, null, 300, 40, '');
  db.prepare('insert into message values(?,?,?,?,?)').run('m_u', 'ses_oc1', t, t, JSON.stringify({ role: 'user' }));
  db.prepare('insert into message values(?,?,?,?,?)').run('m_a', 'ses_oc1', t + 1, t + 1, JSON.stringify({ role: 'assistant', modelID: 'glm-5', time: { created: t, completed: t + 5 }, tokens: { input: 300, output: 40, reasoning: 0, cache: { read: 0, write: 0 } }, path: { cwd: repo } }));
  db.prepare('insert into part values(?,?,?,?,?,?)').run('p1', 'm_u', 'ses_oc1', t, t, JSON.stringify({ type: 'text', text: 'Fix the flaky build' }));
  db.prepare('insert into part values(?,?,?,?,?,?)').run('p2', 'm_a', 'ses_oc1', t + 1, t + 1, JSON.stringify({ type: 'tool', tool: 'bash', state: { status: 'completed', input: { command: 'cargo test --workspace' }, output: 'ok' } }));
  db.prepare('insert into part values(?,?,?,?,?,?)').run('p3', 'm_a', 'ses_oc1', t + 2, t + 2, JSON.stringify({ type: 'text', text: 'The build is stable again.' }));
  db.close();
}
// Antigravity: a conversation whose steps are protobuf, as agy stores them
{
  const { DatabaseSync } = await import('node:sqlite');
  const convDir = path.join(home, '.gemini', 'antigravity-cli', 'conversations');
  fs.mkdirSync(convDir, { recursive: true });
  const db = new DatabaseSync(path.join(convDir, 'agy-conv-1.db'));
  db.exec('create table steps(idx integer, step_type integer, status integer, step_payload blob)');
  const pb = (text) => { const b = Buffer.from(text); return Buffer.concat([Buffer.from([0x0a, b.length]), b]); };
  db.prepare('insert into steps values(?,?,?,?)').run(0, 14, 3, pb('Explain the release script'));
  db.prepare('insert into steps values(?,?,?,?)').run(1, 15, 3, pb('It builds hrm.exe and the installer.'));
  db.close();
}
write(path.join(home, 'my-skill', 'SKILL.md'), '---\nname: my-skill\n---\nDo things');

// ---------- driving the window: Playwright over the WebView2 DevTools protocol ----------
// (Microsoft's documented way to automate WebView2 apps; the app is started with remote debugging on.)
let page;
const exec = (script, args = []) => page.evaluate(([s, a]) => new Function(s).apply(null, a), [script, args]);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function waitFor(fn, what, ms = 15000) {
  const end = Date.now() + ms;
  let last;
  while (Date.now() < end) {
    try { last = await fn(); if (last) return last; } catch (e) { last = e; }
    await sleep(200);
  }
  throw new Error(`timed out waiting for ${what}${last instanceof Error ? ': ' + last.message : ''}`);
}
const $ = (css) => waitFor(async () => (await page.$(css)) || null, css);
const click = async (css) => { await $(css); await page.click(css); };
const type = async (css, t) => {
  await $(css);
  const enter = t.endsWith('\n');
  await page.fill(css, enter ? t.slice(0, -1) : t);
  if (enter) await page.press(css, 'Enter');
};
const text = async (css) => { await $(css); return page.innerText(css); };
const hasText = (css, t) => waitFor(async () => (await text(css)).includes(t), `"${t}" in ${css}`);
const nav = async (hash) => { await exec(`location.hash = arguments[0]`, [hash]); await sleep(400); };
const shot = async (name) => page.screenshot({ path: path.join(shots, name + '.png') });
const select = async (css, value) => { await $(css); await page.selectOption(css, value); };

// ---------- run ----------
const results = [];
// HRM_E2E_ONLY=<part of a name> runs only the tests whose name has it, HRM_E2E_REPEAT times (to chase a flaky one)
async function test(name, fn) {
  const only = process.env.HRM_E2E_ONLY;
  if (only && !name.includes(only)) return;
  for (let n = 0; n < (only ? +(process.env.HRM_E2E_REPEAT || 1) : 1); n++) {
    const t0 = Date.now();
    try { await fn(); results.push([name, 'ok', Date.now() - t0]); console.log(`ok   ${name}`); }
    catch (e) {
      results.push([name, 'FAILED', Date.now() - t0, e]);
      console.log(`FAIL ${name}\n     ${e.stack.split('\n').slice(0, 3).join('\n     ')}`);
      // what the page showed, which says more than a timeout
      console.log('     page: ' + String(await exec("return document.getElementById('sheet').innerText.slice(0, 240)").catch(() => '?')).replace(/\s+/g, ' '));
      await shot('FAILED-' + name.replace(/\W+/g, '-')).catch(() => {});
    }
  }
}

const cdpPort = 9300 + (process.pid % 500);
// the app's own error output, for when its window never comes up
const appLog = path.join(work, 'app.log');
const app = spawn(path.join(appDir, 'HarnessManager.exe'), [], { env: { ...env, HRM_DEVTOOLS_PORT: String(cdpPort) }, stdio: ['ignore', fs.openSync(appLog, 'w'), fs.openSync(appLog, 'a')] });
app.on('exit', (code) => console.log(`     [app] exited with ${code}; its output: ${appLog}`));
let browser, exitCode = 1;
try {
  const { chromium } = await import('playwright-core');
  browser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${cdpPort}`).catch(() => null), 'the app window', 60000);
  page = await waitFor(() => browser.contexts().flatMap((c) => c.pages()).find((p) => !p.url().startsWith('devtools')), 'the app page');
  await page.setViewportSize({ width: 1280, height: 800 }).catch(() => {});
  page.on('console', (m) => { if (m.type() === 'error') console.log('     [window] ' + m.text()); });
  page.on('pageerror', (e) => console.log('     [window] ' + e.message));
  // the first-visit tours are tested on their own; everywhere else they would sit in front of the page
  const TOURED = ['home', 'accounts', 'account', 'projects', 'sessions', 'session', 'routing', 'mcp', 'skills', 'prompts', 'settings'];
  await waitFor(() => exec(`return !!document.querySelector('nav.nav a')`), 'the window drawn');
  // (skipping the home tour, when it is already up, must not turn every page's tour off: the tour test needs them)
  await exec(`for (const v of arguments[0]) localStorage.setItem('hm.tour.' + v, '1'); document.querySelector('[data-act=tour-skip]')?.click(); localStorage.removeItem('hm.tours.off');`, [TOURED]);

  await test('overview: key numbers, live routes, limits, reply timing, mix, events, health and the request log', async () => {
    await $('[data-testid=heatmap]');
    assert.equal(await exec("return document.querySelectorAll('[data-testid=heatmap] rect.hm').length"), 371, '53 weeks of days');
    // the latest replies, as the design shows them; Request log opens the whole log
    await hasText('[data-testid=latest-replies]', 'claude-opus-5-5');
    await click('[data-testid=request-log-open]');
    // request log: every number header lines up with its numbers, and the speed and first-token columns are there
    await $('[data-testid=request-log] tbody tr');
    const off = await exec(`const t = document.querySelector('[data-testid=request-log]'); const r = t.querySelector('tbody tr');
      return [...t.querySelectorAll('thead th.num')].map((th) => Math.abs(th.getBoundingClientRect().right - r.children[th.cellIndex].getBoundingClientRect().right));`);
    assert.ok(off.length === 7 && off.every((d) => d < 1), 'number headers aligned: ' + off);
    await $('[data-testid=request-log] [data-testid=ttft]');
    const fill = () => exec("const w = document.querySelector('.hmwrap'); return document.querySelector('[data-testid=heatmap]').getBoundingClientRect().width / w.clientWidth");
    assert.ok((await fill()) > 0.9, 'the heatmap fills its card');
    // a wider window (maximized): it redraws to fill the new width
    await exec("document.querySelector('.hmwrap').style.width = '1800px'; window.dispatchEvent(new Event('resize'))");
    await waitFor(async () => (await fill()) > 0.9, 'the heatmap grows with the window');
    await exec("document.querySelector('.hmwrap').style.width = ''; window.dispatchEvent(new Event('resize'))");
    await waitFor(async () => Math.abs((await fill()) - 1) < 0.1 && (await exec("return document.querySelector('[data-testid=heatmap]').getBoundingClientRect().width <= document.querySelector('.hmwrap').clientWidth")), 'redrawn for the card again');
    await waitFor(async () => (await exec("return document.querySelectorAll('[data-testid=heatmap] rect.hm:not([data-l=\"0\"])').length")) > 0, 'today shaded');
    await hasText('[data-testid=kpi-cost]', '$0.01');
    await hasText('[data-testid=kpi-tokens]', '5 requests');
    await waitFor(async () => (await exec("return document.querySelectorAll('[data-testid=request-log] tbody tr').length")) === 5, 'five requests');
    await hasText('[data-testid=request-log]', 'claude-opus-5-5');
    await hasText('#today-cost', 'Today $');
    await shot('00-home');
    await click('[data-testid=kpi-cost]');
    await click('[data-testid=tool-codex]');
    await waitFor(async () => (await exec("return [...document.querySelectorAll('[data-testid=request-log] tbody tr')].map(r => r.innerText)")).every((t) => t.includes('Codex')), 'only codex requests');
    // as drawn: every shown tool is a choice, Antigravity too (it keeps no usage logs, so its numbers are zero)
    await click('[data-testid=tool-agy]');
    await hasText('[data-testid=request-log]', 'No replies in this range.');
    await click('[data-testid=tool-all]');
    // the other sections, each from real data
    await $('[data-testid=kpi-risk]');
    await $('[data-testid=routes]');
    const topo = await exec(`const t = document.querySelector('[data-testid=routes]');
      return { nodes: [...t.querySelectorAll('[data-node]')].map((n) => n.dataset.node), gl: !!t.querySelector('canvas').getContext('webgl'), w: t.querySelector('canvas').width };`);
    assert.ok(topo.nodes.includes('a:codex-main') && topo.nodes.includes('t:codex') && topo.nodes.includes('v:OpenAI'), 'account, tool and provider cards: ' + topo.nodes);
    assert.ok(topo.gl && topo.w > 0, 'routes drawn with WebGL');
    // no card runs into the next one in its column, however long its text
    const overlaps = await exec(`const r = [...document.querySelectorAll('[data-testid=routes] [data-node]')].map((n) => n.getBoundingClientRect());
      return r.filter((a, i) => r.some((b, j) => j !== i && Math.abs(a.left - b.left) < 2 && a.top < b.top && a.bottom > b.top + 0.5)).length;`);
    assert.equal(overlaps, 0, 'route cards do not overlap');
    await exec("document.querySelector('[data-testid=routes]').scrollIntoView()");
    await shot('00b-overview-routes');
    await hasText('[data-testid=limits]', 'codex-main');
    await hasText('[data-testid=limits]', '79% left');
    await hasText('[data-testid=activity]', 'Activity, 53 weeks');
    // a range picks the period; the cards follow
    await click('[data-testid=range-7d]');
    await waitFor(async () => (await exec("return document.querySelector('[data-testid=range-7d]').classList.contains('on')")), '7 days picked');
    await hasText('[data-testid=kpi-tokens]', '5 requests');
    await exec("document.querySelector('[data-testid=activity]').scrollIntoView()");
    await shot('00c-overview-panels');
    await click('[data-testid=range-24h]');
  });

  await test('sidebar: Setup folds and stays as left, a page inside opens it, the pinned projects fold', async () => {
    await nav('#home');
    const shut = () => exec("return document.getElementById('nav-setup').hidden");
    const was = await shut();
    await click('[data-testid=nav-setup]');
    await waitFor(async () => (await shut()) === !was, 'Setup toggled');
    await click('[data-testid=nav-setup]');
    await waitFor(async () => (await shut()) === was, 'and back');
    if (!(await shut())) { await click('[data-testid=nav-setup]'); await waitFor(async () => shut(), 'shut'); }
    await nav('#skills');
    await waitFor(async () => !(await shut()), 'open for the page shown');
    await click('[data-testid=nav-mcp]');
    await waitFor(async () => (await exec('return location.hash')) === '#mcp', 'MCP servers opened from the fold');
    await click('[data-testid=fold-pins]');
    await waitFor(async () => exec("return document.getElementById('nav-pins').hidden"), 'pins folded');
    await click('[data-testid=fold-pins]');
    await waitFor(async () => !(await exec("return document.getElementById('nav-pins').hidden")), 'and shown');
  });

  await test('accounts list shows the main profiles of installed tools', async () => {
    await nav('#accounts');
    await $('[data-testid=accounts-table]');
    await $('[data-testid=row-claude-main]');
    await $('[data-testid=row-codex-main]');
    await hasText('[data-testid=row-codex-main]', 'ChatGPT');
    // every account with plan limits has a row in Limits on Overview, read or not
    await nav('#home');
    await hasText('[data-testid=limits]', 'claude-main');
    await hasText('[data-testid=limits]', 'codex-main');
    await nav('#accounts');
    await hasText('[data-testid=row-codex-main]', '79% left');
    await shot('01-accounts');
  });

  await test('add a Claude account: in-app sign-in with a pasted code, then plan and limits', async () => {
    await click('[data-testid=add-account]');
    await type('[data-testid=add-name]', 'work-max');
    await type('[data-testid=add-email]', 'alex@example.com');
    await hasText('.dlg .clibox', 'hrm add work-max --email alex@example.com');
    await shot('02-add-account');
    await click('[data-testid=add-create]');
    await $('[data-testid=login-code]');
    await hasText('[data-testid=login-status]', 'browser');
    await type('[data-testid=login-code]', 'good-code');
    await click('[data-testid=login-send]');
    await hasText('[data-testid=login-status]', 'Signed in as alex@example.com');
    await shot('03-signed-in');
    await click('[data-testid=login-done]');
    await hasText('[data-testid=row-work-max]', 'Max 20x');
    await hasText('[data-testid=row-work-max]', '36% left');
    assert.ok(fs.existsSync(path.join(home, '.claude-work-max', '.credentials.json')));
    assert.ok(fs.existsSync(path.join(home, '.claude-work-max', 'skills', 'pdf', 'SKILL.md')), 'skills linked from the main profile');
    await hasText('[data-testid=best-note]', 'Most room of any account');
    await shot('04-accounts-with-limits');
    // the README's screenshot: the same page with a made-up computer name and the fixture's year-2099 resets as real ones
    await exec(`const swap = [[new RegExp(arguments[0], 'gi'), 'DEVBOX']];
      let resets = 0; // the first is the 5-hour window, the rest are weekly
      swap.push([/\\d{5} d \\d+ h/g, () => (resets++ ? '4 d 6 h' : '2 h 14 m')]);
      const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      for (let n; (n = w.nextNode());) for (const [a, b] of swap) n.nodeValue = n.nodeValue.replace(a, b);
      document.getElementById('toast').className = 'off';
      document.documentElement.dataset.theme = 'dark';`, [os.hostname()]); // the README shows the dark look, Harness
    await sleep(400);
    // HRM_README_SHOT=1 refreshes the tracked copy; otherwise it stays with the other screenshots
    await page.screenshot({ path: process.env.HRM_README_SHOT ? path.join(root, 'docs', 'screenshot.png') : path.join(shots, 'readme.png') });
    await exec("document.documentElement.dataset.theme = 'light'");
    await nav('#home');
    await nav('#accounts');
  });

  await test('closing the sign-in dialog stops the waiting sign-in', async () => {
    const fakeLogins = () => Number(execFileSync('powershell', ['-NoProfile', '-Command',
      `@(Get-Process claude -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq '${path.join(bin, 'claude.exe')}' }).Count`], { encoding: 'utf8' }).trim());
    await click('[data-testid=add-account]');
    await type('[data-testid=add-name]', 'cancel-me');
    await click('[data-testid=add-create]');
    await $('[data-testid=login-code]');
    await waitFor(() => fakeLogins() === 1, 'claude auth login running');
    await click('.dlg .dh [data-act=dialog-close]');
    await waitFor(() => fakeLogins() === 0, 'claude auth login stopped');
    hrm('remove', 'cancel-me', '--delete-home');
    await nav('#home');
    await nav('#accounts');
  });

  await test('Start opens a terminal running the account', async () => {
    await click('[data-testid=start-work-max]');
    await waitFor(() => readLines(termLog).some((t) => JSON.stringify(t.args) === JSON.stringify(['run', 'work-max'])), 'terminal launch');
  });

  await test('add an API-key account', async () => {
    await click('[data-testid=add-account]');
    await click('[data-testid=kind-api_key]');
    await type('[data-testid=add-name]', 'team-ci');
    await type('[data-testid=add-key]', 'sk-ant-e2e');
    await click('[data-testid=add-create]');
    await hasText('[data-testid=row-team-ci]', 'API key');
    hrm('run', 'team-ci');
    assert.equal(readLines(runLog).pop().env.ANTHROPIC_API_KEY, 'sk-ant-e2e');
  });

  await test('add a Codex account: sign-in opens in a terminal', async () => {
    await click('[data-testid=add-account]');
    await select('[data-testid=add-harness]', 'codex');
    await type('[data-testid=add-name]', 'codex-personal');
    await click('[data-testid=add-create]');
    await waitFor(() => readLines(termLog).some((t) => JSON.stringify(t.args) === JSON.stringify(['login', 'codex-personal'])), 'login terminal');
    hrm('login', 'codex-personal'); // what the user does in that terminal
    // the dialog notices the sign-in on its own, and Done redraws the list without leaving the page
    await hasText('[data-testid=login-status]', 'Signed in as alex@example.com');
    await click('[data-testid=login-done]');
    await hasText('[data-testid=row-codex-personal]', 'ChatGPT Pro');
  });

  await test('account detail: overview, sessions, projects and settings tabs', async () => {
    await nav('#account?name=claude-main');
    await hasText('[data-testid=detail-name]', 'claude-main');
    await shot('05-account-detail');
    await click('[data-testid=tab-sessions]');
    await $('[data-testid=session-s-main-1]');
    await click('[data-testid=tab-projects]');
    await hasText('.sheet', 'platform-gitops');
    await click('[data-testid=tab-settings]');
    await $('#pf-email');
  });

  await test('Start in a folder: a recent folder of the account, skip permission prompts, extra arguments', async () => {
    await nav('#account?name=claude-main');
    await click('[data-act=start-in-folder]');
    await hasText('[data-testid=sd-dir]', repo);
    await click('[data-testid=sd-skip]');
    await type('[data-testid=sd-args]', '--model opus --append-system-prompt "Be brief"');
    await shot('05b-start-in-folder');
    await click('[data-testid=sd-ok]');
    await waitFor(() => readLines(termLog).some((t) => t.cwd === repo && t.args.join(' ') === 'run claude-main -- --dangerously-skip-permissions --model opus --append-system-prompt Be brief'), 'started there with those arguments');
  });

  await test('sessions are grouped by repository across Claude Code and Codex', async () => {
    await nav('#sessions');
    await $('[data-testid=group]');
    const groups = await exec(`return [...document.querySelectorAll('[data-testid=group]')].map(g => g.innerText)`);
    const g = groups.find((x) => x.includes('platform-gitops'));
    assert.ok(g, 'a platform-gitops group');
    assert.ok(g.includes('Claude Code') && g.includes('Codex'), 'both tools in one project');
    // as in the design: a red dot for a session where a command failed
    assert.ok(await exec("return !!document.querySelector('[data-testid=session-s-main-1] .sdot.bad')"), 'the session with a failed command is marked');
    await shot('06-sessions');
    // a new session in the project's folder, as any account: the project's latest account comes first
    await exec(`[...document.querySelectorAll('[data-testid=group]')].find((g) => g.innerText.includes('platform-gitops')).querySelector('[data-testid=project-new]').click()`);
    await hasText('[data-testid=np-dir]', repo);
    assert.ok(!(await exec(`return !!document.querySelector('[data-testid=np-choose]')`)), 'a project session keeps its folder');
    await click('[data-testid=np-codex-main]');
    await $('[data-testid=np-skip]');
    await shot('06b-new-session-in-project');
    await click('[data-testid=np-ok]');
    await waitFor(() => readLines(termLog).some((t) => t.cwd === repo && t.args.join(' ').startsWith('run codex-main')), 'codex-main started in the project folder');
    // a new session outside any project: the folder is picked in the dialog, and it cannot start before that
    await click('[data-testid=session-new]');
    await hasText('[data-testid=np-dir]', 'No folder chosen');
    await $('[data-testid=np-choose]');
    assert.ok(await exec(`return document.querySelector('[data-testid=np-ok]').disabled`), 'no start without a folder');
    await shot('06c-new-session');
    await click('.dlg .dh [data-act=dialog-close]');
    await click('[data-testid=group-tool]');
    await waitFor(async () => (await exec(`return document.querySelectorAll('[data-testid=group]').length`)) === 4, 'four tool groups');
    await click('[data-testid=group-project]');
    await type('[data-testid=find]', 'longhorn');
    await waitFor(async () => (await exec(`return document.querySelectorAll('.srow[data-search]:not([hidden])').length`)) === 1, 'the top bar search narrows to one session');
    await page.fill('[data-testid=find]', '');
    await exec(`document.getElementById('find').dispatchEvent(new Event('input'))`);
    await waitFor(async () => (await exec(`return document.querySelectorAll('.srow[data-search]:not([hidden])').length`)) > 1, 'every session is back');
  });

  await test('projects: cost per project, pin, rename and tabs in place, and a project opened in Sessions', async () => {
    await nav('#projects');
    await $('[data-testid=project-tiles]');
    const row = () => exec("return [...document.querySelectorAll('[data-testid^=pcard-]')].find((c) => c.innerText.includes('platform-gitops'))?.innerText || ''");
    // the Claude session priced from the catalogue: 1200 in at $4/M, 300 out at $20/M and the small follow-ups
    await waitFor(async () => /\$\d+\.\d\d/.test(await row()), 'a cost on the row');
    await hasText('[data-testid=project-dist]', 'platform-gitops');
    await shot('06d-projects');
    // the sort, the chart's tabs and the page size redraw their part only: the tiles stay the same nodes
    await exec("window.__tiles = document.getElementById('pj-tiles')");
    await click('[data-testid=psort-cost]');
    await waitFor(() => exec("return document.querySelector('[data-testid=psort-cost]')?.classList.contains('on')"), 'sorted by cost');
    await click('[data-testid=pmetric-cost]');
    await waitFor(() => exec("return document.querySelector('[data-testid=pmetric-cost]')?.classList.contains('on')"), 'distribution by cost');
    assert.ok(await exec("return document.getElementById('pj-tiles') === window.__tiles"), 'the tiles were not redrawn');
    assert.ok((await exec('return location.hash')).includes('metric=cost'), 'the choice is in the hash: ' + (await exec('return location.hash')));
    // pin from the row's icon: the sidebar shows it, settings keep it, the page stays
    await exec("[...document.querySelectorAll('[data-testid^=pcard-]')].find((c) => c.innerText.includes('platform-gitops')).querySelector('[data-act=project-pin]').click()");
    await waitFor(async () => (await exec("return document.querySelector('[data-testid=nav-pins]').innerText")).includes('platform-gitops'), 'pinned in the sidebar');
    assert.ok(JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).pinned_projects.length === 1, 'saved in settings');
    assert.ok(await exec("return document.getElementById('pj-tiles') === window.__tiles"), 'pinning did not redraw the page');
    // the ✕ beside the pin in the sidebar takes it off; the card pins it again
    await exec("document.querySelector('[data-testid=nav-pins] .unpin').click()");
    await waitFor(async () => !(await exec("return document.querySelector('[data-testid=nav-pins]').innerText")).includes('platform-gitops'), 'unpinned from the sidebar');
    await waitFor(async () => exec("return [...document.querySelectorAll('[data-testid^=pcard-]')].find((c) => c.innerText.includes('platform-gitops'))?.querySelector('[data-act=project-pin]')?.dataset.on === '1'"), 'the card shows it unpinned');
    await exec("[...document.querySelectorAll('[data-testid^=pcard-]')].find((c) => c.innerText.includes('platform-gitops')).querySelector('[data-act=project-pin]').click()");
    await waitFor(async () => (await exec("return document.querySelector('[data-testid=nav-pins]').innerText")).includes('platform-gitops'), 'pinned again from the card');
    await exec("[...document.querySelectorAll('[data-testid^=pcard-]')].find((c) => c.innerText.includes('platform-gitops')).querySelector('[data-act=project-pin]').click()");
    await waitFor(async () => !(await exec("return document.querySelector('[data-testid=nav-pins]').innerText")).includes('platform-gitops'), 'unpinned');
    // rename by double-clicking the name: Enter saves, the row and the CLI show the new name; the pencil renames back
    await page.dblclick('[data-testid^=pcard-] [data-name]');
    await type('[data-testid=rename-input]', 'Platform GitOps\n');
    await waitFor(async () => (await row()).includes('Platform GitOps'), 'renamed on the row');
    assert.ok(hrm('projects', 'list').includes('Platform GitOps'), 'the CLI sees the new name');
    await exec("[...document.querySelectorAll('[data-testid^=pcard-]')].find((c) => c.innerText.includes('Platform GitOps')).querySelector('[data-act=project-rename]').click()");
    await type('[data-testid=rename-input]', 'platform-gitops\n');
    await waitFor(async () => (await exec("return document.querySelector('[data-testid=project-cards]').innerText")).includes('platform-gitops'), 'renamed back');
    await waitFor(() => !hrm('projects', 'list').includes('Platform GitOps'), 'and on disk, before the project opens');
    // open the project by clicking the row: Sessions shows only it, with its numbers in the header
    await exec("[...document.querySelectorAll('[data-testid^=pcard-]')].find((c) => c.innerText.includes('platform-gitops')).click()");
    await hasText('[data-testid=project-title]', 'platform-gitops');
    await hasText('[data-testid=project-stats]', 'at list prices');
    assert.ok(await exec("return [...document.querySelectorAll('.srow[data-search]')].every((r) => r.dataset.search.includes('platform-gitops'))"), 'only the project\'s sessions');
    await shot('06e-project-sessions');
    // the header's pin icon pins and unpins too
    await click('.sesshead [data-act=project-pin]');
    await waitFor(async () => (await exec("return document.querySelector('[data-testid=nav-pins]').innerText")).includes('platform-gitops'), 'pinned from the header');
    await click('.sesshead [data-act=project-pin]');
    await waitFor(async () => !(await exec("return document.querySelector('[data-testid=nav-pins]').innerText")).includes('platform-gitops'), 'unpinned from the header');
    await nav('#sessions');
    await hasText('#sheet h1', 'Sessions');
    await $('[data-testid=rail-live]'); // the title shows while the list loads; the rail comes with the list
    assert.equal(await exec("return document.querySelector('[data-testid=rail-live]').getAttribute('href')"), '#sessions?live=1');
  });

  await test('reading a session: steps, failed command, answer and changes', async () => {
    await nav('#sessions');
    await click('[data-testid=read-s-main-1]');
    await hasText('[data-testid=session-title]', 'Longhorn backup target');
    // the steps inline in the chat, folded: each says what it ran; a click unfolds its output
    await hasText('[data-testid=chat] .steps', 'argocd app sync longhorn-staging');
    assert.ok(!(await text('[data-testid=chat]')).includes('connection closed'), 'output folded');
    await click('[data-testid=chat] .si:has(.negT) [data-act=step-open]');
    await hasText('[data-testid=chat]', 'connection closed');
    // the conversation as a chat: the prompt on the right, then the steps, then the answer, in the order they happened
    await hasText('[data-testid=chat] .msg.you', 'Add a Longhorn backup target');
    await hasText('[data-testid=chat] .msg.ai', 'What changed');
    const order = await exec("return [...document.querySelectorAll('#chat > .msg, #chat > .steps')].map((e) => e.className.split(' ')[0] + (e.classList.contains('you') ? ':you' : e.classList.contains('ai') ? ':ai' : ''))");
    assert.deepEqual(order, ['msg:you', 'steps', 'msg:ai'], 'prompt, steps, answer');
    await shot('07-session');
    await click('[data-testid=mode-changes]');
    await click('[data-testid=chat] [data-act=step-open]');
    await hasText('[data-testid=chat]', 's3://longhorn-backups');
    await shot('08-session-changes');
    await click('[data-testid=mode-failed]');
    await hasText('[data-testid=chat]', 'argocd app sync');
    assert.ok(!(await text('[data-testid=chat]')).includes('values-staging.yaml'), 'Failed lists only the failed command');
    await click('[data-testid=resume]');
    await waitFor(() => readLines(termLog).some((t) => t.args.join(' ') === 'run claude-main -- --resume s-main-1'), 'resume in the session account');
    await click('[data-testid=mode-all]');
  });

  await test('an open session shows new messages as its tool writes them', async () => {
    const f = path.join(home, '.claude', 'projects', 'p', 's-live.jsonl');
    try {
    const t0 = new Date().toISOString();
    // long enough to scroll
    const pair = (i, q, a) => [{ type: 'user', sessionId: 's-live', cwd: repo, timestamp: t0, message: { role: 'user', content: q } },
      { type: 'assistant', sessionId: 's-live', cwd: repo, timestamp: t0, message: { id: 'l' + i, model: 'claude-opus-5-5', usage: { input_tokens: 5, output_tokens: 5 }, content: [{ type: 'text', text: a } ] } }];
    write(f, jl([...pair(0, 'Watch me live', 'First answer.'), ...Array.from({ length: 30 }, (_, i) => pair(i + 1, 'Question ' + i, 'Answer ' + i)).flat()]));
    await nav('#session?profile=claude-main&id=s-live');
    await hasText('[data-testid=chat]', 'First answer.');
    // the sandbox has no running console, so start the watcher the page starts for an open session
    // (each redraw re-reads whether the session is open, so it is started again after each one)
    const live = () => exec("return import('./views.js').then((v) => import('./core.js').then((c) => v.watchLive({ ...c.S.cache.transcript.meta, open: true })))");
    await live();
    const atEnd = () => exec("const c = document.getElementById('chat'); return c.scrollHeight - c.scrollTop - c.clientHeight < 40");
    const shown = () => exec("return !document.querySelector('.toend').hidden");
    // not at the end: the button shows; it jumps to the end and follows
    assert.ok(!(await atEnd()) && (await shown()), 'opened at the top with the button showing');
    await click('[data-testid=follow-end]');
    await waitFor(async () => (await atEnd()) && !(await shown()), 'at the end, button gone');
    await sleep(2500);
    // a live redraw swaps the conversation in place; the page never blanks to "Opening the session…"
    await exec("window.__blank = false; new MutationObserver(() => { if (!document.getElementById('chat')) window.__blank = true; }).observe(document.getElementById('sheet'), { childList: true })");
    fs.appendFileSync(f, jl(pair(98, 'And now?', 'Written while you watched.')));
    await hasText('[data-testid=chat]', 'Written while you watched.');
    assert.ok(!(await exec('return window.__blank')), 'the page stayed on screen during the live redraw');
    await waitFor(atEnd, 'following: still at the end after new messages');
    // scrolled up: following stops, the place is kept, and the button is back
    await exec("const c = document.getElementById('chat'); c.scrollTop = 100; c.dispatchEvent(new Event('scroll'))");
    await waitFor(shown, 'the button shows again');
    await live();
    await sleep(2500);
    fs.appendFileSync(f, jl(pair(99, 'One more?', 'Another live line.')));
    await hasText('[data-testid=chat]', 'Another live line.');
    assert.equal(await exec("return document.getElementById('chat').scrollTop"), 100, 'not following: the place is kept');
    } finally { fs.rmSync(f, { force: true }); await nav('#sessions'); }
  });

  await test('tags: on a session and a project, grouped and filtered by tag, and seen by the CLI', async () => {
    await nav('#sessions');
    await $('[data-testid=session-s-main-1]');
    // the + on the row opens the editor; Enter adds a tag; Save writes it and the row shows the chip
    await click('[data-testid=session-s-main-1] .tagb');
    await type('[data-testid=tag-new]', 'release\n');
    await waitFor(async () => exec("return [...document.querySelectorAll('#tg-list input:checked')].map((i) => i.value).includes('release')"), 'release ticked');
    await click('[data-testid=tag-save]');
    await hasText('[data-testid=session-s-main-1]', 'release');
    assert.ok(JSON.parse(hrm('tags', '--json')).some((t) => t.tag === 'release' && t.sessions === 1), 'the CLI lists it');
    // Group by Tag: a release group and an untagged one; the chip filters to the tagged session
    await click('[data-testid=group-tag]');
    await waitFor(async () => (await exec("return [...document.querySelectorAll('[data-testid=group] .gname')].map((e) => e.textContent.trim())")).join(',').includes('release'), 'a release group');
    await hasText('#sheet', 'untagged');
    // (the toast sits over the row at this window size, so the chip is clicked by script)
    await exec("document.querySelector('[data-testid=session-s-main-1] .tags a.tagc').click()");
    await $('[data-testid=chip-tag]');
    await waitFor(async () => (await exec("return document.querySelectorAll('.srow[data-search]').length")) === 1, 'only the tagged session');
    assert.ok(await exec("return !!document.querySelector('[data-testid=rail-tag-release]')"), 'the rail lists the tag');
    await shot('06f-sessions-tags');
    // a project's tag reaches its sessions: tag platform-gitops from the Projects page, then filter by it
    await nav('#projects');
    await $('[data-testid=project-cards]');
    await exec("[...document.querySelectorAll('[data-testid^=pcard-]')].find((c) => c.innerText.includes('platform-gitops')).querySelector('.tagb').click()");
    await type('[data-testid=tag-new]', 'client-x\n');
    await click('[data-testid=tag-save]');
    await waitFor(async () => (await exec("return [...document.querySelectorAll('[data-testid^=pcard-]')].find((c) => c.innerText.includes('platform-gitops')).innerText")).includes('client-x'), 'the card shows it');
    await nav('#sessions?tag=client-x');
    await waitFor(async () => (await exec("return document.querySelectorAll('.srow[data-search]').length")) >= 2, 'every session of the project carries the project tag');
    assert.ok(hrm('sessions', '--tag', 'client-x').includes('platform-gitops'), 'the CLI filters by tag too');
    await click('[data-testid=chip-tag]');
    await click('[data-testid=rail-all]');
  });

  await test('OpenCode and Antigravity sessions are listed and readable', async () => {
    await nav('#sessions');
    await hasText('[data-testid=sessions-finding]', 'from Antigravity, OpenCode, Claude Code and Codex');
    const noFolder = await exec("return [...document.querySelectorAll('[data-testid=group]')].find((g) => g.textContent.includes('No folder')).textContent");
    assert.ok(!noFolder.includes('github.com'), 'a session without a folder has no repository');
    assert.ok(noFolder.includes('1 msg') || noFolder.includes('2 msgs'));
    await click('[data-testid=read-ses_oc1]');
    await hasText('[data-testid=session-title]', 'Fix the flaky build');
    await hasText('[data-testid=chat]', 'cargo test --workspace');
    await hasText('[data-testid=chat]', 'The build is stable again.');
    await nav('#sessions');
    await click('[data-testid=read-agy-conv-1]');
    await hasText('[data-testid=chat]', 'Explain the release script');
    await hasText('[data-testid=chat]', 'It builds hrm.exe and the installer.');
    await click('[data-testid=resume]');
    await waitFor(() => readLines(termLog).some((t) => t.args.join(' ') === 'run agy-main -- --conversation agy-conv-1'), 'resume the Antigravity conversation');
  });

  await test('a turn with thousands of steps shows its first and last steps until asked', async () => {
    const steps = [];
    for (let k = 0; k < 2000; k++) steps.push({ type: 'response_item', timestamp: now, payload: { type: 'function_call', name: 'exec_command', call_id: 'c' + k, arguments: JSON.stringify({ cmd: 'step ' + k }) } });
    write(path.join(home, '.codex', 'sessions', '2026', '11', '01', 'rollout-2026-11-01T11-00-00-cx-long.jsonl'), jl([
      { type: 'session_meta', timestamp: now, payload: { id: 'cx-long', cwd: repo } },
      { type: 'event_msg', timestamp: now, payload: { type: 'user_message', message: 'Run the long migration' } }, ...steps,
      { type: 'response_item', timestamp: now, payload: { type: 'message', role: 'assistant', content: [{ type: 'output_text', text: 'Migration done.' }] } }]));
    await nav('#session?profile=codex-main&id=cx-long');
    await hasText('[data-testid=chat]', 'Migration done.');
    assert.equal(await exec("return document.querySelectorAll('#chat .si').length"), 11, 'first 5, a link, last 5');
    await hasText('[data-testid=more-steps]', 'Show 1,990 more steps');
    await click('[data-testid=more-steps]');
    await waitFor(async () => (await exec("return document.querySelectorAll('#chat .si').length")) === 2000, 'all steps shown');
  });

  await test('export a session as Markdown', async () => {
    const out = path.join(sb, 'export.md');
    await nav('#session?profile=claude-main&id=s-main-1');
    await hasText('[data-testid=session-title]', 'Longhorn');
    // the native save dialog cannot be driven; answer it with a path instead
    await exec('window.__TAURI__.dialog.save = async () => arguments[0]', [out]);
    await click('[data-testid=export-md]');
    await waitFor(() => fs.existsSync(out), 'export file');
    const md = fs.readFileSync(out, 'utf8');
    assert.ok(md.startsWith('# Longhorn backup target for staging'));
    assert.ok(md.includes('## You') && md.includes('Add a Longhorn backup target for staging'));
    assert.ok(md.includes('`Bash` argocd app sync longhorn-staging (failed)'));
    assert.ok(md.includes('What changed'));
  });

  await test('the window follows the Mission Control design in the Harness theme', async () => {
    await nav('#accounts');
    await $('[data-testid=accounts-table]');
    await page.mouse.move(2, 2); // no hover colour on the button being measured
    const d = await exec(`
      const cs = (sel) => getComputedStyle(document.querySelector(sel));
      const on = document.querySelector('nav.nav a.on');
      return {
        frame: getComputedStyle(document.body).backgroundColor,
        sheetBg: cs('.sheet').backgroundColor, sheetRadius: cs('.sheet').borderTopLeftRadius, sheetShadow: cs('.sheet').boxShadow,
        side: document.querySelector('aside.side').getBoundingClientRect().width,
        top: document.querySelector('header.top').getBoundingClientRect().height,
        navActive: getComputedStyle(on).backgroundColor, navLabel: on.textContent.replace(on.querySelector('.count')?.textContent || '', '').trim(),
        font: getComputedStyle(document.body).fontFamily, inter: document.fonts.check('14px "Inter"'), mark: !!document.querySelector('#brand-mark svg linearGradient'),
        groups: [...document.querySelectorAll('[data-testid=accounts-table] .acg > header b')].map((t) => t.textContent),
        primary: cs('.btn.primary').backgroundColor,
        chips: [...document.querySelectorAll('.fbar .toolfilter a')].map((a) => a.textContent.trim()),
      };`);
    assert.equal(d.frame, 'rgb(245, 247, 250)', 'the brand light ground');
    assert.equal(d.sheetBg, 'rgba(0, 0, 0, 0)', 'an open page: cards sit on the ground');
    assert.equal(d.sheetRadius, '0px', 'no framed sheet');
    assert.equal(d.sheetShadow, 'none', 'no shadow on the page');
    assert.equal(d.side, 232, 'sidebar width');
    assert.equal(d.top, 64, 'top bar height');
    assert.ok(d.navActive !== 'rgba(0, 0, 0, 0)' && d.navActive !== d.frame, 'the active item is shaded: ' + d.navActive);
    assert.equal(d.navLabel, 'Accounts');
    assert.ok(d.font.includes('Inter') && d.inter, 'Inter loaded');
    assert.ok(d.mark, 'the Interlocking H in the sidebar');
    assert.deepEqual(d.groups, ['Claude Code', 'Codex', 'OpenCode', 'Antigravity', 'Pi', 'Oh My Pi'], 'accounts grouped by tool, one card each');
    assert.equal(d.primary, 'rgb(15, 110, 122)', 'the primary button in brand teal');
    assert.deepEqual(d.chips, ['All', 'Claude Code', 'Codex', 'OpenCode', 'Antigravity', 'Pi', 'Oh My Pi'], 'a chip for each tool');
    // a tool chip shows only that tool; a group header folds its accounts away
    await click('[data-testid=chip-codex]');
    await waitFor(async () => (await exec("return document.querySelectorAll('[data-testid=accounts-table] .acg').length")) === 1, 'only Codex');
    await click('[data-testid=chip-all]');
    await click('[data-testid=group-claude] [data-act=acct-fold]');
    await waitFor(async () => !(await exec("return !!document.querySelector('[data-testid=row-claude-main]')")), 'Claude Code folded');
    await click('[data-testid=group-claude] [data-act=acct-fold]');
    await $('[data-testid=row-claude-main]');
    await click('[data-testid=add-account]');
    const dlg = await exec("const s = getComputedStyle(document.querySelector('.dlg')); return { radius: s.borderTopLeftRadius, tab: getComputedStyle(document.querySelector('.dseg a.on')).boxShadow };");
    assert.equal(dlg.radius, '16px', 'Add account is a dialog over the list');
    assert.ok(dlg.tab.includes('rgb(15, 110, 122)'), 'teal underline on the selected tab');
    await exec("document.querySelector('[data-act=dialog-close]').click()");
    await nav('#session?profile=claude-main&id=s-main-1');
    await $('[data-testid=chat]');
    const r = await exec("return { panel: !!document.querySelector('[data-testid=conv-panel]'), modes: [...document.querySelectorAll('[data-testid=conv-panel] .seg3 a')].map((a) => a.textContent.trim()), resume: document.querySelector('[data-testid=resume]').textContent.trim() }");
    assert.ok(r.panel, 'the Conversation panel');
    assert.deepEqual(r.modes, ['All', 'Changes', 'Failed']);
    assert.ok(r.resume.startsWith('Resume in'));
  });

  // The design pages (Main, PartNav, PartTopbar, PartHeader, PartKpis, PartBurn) set these sizes; the window must use the same.
  await test('measurements match the design pages', async () => {
    await nav('#home');
    await $('[data-testid=limits]');
    const d = await exec(`
      const cs = (el) => getComputedStyle(el), q = (s) => document.querySelector(s);
      const box = (s) => q(s).getBoundingClientRect();
      const sheet = cs(q('#sheet')), h1 = cs(q('#sheet h1')), panel = cs(q('.panel')), ph2 = cs(q('.panel h2')), kpi = cs(q('.ovk')), kv = cs(q('.ovk strong'));
      const nav = q('nav.nav a'), btn = q('.ovhead .btn.primary'), seg = cs(q('.ovhead .seg3'));
      return {
        side: box('aside.side').width, top: box('header.top').height, padTop: sheet.paddingTop, padLeft: sheet.paddingLeft,
        h1: [h1.fontSize, h1.fontWeight], body: [cs(document.body).fontSize, cs(document.body).fontFamily.split(',')[0]],
        panel: [panel.borderTopLeftRadius, panel.paddingTop, panel.paddingLeft], h2: [ph2.fontSize, ph2.fontWeight],
        kpi: [kpi.borderTopLeftRadius, kpi.paddingTop, kpi.paddingLeft, kpi.paddingBottom], kv: [kv.fontSize, kv.fontWeight, kv.fontFamily.split(',')[0]],
        nav: [nav.getBoundingClientRect().height, cs(nav).borderTopLeftRadius], btn: [btn.getBoundingClientRect().height, cs(btn).borderTopLeftRadius],
        seg: [seg.borderTopLeftRadius, seg.paddingTop], search: [box('.search').height, cs(q('.search')).borderTopLeftRadius],
        gaps: (() => { const k = [...document.querySelectorAll('.ovk')].map((e) => e.getBoundingClientRect()); return Math.round(k[1].left - k[0].right); })(),
      };`);
    assert.equal(d.side, 232, 'sidebar 232 px');
    assert.equal(d.top, 64, 'top bar 64 px');
    assert.deepEqual([d.padTop, d.padLeft], ['28px', '40px'], 'page padding 28 / 40');
    assert.deepEqual(d.h1, ['34px', '600'], 'page title 34 px, 600');
    assert.deepEqual([d.body[0], d.body[1].replace(/"/g, '')], ['14px', 'Inter'], 'body 14 px Inter');
    assert.deepEqual(d.panel, ['16px', '18px', '20px'], 'cards: radius 16, padding 18 / 20');
    assert.deepEqual(d.h2, ['18px', '600'], 'card titles 18 px, 600');
    assert.deepEqual(d.kpi, ['14px', '14px', '16px', '12px'], 'number cards: radius 14, padding 14 / 16 / 12');
    assert.deepEqual([d.kv[0], d.kv[1], d.kv[2].replace(/"/g, '')], ['26px', '600', 'JetBrains Mono'], 'numbers 26 px, 600, JetBrains Mono');
    assert.equal(d.gaps, 12, '12 px between number cards');
    assert.deepEqual(d.nav, [38, '9px'], 'nav items 38 px, radius 9');
    assert.deepEqual(d.btn, [40, '10px'], 'buttons 40 px, radius 10');
    assert.deepEqual(d.seg, ['10px', '3px'], 'segments: radius 10, padding 3');
    assert.deepEqual(d.search, [40, '10px'], 'search 40 px, radius 10');
  });

  // Each page has the design page's sections, in its order: the titles of the design (rendered from the canvas)
  // must appear among the page's headings in that order; headings named by data (accounts, routes) may sit between.
  await test('every page has the sections of its design page, in order', async () => {
    const want = {
      '#home': ['Overview', 'Live routes', 'Limits', 'Activity, 53 weeks', 'Latest replies'],
      '#accounts': ['Accounts', 'Tokens per day, by account', 'Claude Code', 'Codex', 'OpenCode', 'Antigravity', 'By model'],
      '#account?name=claude-main': ['claude-main', 'Limits', 'Tokens per day, 30 days', 'Login and folder', 'Recent sessions'],
      '#projects': ['Projects', 'Where the tokens and the cost went'],
      '#sessions': ['Sessions', 'platform-gitops'],
      '#session?profile=claude-main&id=s-main-1': ['Longhorn backup target for staging', 'Conversation'],
      '#routing': ['API keys & routing', 'Local router', 'Routes', 'Providers', 'Recent requests through the router'],
      '#mcp': ['MCP servers'], '#skills': ['Skills'], '#prompts': ['System prompts'],
      '#apps': ['Apps', 'Harness Manager'],
      '#settings': ['Settings', 'Appearance', 'Window and terminal', 'Updates', 'Usage and limits', 'Connections', 'Diagnostic log', 'Maintenance', 'Backups'],
      '#cli': ['CLI reference', 'Accounts', 'Run', 'Usage and limits', 'Sessions and projects', 'Shared setup', 'Routing and setup'],
    };
    // the replies panel as the design opens it (an earlier test left the full log open)
    await nav('#home');
    await $('[data-testid=limits]');
    if (await exec("return !!document.querySelector('[data-testid=request-log]')")) await click('[data-testid=request-log-open]');
    for (const [h, titles] of Object.entries(want)) {
      const before = await exec("return document.querySelector('#sheet h1')?.textContent || ''");
      await nav(h);
      await waitFor(async () => (await exec("return document.querySelector('#sheet h1')?.textContent || ''")) !== before || h === '#home', h + ' drawn');
      await waitFor(async () => {
        const got = await exec("return [...document.querySelectorAll('#sheet h1, #sheet h2')].map((e) => e.textContent.replace(/\\s+/g, ' ').trim())");
        let i = 0;
        for (const g of got) if (i < titles.length && g.startsWith(titles[i])) i++;
        return i === titles.length;
      }, `${h}: ${titles.join(' / ')}`);
    }
    // Settings: as many rows in each section as the design counts
    await nav('#settings');
    await $('[data-testid=sec-appearance]');
    const counts = await exec("return [...document.querySelectorAll('.sset')].map((s) => s.querySelectorAll('.frow').length)");
    assert.deepEqual(counts.slice(0, 7), [3, 3, 2, 2, 2, 3, 3], 'settings per section, as the design counts them (less the Classic choice and the two Overview rows)');
  });

  await test('usage lives on Accounts: the chart, each row\'s tokens and cost, the range, and the old address', async () => {
    await nav('#accounts');
    await $('[data-testid=usage-panel] [data-testid=usage-chart]');
    // the Claude session priced from the catalogue; Codex's gpt-6 too
    await waitFor(async () => /\$\d+\.\d\d/.test(await text('[data-testid=cost-claude-main]')), 'a cost on the Claude row');
    await hasText('[data-testid=group-codex] header', 'tokens');
    await click('[data-testid=u-days-30]');
    await hasText('[data-testid=row-claude-main]', '30 days');
    await click('[data-testid=u-days-7]');
    await hasText('[data-testid=row-claude-main]', '7 days');
    await shot('09-usage');
    // the old page's address opens Accounts
    await nav('#usage');
    await waitFor(() => exec("return location.hash === '#accounts'"), 'redirected to Accounts');
  });

  await test('routing: add a provider and a route, start and stop the router', async () => {
    const port = 20000 + (process.pid % 20000);
    hrm('settings', 'router_port', String(port));
    await nav('#routing');
    await click('[data-testid=provider-add]');
    await type('[data-testid=pv-id]', 'openrouter');
    await select('[data-testid=pv-format]', 'openai');
    await type('[data-testid=pv-url]', 'https://openrouter.ai/api/v1');
    await type('[data-testid=pv-models]', 'deepseek/deepseek-v4, z-ai/glm-5');
    await type('[data-testid=pv-key]', 'sk-or-e2e');
    await click('[data-testid=pv-save]');
    await hasText('[data-testid=providers]', 'openrouter');
    await click('[data-testid=route-add]');
    await type('[data-testid=rt-id]', 'cheap');
    await select('[data-testid=rt-mode]', 'aggregate');
    await click('[data-testid=rt-save]');
    await hasText('[data-testid=routes]', 'cheap');
    await click('[data-testid=router-start]');
    await hasText('[data-testid=router-state]', 'running');
    await shot('10-routing');
    assert.ok(hrm('router', 'models', 'cheap').includes('hm-openrouter--z-ai/glm-5'));
    hrm('router', 'stop');
    // a routed account's daily spend cap: set in its settings, shown on Overview's burn-down and timeline
    hrm('add', 'capped', '--route', 'cheap');
    await nav('#account?name=capped&tab=settings');
    await type('[data-testid=cap-input]', '5');
    await click('[data-testid=cap-save]');
    await waitFor(() => (JSON.parse(fs.readFileSync(path.join(data, 'profiles.json'), 'utf8')).profiles.find((x) => x.name === 'capped') || {}).daily_cap_usd === 5, 'cap saved');
    await nav('#home');
    await hasText('[data-testid=limits]', '$5.00 daily cap');
    await hasText('[data-testid=limits]', 'resets');
    await exec("document.querySelector('[data-testid=limits]').scrollIntoView()");
    await shot('10b-overview-cap');
    hrm('remove', 'capped');
  });

  await test('MCP servers: add, write to tools, edit as JSON, search, and import from apps', async () => {
    const claudeMcp = () => JSON.parse(fs.readFileSync(path.join(home, '.claude.json'), 'utf8')).mcpServers || {};
    await nav('#mcp');
    await $('[data-testid=mcp-row-pre]');
    assert.ok(await exec(`return document.querySelector('[data-testid=t-mcp-pre-claude]').checked`), 'read from .claude.json at launch, ticked for Claude Code');
    await nav('#mcp');
    await click('[data-testid=mcp-add]');
    await $('[data-testid=mcp-form]');
    await type('[data-testid=mc-name]', 'fs');
    await type('[data-testid=mc-cmd]', 'npx');
    const args = ['-y', '@modelcontextprotocol/server-filesystem', 'C:\\work'];
    for (let i = 0; i < args.length; i++) { await click('[data-testid=mc-add-arg]'); await type(`[data-testid=mc-arg-${i}]`, args[i]); }
    await click('[data-testid=mc-add-env]');
    await type('[data-kv=env][data-p="0"]', 'ROOT');
    await type('[data-kv=env][data-p="1"]', 'C:\\work');
    assert.equal(await exec(`return document.querySelector('[data-kv=env][data-p="1"]').type`), 'password', 'values hidden until asked');
    // a new server is written to Claude Code unless unticked
    assert.ok(await exec(`return document.querySelector('[data-testid=mc-to-claude]').checked`));
    await click('[data-testid=mc-save]');
    await waitFor(() => JSON.stringify(claudeMcp().fs) === JSON.stringify({ type: 'stdio', command: 'npx', args, env: { ROOT: 'C:\\work' } }), 'fs in .claude.json, with its arguments and env');
    await waitFor(async () => (await exec('return location.hash')) === '#mcp', 'back on the list');
    await click('[data-testid=t-mcp-fs-codex]');
    await waitFor(() => fs.readFileSync(path.join(home, '.codex', 'config.toml'), 'utf8').includes('[mcp_servers.fs]'), 'fs in config.toml');
    await shot('11-mcp');
    assert.equal(await exec(`const t = document.querySelector('[data-testid=mcp-matrix]'); return t.tHead.rows[0].cells[document.querySelector('[data-testid=t-mcp-fs-codex]').closest('td').cellIndex].title`), 'Codex: 1 servers', 'the Codex count follows the tick');
    await click('[data-testid=t-mcp-fs-codex]');
    await waitFor(() => !fs.readFileSync(path.join(home, '.codex', 'config.toml'), 'utf8').includes('[mcp_servers.fs]'), 'fs removed from config.toml');
    // edit as JSON: the form's server comes out as Claude Code writes it, and goes back in
    await click('[data-testid=mcp-edit-fs]');
    await $('[data-testid=mcp-form]');
    await click('[data-testid=mc-mode-json]');
    await $('[data-testid=mc-json]');
    const json = JSON.parse(await exec(`return document.querySelector('[data-testid=mc-json]').value`));
    assert.deepEqual(json.args, args);
    await page.fill('[data-testid=mc-json]', JSON.stringify({ ...json, args: ['-y', '@modelcontextprotocol/server-filesystem', 'D:\\data'] }));
    await shot('11b-mcp-json');
    await click('[data-testid=mc-save]');
    await waitFor(() => claudeMcp().fs && claudeMcp().fs.args[2] === 'D:\\data', 'edited in .claude.json');
    // a command with no arguments: the page still draws
    await click('[data-testid=mcp-add]');
    await type('[data-testid=mc-name]', 'solo');
    await type('[data-testid=mc-cmd]', 'mcp-solo');
    await click('[data-testid=mc-save]');
    await $('[data-testid=t-mcp-solo-claude]');
    // the top bar's search narrows the list as you type
    await type('[data-testid=find]', 'solo');
    await waitFor(async () => exec(`return document.querySelector('[data-testid=mcp-row-fs]').hidden && !document.querySelector('[data-testid=mcp-row-solo]').hidden`), 'only solo shown');
    await type('[data-testid=find]', '');
    // import from apps reads the tools again; the info dialog says which file of each tool holds a server
    await click('[data-testid=mcp-import]');
    await hasText('#toast', 'No new MCP servers found');
    await click('[data-testid=mcp-info-fs]');
    await hasText('[data-testid=mcp-info]', '.claude.json');
    await hasText('[data-testid=mcp-info]', 'config.toml');
    await hasText('[data-testid=mcp-info]', 'written');
    await shot('11c-mcp-info');
    await click('.dlg .dh [data-act=dialog-close]');
    // thirty servers more in Claude Code's file: the list pages at 25
    const cj = JSON.parse(fs.readFileSync(path.join(home, '.claude.json'), 'utf8'));
    for (let i = 0; i < 30; i++) cj.mcpServers[`bulk-${String(i).padStart(2, '0')}`] = { type: 'stdio', command: 'mcp-bulk' };
    fs.writeFileSync(path.join(home, '.claude.json'), JSON.stringify(cj));
    await click('[data-testid=mcp-import]');
    await $('[data-testid=mcp-page-1]');
    const onPage = () => exec("return document.querySelectorAll('[data-testid=mcp-matrix] tbody tr[data-search]:not([hidden]):not(.paged)').length");
    assert.equal(await onPage(), 25, '25 on the first page');
    await click('[data-testid=mcp-page-1]');
    await waitFor(async () => (await onPage()) === 8, 'the rest on the second page');
    await click('[data-testid=mcp-per-50]');
    await waitFor(async () => !(await exec("return !!document.querySelector('[data-testid=mcp-page-1]')")), 'one page at 50');
  });

  await test('skills: install from a folder and link into Codex', async () => {
    await nav('#skills');
    await click('[data-testid=skill-install]');
    await type('[data-testid=sk-src]', path.join(home, 'my-skill'));
    await click('[data-testid=sk-save]');
    await click('[data-testid=t-skill-my-skill-codex]');
    await waitFor(() => fs.existsSync(path.join(home, '.codex', 'skills', 'my-skill', 'SKILL.md')), 'skill linked into codex');
    await shot('12-skills');
    // the column header: every skill shown, into OpenCode in one go, then out again
    await click('[data-testid=skill-bulk-opencode]');
    await waitFor(() => fs.existsSync(path.join(home, '.config', 'opencode', 'skills', 'my-skill', 'SKILL.md')), 'every skill linked into OpenCode');
    await waitFor(async () => exec("return document.querySelector('[data-testid=t-skill-my-skill-opencode]').checked"), 'the row shows the tick');
    await click('[data-testid=skill-bulk-opencode]');
    await waitFor(() => !fs.existsSync(path.join(home, '.config', 'opencode', 'skills', 'my-skill', 'SKILL.md')), 'and unlinked again');
  });

  await test('skills: folders in the store that the list lacks show as Not managed, and Manage adds them', async () => {
    write(path.join(home, '.agents', 'skills', 'loose', 'SKILL.md'), '---\nname: loose\ndescription: Put here by hand\n---\n');
    await nav('#home');
    await nav('#skills');
    await $('.unmcard [data-testid=skill-row-loose]');
    await hasText('.unmcard', 'Not managed');
    await hasText('[data-testid=skill-row-loose]', 'Put here by hand');
    await click('[data-testid=skill-adopt-loose]');
    await waitFor(async () => (await exec("return !document.querySelector('.unmcard [data-testid=skill-row-loose]') && !!document.querySelector('[data-testid=skill-row-loose]')")), 'managed now');
    await click('[data-testid=t-skill-loose-codex]');
    await waitFor(() => fs.existsSync(path.join(home, '.codex', 'skills', 'loose', 'SKILL.md')), 'and linked into Codex');
    // Claude Code already has its own copy of loose: the sync leaves it alone and offers to replace it with the link
    write(path.join(home, '.claude', 'skills', 'loose', 'SKILL.md'), 'an older copy, kept by hand');
    await click('[data-testid=t-skill-loose-claude]');
    await $('[data-testid=notice-act]');
    await click('[data-testid=notice-act]');
    await click('#confirm-ok');
    await waitFor(() => fs.existsSync(path.join(home, '.claude', 'skills', '.hrm-backup', 'loose', 'SKILL.md')), 'the copy moved into .hrm-backup');
    await waitFor(() => fs.lstatSync(path.join(home, '.claude', 'skills', 'loose')).isSymbolicLink(), 'and the shared copy is linked there');
    // thirty more folders managed at once: the list pages at 25, and the search narrows across pages
    for (let i = 0; i < 30; i++) write(path.join(home, '.agents', 'skills', `bulk-${String(i).padStart(2, '0')}`, 'SKILL.md'), `---
name: bulk-${i}
description: Bulk skill ${i}
---
`);
    await nav('#home');
    await nav('#skills');
    await click('[data-testid=skill-adopt-all]');
    await $('[data-testid=sk-page-1]');
    assert.equal(await exec("return document.querySelectorAll('table.sklist tbody tr[data-search]:not([hidden]):not(.paged)').length"), 25, '25 on the first page');
    await click('[data-testid=sk-page-1]');
    await waitFor(async () => (await exec("return document.querySelectorAll('table.sklist tbody tr[data-search]:not([hidden]):not(.paged)').length")) < 25, 'the rest on the second page');
    await click('[data-testid=sk-per-50]');
    await waitFor(async () => !(await exec("return !!document.querySelector('[data-testid=sk-page-1]')")), 'one page at 50');
    await type('[data-testid=find]', 'bulk-07');
    await waitFor(async () => (await exec("return document.querySelectorAll('table.sklist tbody tr[data-search]:not([hidden]):not(.paged)').length")) === 1, 'the search narrows to one');
    await page.fill('[data-testid=find]', '');
    await exec(`document.getElementById('find').dispatchEvent(new Event('input'))`);
  });

  await test('skills: Discover lists the skills of each repository and installs one into the store', async () => {
    await nav('#skills');
    await click('[data-testid=sk-tab-discover]');
    await hasText('[data-testid=discover]', 'Says hello in every language');
    await hasText('[data-testid=discover]', 'Reviews a change line by line');
    // three default repositories, one download each
    assert.equal(codeloadHits.filter((u) => u.includes('/zip/')).length, 3, 'one download per repository: ' + codeloadHits);
    await type('[data-testid=dc-search]', 'review');
    await waitFor(async () => exec(`return document.querySelector('[data-testid=dc-hello]').hidden`), 'search narrows the list');
    await page.fill('[data-testid=dc-search]', '');
    await exec(`document.querySelector('[data-testid=dc-search]').dispatchEvent(new Event('input'))`);
    await click('[data-testid=dc-install-hello]');
    await waitFor(() => fs.existsSync(path.join(home, '.agents', 'skills', 'hello', 'SKILL.md')), 'installed into the store');
    await waitFor(() => fs.existsSync(path.join(home, '.claude', 'skills', 'hello', 'SKILL.md')), 'and linked into Claude Code');
    await shot('12b-skills-discover');
    // Installed shows it with its source and description; a source filter and search narrow the list
    await click('[data-testid=sk-tab-installed]');
    await hasText('[data-testid=skill-row-hello]', 'anthropics/skills');
    await hasText('[data-testid=skill-row-hello]', 'Says hello in every language');
    await select('#sk-filter', 'Local');
    await waitFor(async () => exec(`return document.querySelector('[data-testid=skill-row-hello]').hidden && !document.querySelector('[data-testid=skill-row-my-skill]').hidden`), 'filtered to local skills');
    await select('#sk-filter', '');
    // a newer version at the source shows Update, which installs it
    write(path.join(repoDir, 'skills', 'hello', 'SKILL.md'), '---\nname: hello\ndescription: Says hello, now louder\n---\nHELLO.');
    execFileSync('pwsh', ['-NoProfile', '-Command', `Compress-Archive -Path '${repoDir}' -DestinationPath '${repoZip}' -Force`]);
    await click('[data-testid=skill-check]');
    await click('[data-testid=skill-update-hello]');
    await waitFor(() => fs.readFileSync(path.join(home, '.agents', 'skills', 'hello', 'SKILL.md'), 'utf8').includes('HELLO.'), 'updated in the store');
    await hasText('[data-testid=skill-row-hello]', 'now louder');
  });

  await test('apps: each tool with its path and version, and Update runs its installer in a terminal', async () => {
    // Claude Code as npm installs it: <prefix>\claude.exe beside <prefix>\node_modules\@anthropic-ai\claude-code
    write(path.join(bin, 'node_modules', '@anthropic-ai', 'claude-code', 'package.json'), JSON.stringify({ name: '@anthropic-ai/claude-code', version: '2.1.300' }));
    await nav('#apps');
    await hasText('[data-testid=app-claude]', '2.1.300');
    await hasText('[data-testid=app-claude]', path.join(bin, 'claude.exe'));
    // a tool outside npm: its version comes from --version; how each was installed is read from where it sits
    await hasText('[data-testid=app-codex]', '1.0.0');
    await hasText('[data-testid=app-kind-claude]', 'npm');
    await hasText('[data-testid=app-kind-codex]', 'manual');
    await shot('16-apps');
    await click('[data-testid=app-update-claude]');
    await waitFor(() => readLines(termLog).some((t) => t.shell && t.shell.startsWith('npm install -g @anthropic-ai/claude-code@latest')), 'the npm update runs in a terminal');
  });

  await test('prompts: write a prompt into the tool file, keeping the old content', async () => {
    write(path.join(home, '.codex', 'AGENTS.md'), 'hand-written instructions');
    await nav('#prompts');
    await click('.tabs2 a[data-v=codex]');
    await $('.tabs2 a.on[data-v=codex]');
    await click('[data-testid=prompt-new]');
    await type('#pr-body', 'Be brief and cite files.');
    await click('[data-testid=prompt-enable]');
    await waitFor(() => fs.readFileSync(path.join(home, '.codex', 'AGENTS.md'), 'utf8') === 'Be brief and cite files.', 'AGENTS.md written');
    assert.ok(JSON.parse(hrm('prompts', 'list', '--json')).some((p) => p.body === 'hand-written instructions'), 'old content saved first');
    await shot('13-prompts');
  });

  await test('refresh says what it did, and why Claude limits stay old while Live is off', async () => {
    await nav('#accounts');
    await click('[data-act=refresh-now]');
    await waitFor(async () => /new record|up to date/.test(await exec(`return document.getElementById('toast').textContent`)), 'refresh reported');
    // a logged-in Claude account whose saved limits are 7 hours old, with Live off
    const msg = await exec(`return import('./core.js').then((m) => {
      const keep = m.S.state;
      m.S.state = { live_quota: false, profiles: [{ harness: 'claude', kind: 'subscription', identity: { logged_in: true }, quota: { fetched_at: Date.now() - 7 * 3600e3 } }] };
      const live = m.refreshText({ new_rows: 1, quota_errors: [] });
      m.S.state.live_quota = true;
      const on = m.refreshText({ new_rows: 0, quota_errors: [] });
      m.S.state = keep;
      return [live, on];
    })`);
    assert.ok(msg[0].startsWith('1 new record.') && msg[0].includes('Turn on Live Claude limits'), msg[0]);
    assert.equal(msg[1], 'Usage is up to date.');
  });

  await test('backups: back up, lose a skill folder, restore it; export to a file; the interval saves on change', async () => {
    await nav('#settings');
    await click('[data-testid=backup-now]');
    await $('[data-testid=backup-Manual]');
    const sk = path.join(home, '.agents', 'skills', 'my-skill');
    assert.ok(fs.existsSync(path.join(sk, 'SKILL.md')));
    fs.rmSync(sk, { recursive: true, force: true });
    await click('[data-testid=restore-Manual]');
    await click('#confirm-ok');
    await waitFor(() => fs.existsSync(path.join(sk, 'SKILL.md')), 'the skill folder is back');
    await $('[data-testid="backup-Before restore"]');
    const out = path.join(sb, 'exported-backup.zip');
    await exec('window.__TAURI__.dialog.save = async () => arguments[0]', [out]);
    await click('[data-testid=backup-export]');
    await waitFor(() => fs.existsSync(out) && fs.statSync(out).size > 200, 'exported');
    await select('[data-testid=st-bkhours]', '6');
    await waitFor(() => JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).backup_hours === 6, 'the interval saved');
    await shot('14b-backups');
  });

  await test('appearance: dark theme, limits as used, and a hidden tool left out of the pages', async () => {
    await nav('#settings');
    await click('[data-testid=st-theme] a[data-v=dark]');
    await waitFor(async () => (await exec(`return getComputedStyle(document.body).backgroundColor`)) === 'rgb(10, 15, 19)', 'the page turns near-black');
    await shot('17-dark-settings');
    await nav('#accounts');
    await $('[data-testid=accounts-table]');
    await shot('17b-dark-accounts');
    await nav('#home');
    await $('[data-testid=limits]');
    await sleep(600);
    await shot('17c-dark-overview');
    await exec("document.querySelector('[data-testid=routes]').scrollIntoView()");
    await shot('17d-dark-overview-routes');
    await exec("document.querySelector('[data-testid=activity]').scrollIntoView()");
    await shot('17e-dark-overview-panels');
    // every other page in dark, to compare with the dark design pages
    for (const [h, n] of [['#projects', 'projects'], ['#sessions', 'sessions'], ['#session?profile=claude-main&id=s-main-1', 'session'], ['#account?name=claude-main', 'account'],
      ['#routing', 'routing'], ['#mcp', 'mcp'], ['#skills', 'skills'], ['#prompts', 'prompts'], ['#apps', 'apps'], ['#cli', 'cli']]) {
      const before = await exec("return document.querySelector('#sheet h1')?.textContent || ''");
      await nav(h);
      await waitFor(async () => (await exec("return document.querySelector('#sheet h1')?.textContent || ''")) !== before, n + ' drawn');
      await sleep(500);
      await shot('18-dark-' + n);
      // the design's measurements on every page: page padding, title, and every card's corners
      const m = await exec(`const cs = (e) => getComputedStyle(e), sh = cs(document.getElementById('sheet')), h1 = cs(document.querySelector('#sheet h1'));
        const cards = [...document.querySelectorAll('#sheet .panel, #sheet .acg, #sheet .grp, #sheet .tcard, #sheet .apgroup, #sheet .mcpmain, #sheet .sset, #sheet .plist, #sheet .peditor')];
        return { gap: sh.rowGap, pad: [sh.paddingTop, sh.paddingLeft], h1: [h1.fontSize, h1.fontWeight], cards: cards.length, radius: [...new Set(cards.map((c) => cs(c).borderTopLeftRadius))] };`);
      assert.deepEqual(m.pad, ['28px', '40px'], n + ': page padding 28 / 40');
      assert.equal(m.gap, n === 'session' ? '20px' : '24px', n + ': the page parts sit 24 px apart (a session 20 px), as drawn');
      assert.deepEqual(m.h1, [n === 'session' ? '30px' : '34px', '600'], n + ': title as drawn (34 px, a session 30 px), 600');
      assert.ok(m.cards > 0, n + ': drawn in cards');
      assert.deepEqual(m.radius, ['16px'], n + ': card corners 16 px');
      // card padding, table rows and type as the design pages set them
      const want = {
        sessions: [['.grp .gh', 'padding', '14px 20px'], ['.srow', 'padding', '12px 20px'], ['.grp .gh h2.gname', 'fontSize', '18px']],
        session: [['.panel', 'padding', '18px 20px']], account: [['.panel', 'padding', '18px 20px']],
        routing: [['.panel', 'padding', '18px 20px']], apps: [['.panel', 'padding', '18px 20px']],
        mcp: [['.mcpmain', 'padding', '0px'], ['.mcpmain th', 'padding', '12px 6px 12px 20px'], ['.mcpmain .mn b', 'font', '15px/700']],
        skills: [['.tcard:has(> .sklist)', 'padding', '0px'], ['.sklist td', 'padding', '14px 6px 14px 20px'], ['.sklist .mn b', 'fontWeight', '700']],
        prompts: [['.pgrid > div:last-child', 'padding', '18px 20px']], cli: [['.cliex', 'padding', '16px 20px'], ['.cligrid .panel', 'padding', '16px 20px']],
      }[n] || [];
      for (const [sel, prop, v] of want) {
        const got = await exec(`const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return 'missing'; const c = getComputedStyle(e);
          return ${JSON.stringify(prop)} === 'font' ? c.fontSize + '/' + c.fontWeight : c[${JSON.stringify(prop)}];`);
        assert.equal(got, v, `${n}: ${sel} ${prop} as drawn`);
      }
    }
    await nav('#settings');
    await $('[data-testid=sec-appearance]');
    assert.equal(await exec("return getComputedStyle(document.querySelector('.sset')).padding"), '0px 20px 4px', 'settings sections: padding as drawn');
    assert.equal(await exec("return getComputedStyle(document.querySelector('.sset')).maxWidth"), '960px', 'settings: at most a 1040 px column (960 inside its padding), as drawn');
    await click('[data-testid=st-theme] a[data-v=light]');
    await waitFor(async () => (await exec(`return getComputedStyle(document.body).backgroundColor`)) === 'rgb(245, 247, 250)', 'light again');
    await click('[data-testid=st-quota] a[data-v=used]');
    await waitFor(() => JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).quota_display === 'used', 'saved');
    await nav('#accounts');
    await hasText('[data-testid=row-codex-main]', '% used');
    await nav('#settings');
    await click('[data-testid=st-quota] a[data-v=left]');
    await waitFor(() => JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).quota_display === 'left', 'back to left');
    // Apps > Show off: the tool leaves Accounts and MCP servers
    await nav('#apps');
    await click('[data-testid=app-show-opencode]');
    await waitFor(() => (JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).hidden_tools || []).includes('opencode'), 'hidden');
    await nav('#accounts');
    await $('[data-testid=group-claude]');
    assert.ok(!(await exec(`return !!document.querySelector('[data-testid=group-opencode]')`)), 'no OpenCode card');
    await nav('#mcp');
    await $('[data-testid=mcp-matrix]');
    assert.ok(!(await exec(`return !!document.querySelector('[data-h=opencode]')`)), 'no OpenCode column');
    await nav('#apps');
    await click('[data-testid=app-show-opencode]');
    await waitFor(() => !(JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).hidden_tools || []).includes('opencode'), 'shown again');
  });

  if (!process.env.HRM_E2E_APP_DIR) await test('a new Harness Manager: found at launch, offered in Settings > Updates', async () => {
    // the check at launch found it: the dot on the Settings link, and the offer without a click
    await waitFor(async () => exec("return !document.getElementById('settings-upd').hidden"), 'the dot on the Settings link');
    await nav('#settings');
    await $('[data-testid=self-install]');
    await hasText('.sheet', 'Version 9.9.9 is available');
    await hasText('.sheet', 'Faster limits.');
    // the offer is out of date (the release is the version running, as after an update): installing says so and withdraws it
    latestVersion = await exec("return document.querySelector('.sheet').textContent.match(/you have (\\d+\\.\\d+\\.\\d+)/)[1]");
    await click('[data-testid=self-install]');
    await hasText('#toast', `Harness Manager ${latestVersion} is the newest version`);
    await $('[data-testid=self-check]');
    assert.ok(await exec("return document.getElementById('settings-upd').hidden"), 'the dot is gone');
    latestVersion = '9.9.9';
    await click('[data-testid=self-check]');
    await $('[data-testid=self-install]');
  });

  await test('settings: change the terminal', async () => {
    await nav('#settings');
    await click('[data-testid=st-term] a[data-v=console]');
    await waitFor(() => JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).terminal === 'console', 'settings saved');
    await waitFor(async () => (await exec("return document.querySelector('[data-testid=st-term] a.on')?.dataset.v")) === 'console', 'drawn with Console picked');
    await shot('14-settings');
    // switches save at once
    const saved = () => JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8'));
    await exec("document.querySelector('[data-testid=st-tray]').dataset.old = '1'");
    await click('[data-testid=st-tray]');
    await waitFor(() => saved().close_to_tray === false, 'close to tray off');
    // the page draws again after a save: click the new switch, not the old one (the old one reads unchecked too)
    await waitFor(async () => (await exec(`const e = document.querySelector('[data-testid=st-tray]'); return !!e && !e.dataset.old && !e.checked`)), 'redrawn');
    await click('[data-testid=st-tray]');
    await waitFor(() => saved().close_to_tray === true, 'and on again');
    // Launch at sign-in: the Run entry starts the app in the tray
    const runKey = `HKCU\\Software\\HarnessManagerTest-${process.pid}\\Run`;
    const entry = () => { try { return execFileSync('reg', ['query', runKey, '/v', 'HarnessManager'], { encoding: 'utf8' }); } catch { return ''; } };
    await click('[data-testid=st-autostart]');
    await waitFor(() => entry().includes('--tray'), 'sign-in entry written');
    await waitFor(async () => (await exec(`return document.querySelector('[data-testid=st-autostart]').checked`)) === true, 'redrawn with it on');
    await click('[data-testid=st-autostart]');
    await waitFor(() => !entry(), 'and removed');
  });

  await test('a walkthrough on a page\'s first visit, and setting tooltips with their defaults', async () => {
    const tour = () => exec(`return document.querySelector('[data-testid=tour]')?.innerText || ''`);
    await exec(`localStorage.removeItem('hm.tour.accounts')`);
    await nav('#home');
    await nav('#accounts');
    await waitFor(async () => (await tour()).includes('1 of'), 'the tour starts on the first visit');
    assert.ok((await tour()).includes('Add an account'));
    assert.ok(await exec(`return !!document.querySelector('.tour-spot')`), 'the part is highlighted');
    await click('[data-testid=tour-next]');
    await waitFor(async () => (await tour()).includes('2 of'), 'Next moves on');
    await shot('15-tour');
    await page.keyboard.press('Escape');
    await waitFor(async () => !(await tour()), 'Esc ends it');
    await nav('#home');
    await nav('#accounts');
    await $('[data-testid=accounts-table]');
    assert.ok(!(await tour()), 'an ended tour stays ended');
    await exec(`localStorage.removeItem('hm.tour.routing')`);
    await nav('#routing');
    await $('[data-testid=provider-add]');
    await sleep(300);
    assert.ok(!(await tour()), 'skipping one page\'s tour skips every page\'s');
    await nav('#accounts');
    await $('[data-testid=accounts-table]');
    await click('[data-testid=tour-start]');
    await waitFor(async () => (await tour()).includes('1 of'), 'the ? button replays it');
    await click('[data-testid=tour-skip]');
    await nav('#settings');
    await page.hover('[data-testid=tip-refresh]');
    await waitFor(async () => (await exec(`return getComputedStyle(document.getElementById('tip-refresh')).display`)) === 'block', 'tooltip shown');
    await hasText('#tip-refresh', 'Default: 5');
    await shot('16-settings-tooltip');
    await click('[data-testid=guides-reset]');
    await waitFor(async () => (await tour()).includes('1 of'), 'Show the tours again starts the tour of this page');
    await click('[data-testid=tour-skip]');
    await exec(`for (const v of arguments[0]) localStorage.setItem('hm.tour.' + v, '1')`, [TOURED]);
  });

  await test('rename an account: shown everywhere by its new name, the id stays', async () => {
    await nav('#account?name=team-ci&tab=settings');
    await type('[data-testid=rename-input]', 'Team CI bot');
    await click('[data-testid=rename-save]');
    await hasText('[data-testid=detail-name]', 'Team CI bot');
    await nav('#accounts');
    await hasText('[data-testid=row-team-ci]', 'Team CI bot');
    const p = JSON.parse(hrm('list', '--json')).find((x) => x.name === 'team-ci');
    assert.equal(p.label, 'Team CI bot', 'the id stays team-ci');
  });

  await test('remove an account and its folder', async () => {
    await nav('#account?name=team-ci&tab=settings');
    await click('[data-testid=remove-account]');
    await click('[data-testid=remove-home]');
    await click('[data-testid=remove-confirm]');
    await waitFor(async () => (await exec(`return location.hash`)) === '#accounts', 'back on accounts');
    await waitFor(async () => !(await exec(`return !!document.querySelector('[data-testid=row-team-ci]')`)), 'row gone');
    assert.ok(fs.existsSync(path.join(home, '.claude', 'skills', 'pdf', 'SKILL.md')), 'main profile untouched');
  });

  await test('remove accounts from the list: keep the folder, and a main profile keeps everything', async () => {
    const codexHome = JSON.parse(hrm('show', 'codex-personal', '--json')).home;
    await nav('#accounts');
    await click('[data-testid=menu-codex-personal]');
    await click('.menu a.danger');
    await $('[data-testid=remove-confirm]');
    await click('[data-testid=remove-confirm]');
    await waitFor(async () => !(await exec(`return !!document.querySelector('[data-testid=row-codex-personal]')`)), 'row gone');
    assert.ok(fs.existsSync(codexHome), 'its folder stays unless asked');
    assert.ok(!JSON.parse(hrm('list', '--json')).some((p) => p.name === 'codex-personal'));
    // a main profile: no "delete its folder" choice, and the tool's own folder stays
    await click('[data-testid=menu-codex-main]');
    await click('.menu a.danger');
    await $('[data-testid=remove-confirm]');
    assert.ok(!(await exec(`return !!document.querySelector('[data-testid=remove-home]')`)), 'no folder delete for a main profile');
    await click('[data-testid=remove-confirm]');
    await waitFor(async () => !(await exec(`return !!document.querySelector('[data-testid=row-codex-main]')`)), 'main row gone');
    assert.ok(fs.existsSync(path.join(home, '.codex')), 'the tool keeps its own folder');
  });

  // last: it hides the window
  await test('closing the window keeps the app running in the tray', async () => {
    const win = (call) => exec(`return window.__TAURI__.window.getCurrentWindow().${call}`);
    assert.equal(await win('isVisible()'), true);
    await win('close()'); // what the title bar's X does
    await sleep(1500);
    assert.equal(app.exitCode, null, 'still running');
    assert.equal(await win('isVisible()'), false, 'hidden, not closed');
    await win('show()'); // what the tray icon does
    await waitFor(async () => (await win('isVisible()')) === true, 'shown again');
  });

  // ---- across computers (WP-4) ----
  // The screens for several computers. The sync core (computers_status, computers_now, computers_set; scope and the
  // per-computer fields of home, sessions, projects and live) is built in parallel: without it every page draws the
  // "sync off" state, which the first tests check. HRM_E2E_SYNC=1 runs the rest against a seeded sync folder.
  await test('computers: the Settings section in the off state, the chooser, no chip, and the Overview as before', async () => {
    await nav('#settings');
    await $('[data-testid=sec-computers] [data-testid=sync-folder]');
    const d = await exec(`return {
      name: document.getElementById('st-devname').value,
      folderOn: document.querySelector('[data-testid=sync-folder]').classList.contains('on'), folderText: document.querySelector('[data-testid=sync-folder-path]').textContent,
      hub: document.querySelector('[data-testid=sync-hub]').innerText, hubOff: document.querySelector('[data-testid=sync-hub] input').disabled && document.querySelector('[data-testid=sync-hub] button').disabled,
      direct: document.querySelector('[data-testid=st-direct]').disabled, pair: document.querySelector('[data-testid=pair]').disabled,
      transcripts: document.querySelector('[data-testid=st-transcripts]').checked, presence: document.querySelector('[data-testid=st-presence]').checked,
      pills: [...document.querySelectorAll('[data-testid=computers] .pill')].map((p) => p.textContent.trim()),
      cards: [...document.querySelectorAll('[data-testid=computer-cards] .dev b')].map((b) => b.textContent.trim()),
      syncNow: !!document.querySelector('[data-testid=computers-now]'), chip: document.getElementById('pcs').hidden,
      order: [...document.querySelectorAll('.sset h2 .fold')].map((h) => h.firstChild.textContent.trim()) }`);
    assert.ok(d.name, 'this computer has a name');
    assert.equal(d.folderOn, false, 'sync is off: the folder card is not chosen');
    assert.ok(d.folderText.includes('No folder'), 'no folder yet');
    assert.ok(d.hub.includes('A hub you host') && d.hubOff, 'the hub card waits for phase 5');
    assert.ok(!d.direct && !d.pair, 'the direct connection and pairing are live (phase 2)');
    assert.ok(d.pills.includes('phase 5') && !d.pills.includes('phase 2') && d.pills.includes('this one'), d.pills.join(','));
    assert.ok(d.transcripts && d.presence, 'transcripts and presence default to on');
    assert.deepEqual(d.cards, [d.name], 'only this computer');
    assert.equal(d.syncNow, false, 'no Sync now while sync is off');
    assert.equal(d.chip, true, 'no computers chip while sync is off');
    assert.deepEqual(d.order.slice(-2), ['Computers', 'Backups and archive'], 'the section sits above Backups');
    await exec("document.querySelector('[data-testid=sec-computers]').scrollIntoView()");
    await shot('19-settings-computers');
    // the tooltip of the section's first setting, like every other row
    await page.hover('[data-testid=tip-devname]');
    await waitFor(async () => (await exec(`return getComputedStyle(document.getElementById('tip-devname')).display`)) === 'block', 'tooltip shown');
    // with sync off the other pages are as before: no scope switch, no Computer column, no dots
    await nav('#home');
    await $('[data-testid=limits]');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=scope], [data-testid=activity-pc], [data-testid=running-now], .ovk .split, .tn .cdots')")), 'the Overview is as before');
    await nav('#sessions');
    await $('[data-testid=group]');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=pc-seg], .srow .pc, .srow.withpc')")), 'Sessions as before');
    await nav('#projects');
    await $('[data-testid=project-cards] .pcrow');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=pcard-pcs]')")), 'Projects as before');
  });

  await test('computers: the All computers · This computer choice is remembered on this computer, All by default', async () => {
    const r = await exec(`return import('./computers.js').then((m) => { localStorage.removeItem('hm.scope'); const a = m.scope(); m.setScope('here'); const b = m.scope(); const c = localStorage.getItem('hm.scope'); m.setScope('all'); return [a, b, c, m.scope()]; })`);
    assert.deepEqual(r, ['all', 'here', 'here', 'all']);
  });

  if (process.env.HRM_E2E_SYNC) await test('computers: a second computer in the sync folder reaches Settings, the chip, the Overview, Sessions and Projects', async () => {
    const { createHash } = await import('node:crypto');
    // the other computer's folder: its record, one journal chunk (two usage rows and a session), its manifest and presence
    const sync = path.join(sb, 'sync'), dev = 'e2e-laptop-0000', inc = 'inc-e2e-1', devDir = path.join(sync, 'devices', dev);
    const key = 'github.com/example/platform-gitops', cwd = 'C:\\laptop\\Projects\\platform-gitops', t0 = Date.now() - 3600000;
    const usage = (n) => ({ request_id: `claude:laptop-s1:m${n}`, profile: 'claude-main', harness: 'claude', ts: t0 + n * 60000, model: 'claude-opus-5-5', project: cwd, session_id: 'laptop-s1', input: 1000, output: 200, cache_read: 500, cache_write: 0, ttft_ms: 900, gen_ms: 4000, cost_usd: 0.0081, price_rev: 'e2e' });
    const session = { harness: 'claude', profile: 'claude-main', id: 'laptop-s1', title: 'Rotate the staging certificates', cwd, started_at: t0, last_active_at: t0 + 120000, prompts: 2, tool_calls: 2, failed: 0, messages: 4, model: 'claude-opus-5-5', tokens: 3400, source_path: '', project_key: key, project_name: 'platform-gitops', open: false, tags: [] };
    const chunk = jl([{ seq: 1, ts: t0, op: 'usage', key: usage(1).request_id, data: usage(1) }, { seq: 2, ts: t0 + 60000, op: 'usage', key: usage(2).request_id, data: usage(2) },
      { seq: 3, ts: t0 + 120000, op: 'session', key: 'claude/claude-main/laptop-s1', data: session }]);
    const file = `${inc}-1-3.jsonl`;
    write(path.join(devDir, 'journal', file), chunk);
    write(path.join(devDir, 'device.json'), JSON.stringify({ id: dev, name: 'LAPTOP', os: 'windows', app_version: '1.3.2', incarnation: inc, roots: { [key]: cwd }, updated_at: Date.now() }));
    write(path.join(devDir, 'manifest.json'), JSON.stringify({ incarnation: inc, last_seq: 3, chunks: [{ file, from: 1, to: 3, sha256: createHash('sha256').update(chunk).digest('hex'), bytes: Buffer.byteLength(chunk) }] }));
    write(path.join(devDir, 'presence.json'), JSON.stringify({ at: Date.now(), app_version: '1.3.2', running: [{ profile: 'claude-main', harness: 'claude', project: key, project_name: 'platform-gitops', session_id: 'laptop-s2', cwd, started_at: Date.now() - 600000 }], accounts: { 'claude-main': { tokens_5m: 5000, requests_5m: 4 } }, tps_5m: 16 }));
    // the folder is chosen (what the Change… button does after the picker), then Sync now runs a round
    await exec("return window.__TAURI__.core.invoke('call', { op: 'computers_set', args: { sync_mode: 'folder', sync_folder: arguments[0] } })", [sync]);
    assert.equal(JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).sync_mode, 'folder', 'saved in settings');
    await nav('#settings');
    await $('[data-testid=sync-folder].on');
    await click('[data-testid=computers-now]');
    await waitFor(async () => (await text('[data-testid=computer-cards]')).includes('LAPTOP'), 'the other computer listed');
    const cards = await exec(`return [...document.querySelectorAll('[data-testid=computer-cards] .dev')].map((d) => d.innerText.replace(/\\s+/g, ' '))`);
    assert.equal(cards.length, 2);
    assert.ok(/LAPTOP.*(windows|Windows).*1\.3\.2.*(heard|writing)/.test(cards[1]), cards[1]);
    assert.ok(fs.existsSync(path.join(sync, 'devices')) && fs.readdirSync(path.join(sync, 'devices')).length === 2, 'this computer wrote its own record beside the other');
    await exec("document.querySelector('[data-testid=sec-computers]').scrollIntoView()");
    await shot('19b-settings-two-computers');
    // the chip: a dot per computer, both heard from lately
    const chip = await exec(`const el = document.getElementById('pcs'); return { hidden: el.hidden, dots: el.querySelectorAll('.cdot').length, on: el.querySelectorAll('.cdot.on').length, text: el.textContent }`);
    assert.equal(chip.hidden, false, 'the chip shows with sync on');
    assert.equal(chip.dots, 2);
    assert.equal(chip.on, 2, 'both heard within three minutes');
    assert.ok(chip.text.includes('LAPTOP') && chip.text.includes('synced'), chip.text);
    // the Overview: the switch, the split on the tiles, the Activity switch, the account's dots and Running now
    await nav('#home');
    await $('[data-testid=scope]');
    await waitFor(async () => (await exec("return !!document.querySelector('[data-testid=kpi-tokens] .split')")), 'the tokens tile split by computer');
    assert.ok((await text('[data-testid=kpi-tokens] .kn')).includes('%'), 'the largest computer named in the note');
    assert.ok((await text('[data-testid=activity-pc]')).includes('LAPTOP'), 'the Activity grid has a switch per computer');
    await $('[data-testid=routes] [data-node="a:claude-main"] .cdots');
    await hasText('[data-testid=running-now]', 'LAPTOP');
    await shot('19c-overview-all-computers');
    await click('[data-testid=activity-pc-' + dev + ']');
    await hasText('[data-testid=activity] .ph', 'LAPTOP');
    await click('[data-testid=scope-here]');
    await waitFor(async () => (await exec("return document.querySelector('[data-testid=scope] a.on')?.dataset.v")) === 'here', 'This computer picked');
    assert.equal(await exec("return localStorage.getItem('hm.scope')"), 'here', 'remembered');
    await shot('19d-overview-this-computer');
    await click('[data-testid=scope-all]');
    // Sessions: the Computer column, the filter, and the session open on the other computer
    await nav('#sessions?days=all');
    await $('[data-testid=session-laptop-s1] .pc');
    assert.ok((await text('[data-testid=session-laptop-s1] .pc')).includes('LAPTOP'));
    assert.ok((await text('[data-testid=session-s-main-1] .pc')).trim().length > 0, 'this computer named on its own session');
    await click('[data-testid=pc-' + dev + ']');
    await waitFor(async () => !(await exec("return !!document.querySelector('[data-testid=session-s-main-1]')")) && (await exec("return !!document.querySelector('[data-testid=session-laptop-s1]')")), 'only the other computer\'s sessions');
    await shot('19e-sessions-computer');
    await click('[data-testid=pc-here]');
    await waitFor(async () => (await exec("return !!document.querySelector('[data-testid=session-s-main-1]')")) && !(await exec("return !!document.querySelector('[data-testid=session-laptop-s1]')")), 'only this computer\'s sessions');
    await click('[data-testid=pc-all]');
    await $('[data-testid=session-laptop-s1]');
    // Projects: the repository both computers have carries two dots; the counts sum both
    await nav('#projects?days=all');
    const card = () => exec("return [...document.querySelectorAll('[data-testid^=pcard-]')].find((c) => c.innerText.includes('platform-gitops'))?.querySelector('[data-testid=pcard-pcs]')?.outerHTML || ''");
    await waitFor(card, 'the repository card with its computers');
    assert.equal((await card()).match(/<i class="cdot /g).length, 2, 'a dot per computer that has the repository: ' + (await card()));
    assert.ok(!(await card()).includes('not cloned'), 'this computer has it');
    await shot('19f-projects-computers');
  });

  // ---- Pi and Oh My Pi (WP-1) ----
  await test('Pi and Oh My Pi: seeded sessions appear in the window and in hrm sessions and hrm usage', async () => {
    const t = Date.now();
    const iso = (ms) => new Date(ms).toISOString();
    const usage = (i, o, cr) => ({ input: i, output: o, cacheRead: cr, cacheWrite: 0, totalTokens: i + o + cr, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } });
    const reply = (content, i, o, cr, began, model) => ({ role: 'assistant', content, api: 'anthropic-messages', provider: 'anthropic', model, usage: usage(i, o, cr), stopReason: 'stop', timestamp: began });
    // Pi names the folder after the path with / \ : replaced by -, wrapped in --
    const piDir = path.join(home, '.pi', 'agent', 'sessions', `--${repo.replace(/^[/\\]/, '').replace(/[/\\:]/g, '-')}--`);
    write(path.join(piDir, `${iso(t).replace(/[:.]/g, '-')}_pi-e2e-1.jsonl`), jl([
      { type: 'session', version: 3, id: 'pi-e2e-1', timestamp: iso(t), cwd: repo },
      { type: 'message', id: 'u1', parentId: null, timestamp: iso(t + 1000), message: { role: 'user', content: 'Rotate the Longhorn backup credentials', timestamp: t + 1000 } },
      { type: 'message', id: 'a1', parentId: 'u1', timestamp: iso(t + 4000), message: reply([{ type: 'toolCall', id: 'c1', name: 'bash', arguments: { command: 'kubectl -n longhorn get secret' } }], 700, 50, 100, t + 2000, 'claude-opus-5-5') },
      { type: 'message', id: 'r1', parentId: 'a1', timestamp: iso(t + 4100), message: { role: 'toolResult', toolCallId: 'c1', toolName: 'bash', content: [{ type: 'text', text: 'minio-creds   Opaque' }], isError: false, timestamp: t + 4100 } },
      { type: 'message', id: 'a2', parentId: 'r1', timestamp: iso(t + 6000), message: reply([{ type: 'text', text: 'Rotated.' }], 800, 20, 700, t + 5000, 'claude-opus-5-5') },
    ]));
    // Oh My Pi: a folder under home is `-<relative>`, and a 256-byte title slot comes before the header
    const slot = (pad) => JSON.stringify({ type: 'title', v: 1, title: 'Pin the ArgoCD image', updatedAt: iso(t), pad });
    const titleLine = slot(' '.repeat(255 - slot('').length)) + '\n';
    assert.equal(Buffer.byteLength(titleLine), 256);
    const ompDir = path.join(home, '.omp', 'agent', 'sessions', `-${path.relative(home, repo).replace(/[/\\:]/g, '-')}`);
    write(path.join(ompDir, `${iso(t).replace(/[:.]/g, '-')}_omp-e2e-1.jsonl`), titleLine + jl([
      { type: 'session', version: 3, id: 'omp-e2e-1', timestamp: iso(t), cwd: repo },
      { type: 'message', id: 'u1', parentId: null, timestamp: iso(t + 1000), message: { role: 'user', content: 'Pin the ArgoCD image to a digest', timestamp: t + 1000 } },
      { type: 'message', id: 'a1', parentId: 'u1', timestamp: iso(t + 3000), message: reply([{ type: 'text', text: 'Pinned.' }], 400, 30, 0, t + 1500, 'gpt-6') },
      { type: 'model_usage', id: 'm1', parentId: 'a1', timestamp: iso(t + 3500), purpose: 'title', api: 'openai-completions', provider: 'openai', model: 'gpt-6', usage: usage(100, 8, 0), stopReason: 'stop' },
    ]));
    // the tools' homes exist now: init adds their main profiles, as the first run does for installed tools
    hrm('init');
    const hs = hrm('harnesses');
    assert.ok(hs.includes('pi ') && hs.includes('Oh My Pi'), `hrm harnesses lists both:\n${hs}`);
    const names = JSON.parse(hrm('list', '--json')).map((p) => p.name);
    assert.ok(names.includes('pi-main') && names.includes('omp-main'), `main profiles added: ${names}`);
    const u = JSON.parse(hrm('usage', '--json'));
    assert.equal(u.by_profile['pi-main'].reduce((a, b) => a + b, 0), 1570, 'Pi: input + output of both replies');
    assert.equal(u.by_profile['omp-main'].reduce((a, b) => a + b, 0), 538, 'Oh My Pi: the reply and the title call');
    // the app rescans sessions on its own schedule; the listing is read until both are there (briefly)
    let sessions = '';
    for (let i = 0; i < 20 && !(sessions.includes('pi-e2e-1') && sessions.includes('omp-e2e-1')); i++) { if (i) await sleep(500); sessions = hrm('sessions'); }
    const logFile = path.join(data, 'logs', 'harness-manager.log');
    const diag = () => `${sessions} | accounts now: ${JSON.parse(hrm('list', '--json')).map((p) => p.name)} | log: ${fs.existsSync(logFile) ? fs.readFileSync(logFile, 'utf8').trim().split('\n').slice(-6).join(' | ') : '-'}`.replace(/\s+/g, ' ');
    assert.ok(sessions.includes('Rotate the Longhorn backup credentials') && sessions.includes('pi-e2e-1'), diag());
    assert.ok(sessions.includes('Pin the ArgoCD image') && sessions.includes('omp-e2e-1'), diag());
    await nav('#sessions');
    await $('[data-testid=session-pi-e2e-1]');
    await hasText('[data-testid=session-pi-e2e-1]', 'Rotate the Longhorn');
    await hasText('[data-testid=session-omp-e2e-1]', 'Pin the ArgoCD image');
    await nav('#accounts');
    await $('[data-testid=row-pi-main]');
    await $('[data-testid=row-omp-main]');
    await shot('40-pi-omp');
  });

  // ---- direct connection (WP-6) ----
  // Phase 2's operations (computers_pair_start | cancel | join, computers_forget, computers_set_peer, the new fields of
  // computers_status and the computers-pairing event) are built in parallel; without them the dialog shows their
  // "unknown operation" answer as its error text, which the first test checks. HRM_E2E_PEERS=1 pairs with a second
  // data folder through hrm computers pair, as the contract's Window test describes.
  await test('direct connection: the switch without its pill, the dialog with both tabs, the code input, and an unknown operation shown as the error text', async () => {
    await nav('#settings');
    await $('[data-testid=sec-computers] [data-testid=pair]');
    const d = await exec(`return { direct: document.querySelector('[data-testid=st-direct]').disabled, pair: document.querySelector('[data-testid=pair]').disabled,
      pills: [...document.querySelectorAll('[data-testid=computers] .pill')].map((p) => p.textContent.trim()), port: document.querySelector('[data-testid=listen-port]').textContent, change: !!document.querySelector('[data-testid=listen-port-change]') }`);
    assert.ok(!d.direct && !d.pair, 'the switch and Pair are live');
    assert.ok(!d.pills.includes('phase 2'), d.pills.join(','));
    assert.ok(d.port && d.change, 'the listen port and Change… sit in the popover');
    // the port: Change… in the popover opens a small dialog; 0 means a free port
    await page.hover('[data-testid=tip-direct]');
    await click('[data-testid=listen-port-change]');
    await $('[data-testid=lp-v]');
    await page.fill('[data-testid=lp-v]', '0');
    await click('[data-testid=lp-ok]');
    await waitFor(async () => !(await exec("return document.getElementById('dialog-root').innerHTML")), 'the port dialog closed');
    await $('[data-testid=sec-computers] [data-testid=pair]');
    await click('[data-testid=pair]');
    await $('[data-testid=pair-tabs]');
    assert.deepEqual(await exec("return [...document.querySelectorAll('[data-testid=pair-tabs] a')].map((a) => a.textContent)"), ['Show a code', 'Enter a code']);
    // Show a code asks computers_pair_start: six digits, three and three
    await waitFor(async () => /\d{3} \d{3}/.test(await text('[data-testid=pair-show-body]')), 'a code in the dialog');
    await shot('41-pair-show-code');
    await click('[data-testid=pair-enter]');
    await $('[data-testid=pair-code-in]');
    const code = async (t) => { await page.fill('[data-testid=pair-code-in]', ''); await page.type('[data-testid=pair-code-in]', t); return exec("return document.getElementById('pair-code-in').value"); };
    assert.equal(await code('4829157'), '482 915', 'six digits at most, a space after three');
    assert.equal(await code('ab12c'), '12', 'digits only');
    // too short: no call; six digits: the operation's own error text
    await click('[data-testid=pair-join]');
    await hasText('[data-testid=pair-error]', 'six digits');
    await code('482915');
    await page.fill('[data-testid=pair-host]', 'laptop:7777');
    await click('[data-testid=pair-join]');
    await waitFor(async () => { const t = (await exec("return document.querySelector('[data-testid=pair-error]')?.textContent || ''")).trim(); return t.length > 0 && !t.includes('six digits'); }, "the operation's own error text (no computer is pairing at laptop:7777)", 60000);
    assert.equal(await exec("return document.getElementById('pair-join').disabled"), false, 'Pair can be tried again');
    await shot('41b-pair-enter-code');
    await page.keyboard.press('Escape');
    await waitFor(async () => !(await exec("return document.getElementById('dialog-root').innerHTML")), 'the dialog closed');
  });

  if (process.env.HRM_E2E_PEERS) await test('direct connection: pairing with a second computer run by hrm computers pair, the card direct and connected, its session after Sync now, Forget', async () => {
    // the second computer: its own data folder and profiles home, the same fake tools, the direct connection on
    const data2 = path.join(sb, 'data2'), home2 = path.join(sb, 'home2');
    fs.mkdirSync(home2, { recursive: true });
    write(path.join(data2, 'settings.json'), JSON.stringify({ theme: 'light', device_name: 'LAPTOP', direct_connection: true }));
    const env2 = { ...env, HRM_HOME: data2, HRM_PROFILES_HOME: home2 };
    const hrm2 = (...args) => execFileSync(path.join(appDir, 'hrm.exe'), args, { env: env2, encoding: 'utf8' });
    // a session in the second computer's Claude profile, indexed there so its journal carries it
    const cwd2 = 'C:\\laptop\\Projects\\platform-gitops', t = new Date().toISOString();
    write(path.join(home2, '.claude', 'projects', 'p', 'laptop-s9.jsonl'), jl([
      { type: 'user', sessionId: 'laptop-s9', cwd: cwd2, timestamp: t, message: { role: 'user', content: 'Rotate the staging certificates' } },
      { type: 'assistant', sessionId: 'laptop-s9', cwd: cwd2, timestamp: t, message: { id: 'm9', model: 'claude-opus-5-5', usage: { input_tokens: 500, output_tokens: 60 }, content: [{ type: 'text', text: 'Rotated.' }] } },
      { type: 'ai-title', aiTitle: 'Rotate the staging certificates', sessionId: 'laptop-s9' },
    ]));
    hrm2('init');
    hrm2('sessions');
    const id2 = JSON.parse(fs.readFileSync(path.join(data2, 'settings.json'), 'utf8')).device_id;
    assert.ok(id2, 'the second computer has an id');
    // the second computer shows a code and waits up to two minutes; its stdout gives the code and the host hint
    const pair = spawn(path.join(appDir, 'hrm.exe'), ['computers', 'pair'], { env: env2, stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '';
    pair.stdout.on('data', (b) => { out += b; });
    pair.stderr.on('data', (b) => { out += b; });
    const m = await waitFor(() => out.match(/\b(\d{3}) ?(\d{3})\b/), 'the code on stdout: ' + out, 30000);
    const code = m[1] + m[2], host = (out.match(/[\w.-]+:\d{2,5}/) || [])[0];
    assert.ok(host, 'the host hint on stdout: ' + out);
    let serve;
    try {
      // this computer: the switch on, then Enter a code with the code and the host
      await nav('#settings');
      await $('[data-testid=st-direct]');
      if (!(await exec("return document.querySelector('[data-testid=st-direct]').checked"))) {
        await click('[data-testid=st-direct]');
        await waitFor(() => exec("return document.querySelector('[data-testid=st-direct]')?.checked"), 'the direct connection on');
      }
      await waitFor(async () => exec("return !!document.querySelector('[data-testid=pair]')"), 'the Pair button; the section reads: ' + (await exec("return (document.querySelector('[data-testid=sec-computers]') || { innerText: '(no section)' }).innerText.slice(0, 1500)")), 15000);
      await click('[data-testid=pair]');
      await click('[data-testid=pair-enter]');
      await page.type('[data-testid=pair-code-in]', code);
      await page.fill('[data-testid=pair-host]', host);
      await click('[data-testid=pair-join]');
      await waitFor(async () => !(await exec("return document.getElementById('dialog-root').innerHTML")), 'the dialog closed after pairing (page: ' + (await exec("return document.getElementById('dialog-root').innerText").catch(() => '')) + ')', 30000);
      // pair ends by printing this computer's name; then the second computer serves (listener and mDNS) for the rest
      await waitFor(() => pair.exitCode !== null, 'hrm computers pair ended', 30000);
      const myName = JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).device_name;
      assert.ok(out.includes(myName), `pair printed this computer's name (${myName}): ${out}`);
      serve = spawn(path.join(appDir, 'hrm.exe'), ['computers', 'serve', '--seconds', '150'], { env: env2, stdio: 'ignore' });
      const card = `[data-testid=computer-${id2}]`;
      await $(card);
      await waitFor(async () => /direct .* connected/.test(await text(card + ' [data-testid=transport]').catch(() => '')), 'the card shows direct and connected', 45000);
      await exec("document.querySelector('[data-testid=sec-computers]').scrollIntoView()");
      await shot('42-paired-connected');
      const chip = await exec(`const el = document.getElementById('pcs'); return { hidden: el.hidden, on: el.querySelectorAll('.cdot.on').length }`);
      assert.ok(!chip.hidden && chip.on >= 2, 'the chip: this computer and the paired one on (an earlier test may have left a folder peer): ' + JSON.stringify(chip));
      // the second computer's session arrives after Sync now
      await click('[data-testid=computers-now]');
      await nav('#sessions?days=all');
      await waitFor(() => exec("return !!document.querySelector('[data-testid=session-laptop-s9]')"), 'the session from the second computer', 30000);
      await hasText('[data-testid=session-laptop-s9] .pc', 'LAPTOP');
      await shot('42b-sessions-from-peer');
      // Forget: a confirm, then the card is gone
      await nav('#settings');
      await click(`[data-testid=forget-${id2}]`);
      await click('#confirm-ok');
      await waitFor(async () => !(await exec(`return !!document.querySelector('[data-testid=computer-${id2}]')`)), 'the card removed', 15000);
      await shot('42c-forgotten');
    } finally { if (serve) serve.kill(); if (pair.exitCode === null) pair.kill(); }
  });

  // ---- resume here (WP-7) ----
  // A session from another computer, seeded in the sync folder with its compressed transcript: the row offers
  // Resume here when its repository and account are here (and says why not otherwise), reading it fetches the
  // transcript, and Resume here launches the tool on a copy in this computer's clone, as a fork.
  await test('resume here: a remote session is read from the folder and resumed as a fork in the local clone', async () => {
    const { createHash } = await import('node:crypto');
    const sync = path.join(sb, 'sync-resume'), dev = 'e2e-laptop-7777', inc = 'inc-e2e-7', devDir = path.join(sync, 'devices', dev);
    const key = 'github.com/example/platform-gitops', cwd = 'C:\laptop\Projects\platform-gitops', t0 = Date.now() - 7200000, iso = new Date(t0).toISOString();
    // the transcript as Claude Code wrote it on the laptop, compressed as the folder carries it
    const log = jl([
      { type: 'user', sessionId: 'laptop-s7', cwd, timestamp: iso, message: { role: 'user', content: 'Rotate the staging certificates on the laptop' } },
      { type: 'assistant', sessionId: 'laptop-s7', cwd, timestamp: iso, message: { id: 'l7', model: 'claude-opus-5-5', usage: { input_tokens: 100, output_tokens: 20 }, content: [{ type: 'text', text: 'Certificates rotated on the laptop.' }] } },
    ]);
    const packed = zlib.zstdCompressSync(Buffer.from(log));
    const file = 'transcripts/claude/claude-main/laptop-s7.jsonl.zst';
    write(path.join(devDir, file), packed);
    const session = (id, title, pkey, pname) => ({ harness: 'claude', profile: 'claude-main', id, title, cwd, started_at: t0, last_active_at: t0 + 60000, prompts: 1, tool_calls: 0, failed: 0, messages: 2, model: 'claude-opus-5-5', tokens: 120, source_path: `C:\laptop\.claude\projects\C--laptop-Projects-platform-gitops\${id}.jsonl`, project_key: pkey, project_name: pname, open: false, tags: [] });
    const chunk = jl([
      { seq: 1, ts: t0, op: 'session', key: 'claude/claude-main/laptop-s7', data: session('laptop-s7', 'Rotate the staging certificates (laptop)', key, 'platform-gitops') },
      { seq: 2, ts: t0, op: 'session', key: 'claude/claude-main/laptop-s8', data: session('laptop-s8', 'A project only the laptop has', 'github.com/example/elsewhere', 'elsewhere') },
      { seq: 3, ts: t0 + 1, op: 'transcript', key: 'claude/claude-main/laptop-s7', data: { file, bytes: packed.length, sha256: createHash('sha256').update(packed).digest('hex'), mtime: t0 + 60000, source_rel: 'projects/C--laptop-Projects-platform-gitops/laptop-s7.jsonl' } },
    ]);
    const chunkFile = `${inc}-1-3.jsonl`;
    write(path.join(devDir, 'journal', chunkFile), chunk);
    write(path.join(devDir, 'device.json'), JSON.stringify({ id: dev, name: 'LAPTOP', os: 'windows', app_version: '1.4.0', incarnation: inc, roots: { [key]: cwd }, updated_at: Date.now() }));
    write(path.join(devDir, 'manifest.json'), JSON.stringify({ incarnation: inc, last_seq: 3, chunks: [{ file: chunkFile, from: 1, to: 3, sha256: createHash('sha256').update(chunk).digest('hex'), bytes: Buffer.byteLength(chunk) }] }));
    // the folder is chosen: a round merges the laptop's journal
    await exec("return window.__TAURI__.core.invoke('call', { op: 'computers_set', args: { sync_mode: 'folder', sync_folder: arguments[0] } })", [sync]);
    await nav('#sessions?days=all');
    await $('[data-testid=resume-here-laptop-s7]');
    await hasText('[data-testid=reason-laptop-s8]', 'not cloned here');
    await shot('41-sessions-resume-here');
    // reading it: fetched from the folder, shown through Claude Code's parser
    await click('[data-testid=read-laptop-s7]');
    await hasText('[data-testid=session-title]', 'Rotate the staging certificates (laptop)');
    await hasText('[data-testid=chat] .msg.ai', 'Certificates rotated on the laptop.');
    await hasText('[data-testid=session-from]', 'LAPTOP');
    assert.ok(fs.existsSync(path.join(data, 'transcripts', dev, 'claude', 'claude-main', 'laptop-s7.jsonl')), 'the fetched copy sits in the data folder');
    await shot('42-session-remote');
    // Resume here: the dialog names the local clone, then the fake claude gets the copy's path and --fork-session, in the clone
    await nav('#sessions?days=all');
    await click('[data-testid=resume-here-laptop-s7]');
    await hasText('[data-testid=resume-here-root]', 'platform-gitops');
    await hasText('[data-testid=resume-here-dialog]', 'the LAPTOP copy stays as it is');
    await shot('43-resume-here-dialog');
    await click('[data-testid=resume-here-ok]');
    const launch = await waitFor(() => readLines(termLog).find((t) => t.args && t.args[3] === '--resume' && String(t.args[4]).endsWith('laptop-s7.jsonl')), 'the fork launched');
    assert.deepEqual(launch.args.slice(0, 3).concat(launch.args.slice(5)), ['run', 'claude-main', '--', '--fork-session'], JSON.stringify(launch));
    assert.equal(launch.cwd, repo, 'opened in this computer\'s clone');
    assert.ok(fs.existsSync(launch.args[4]) && launch.args[4].startsWith(path.join(home, '.claude', 'projects')), 'the copy is under this computer\'s Claude Code folder: ' + launch.args[4]);
    // the copy is a session of this computer now, marked as continued from the laptop
    await nav('#sessions?days=all');
    await hasText('[data-testid=continued-laptop-s7]', 'continued from LAPTOP');
    // Settings: the Carry transcripts popover counts what this computer carried
    await nav('#settings');
    await $('[data-testid=sec-computers] [data-testid=st-transcripts]');
    await hasText('#tip-transcripts', 'Carried');
    await hasText('#tip-transcripts', '20 MB');
  });

  // ---- backup, archive and welcome back (WP-8) ----
  // Backups hold the history and a restore merges it; old transcripts move to the archive while the session stays
  // listed and readable; a sync folder with history offers Welcome back when it is chosen.
  await test('backups and archive: the section, hrm archive now on an old session, Restore… from a backup, Welcome back', async () => {
    const { createHash } = await import('node:crypto');
    await nav('#settings');
    await $('[data-testid=sec-backups-and-archive] [data-testid=archive-now]');
    const d = await exec(`return { title: document.querySelector('[data-testid=sec-backups-and-archive] h2 .fold').firstChild.textContent.trim(),
      folder: document.querySelector('[data-testid=st-bk-folder]').checked, transcripts: document.querySelector('[data-testid=st-bk-transcripts]').checked,
      days: document.querySelector('[data-testid=st-archive-days] a.on')?.textContent, sync: document.querySelector('[data-testid=st-archive-sync]').checked,
      dir: document.querySelector('[data-testid=st-archive-dir]').textContent, restore: !!document.querySelector('[data-testid=restore-open]') }`);
    assert.equal(d.title, 'Backups and archive');
    assert.ok(d.folder && !d.transcripts, 'backups go to the sync folder; transcripts stay out by default');
    assert.equal(d.days, '30 days');
    assert.ok(d.sync && d.dir.includes('No folder') && d.restore, JSON.stringify(d));
    await exec("document.querySelector('[data-testid=sec-backups-and-archive]').scrollIntoView()");
    await shot('44-settings-backups-archive');
    // an old session, three days old: a one-day age and hrm archive now move it into the chosen folder (and the sync folder)
    const oldAt = new Date(Date.now() - 3 * 86400000), old = oldAt.toISOString(), month = `${oldAt.getFullYear()}-${String(oldAt.getMonth() + 1).padStart(2, '0')}`;
    write(path.join(home, '.claude', 'projects', 'p', 's-old-1.jsonl'), jl([
      { type: 'user', sessionId: 's-old-1', cwd: repo, timestamp: old, message: { role: 'user', content: 'Tidy the old Longhorn manifests' } },
      { type: 'assistant', sessionId: 's-old-1', cwd: repo, timestamp: old, message: { id: 'o1', model: 'claude-opus-5-5', usage: { input_tokens: 50, output_tokens: 20 }, content: [{ type: 'text', text: 'The old manifests are tidy.' }] } },
    ]));
    const archiveDir = path.join(sb, 'archive');
    hrm('settings', 'archive_days', '1');
    hrm('settings', 'archive_dir', archiveDir);
    const out = hrm('archive', 'now');
    assert.ok(/^[01] sessions? moved/.test(out), out); // the app's own daily step may have moved it first
    assert.ok(!fs.existsSync(path.join(home, '.claude', 'projects', 'p', 's-old-1.jsonl')), 'the original left the tool\'s folder');
    assert.ok(fs.existsSync(path.join(home, '.claude', 'projects', 'p', 's-main-1.jsonl')), 'the fresh session stays');
    const copies = fs.readdirSync(path.join(archiveDir, 'platform-gitops', month));
    assert.ok(copies.includes('claude-claude-main-s-old-1.jsonl.zst') && copies.includes('claude-claude-main-s-old-1.json'), copies.join(','));
    assert.equal(zlib.zstdDecompressSync(fs.readFileSync(path.join(archiveDir, 'platform-gitops', month, 'claude-claude-main-s-old-1.jsonl.zst'))).toString().split('\n')[0].includes('Tidy the old'), true);
    const st = hrm('archive', 'status');
    assert.ok(st.startsWith('1 archived'), st);
    // the row keeps its place with the pill, reads from the archive, and offers no Resume
    await nav('#sessions?days=all');
    await hasText('[data-testid=archived-s-old-1]', 'archived');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=session-s-old-1] [data-act=resume]')")), 'no Resume on an archived session');
    await shot('45-sessions-archived');
    await click('[data-testid=read-s-old-1]');
    await hasText('[data-testid=chat] .msg.ai', 'The old manifests are tidy.');
    await hasText('[data-testid=resume-reason]', 'archived');
    await shot('46-session-archived');
    // Restore… from a backup made now: the preview counts this computer's history and names it; merging adds nothing
    await nav('#settings');
    await click('[data-testid=backup-now]');
    await $('[data-testid=backup-Manual]');
    await click('[data-testid=restore-open]');
    await click('[data-testid=restore-src-Manual]');
    await hasText('[data-testid=restore-counts]', 'archived transcripts');
    const counts = (await text('[data-testid=restore-counts]')).replace(/,/g, '');
    assert.ok(+((counts.match(/(\d+) requests/) || [])[1] || 0) > 0 && /\b1 archived/.test(counts), counts);
    const before = JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8'));
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=welcome-adopt]')")), 'its own backup: this computer is in it as itself, nothing to adopt');
    assert.equal((await text('[data-testid=welcome-new]')).trim(), 'Restore');
    await shot('47-restore-dialog');
    await click('[data-testid=welcome-new]');
    await hasText('#toast', 'Restored: 0 requests');
    const after = JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8'));
    assert.equal(after.device_id, before.device_id, 'its own backup: the id stays');
    // Welcome back: a sync folder with another computer's journal (seeded as the phase 1 test does), chosen
    const sync = path.join(sb, 'sync-welcome'), dev = 'e2e-desk-8888', inc = 'inc-e2e-8', devDir = path.join(sync, 'devices', dev);
    const t0 = Date.now() - 86400000, key = 'github.com/example/platform-gitops', cwd = 'C:\\desk\\Projects\\platform-gitops';
    const usage = (n) => ({ request_id: `claude:desk-s1:m${n}`, profile: 'claude-main', harness: 'claude', ts: t0 + n * 60000, model: 'claude-opus-5-5', project: cwd, session_id: 'desk-s1', input: 1000, output: 200, cache_read: 500, cache_write: 0, ttft_ms: 900, gen_ms: 4000, cost_usd: 0.0081, price_rev: 'e2e' });
    const session = { harness: 'claude', profile: 'claude-main', id: 'desk-s1', title: 'Rotate the desk certificates', cwd, started_at: t0, last_active_at: t0 + 120000, prompts: 2, tool_calls: 2, failed: 0, messages: 4, model: 'claude-opus-5-5', tokens: 3400, source_path: '', project_key: key, project_name: 'platform-gitops', open: false, tags: [] };
    const chunk = jl([{ seq: 1, ts: t0, op: 'usage', key: usage(1).request_id, data: usage(1) }, { seq: 2, ts: t0 + 60000, op: 'usage', key: usage(2).request_id, data: usage(2) },
      { seq: 3, ts: t0 + 120000, op: 'session', key: 'claude/claude-main/desk-s1', data: session }]);
    const file = `${inc}-1-3.jsonl`;
    write(path.join(devDir, 'journal', file), chunk);
    write(path.join(devDir, 'device.json'), JSON.stringify({ id: dev, name: 'DESK', os: 'windows', app_version: '1.5.0', incarnation: inc, roots: { [key]: cwd }, updated_at: Date.now() }));
    write(path.join(devDir, 'manifest.json'), JSON.stringify({ incarnation: inc, last_seq: 3, chunks: [{ file, from: 1, to: 3, sha256: createHash('sha256').update(chunk).digest('hex'), bytes: Buffer.byteLength(chunk) }] }));
    // what the chooser does after the picker: the folder is set, then Welcome back asks
    await exec("return window.__TAURI__.core.invoke('call', { op: 'computers_set', args: { sync_mode: 'folder', sync_folder: arguments[0] } })", [sync]);
    await exec("import('./restore.js').then((m) => m.welcomeBack(arguments[0]))", [sync]);
    await hasText('[data-testid=welcome-back]', 'DESK');
    await hasText('[data-testid=restore-computers]', 'DESK · 2 requests · 1 sessions');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=welcome-adopt]')")), 'no record with this computer\'s name: no adopt');
    await shot('48-welcome-back');
    await click('[data-testid=welcome-new]');
    await waitFor(async () => !(await exec("return !!document.querySelector('[data-testid=welcome-back]')")), 'the dialog closed');
    await nav('#sessions?days=all');
    await hasText('[data-testid=session-desk-s1] .pc', 'DESK');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=session-desk-s1] [data-act=resume]')")), 'a past computer\'s session is not resumed here as a local one');
  });

  exitCode = results.some((r) => r[1] !== 'ok') ? 1 : 0;
} catch (e) {
  console.error(e);
} finally {
  if (browser) await browser.close().catch(() => {});
  app.kill();
  codeload.close();
  try { execFileSync('reg', ['delete', `HKCU\\Software\\HarnessManagerTest-${process.pid}`, '/f'], { stdio: 'ignore' }); } catch {}
  try { execFileSync(path.join(appDir, 'hrm.exe'), ['router', 'stop'], { env }); } catch {}
  // remove test credentials
  try { execFileSync('pwsh', ['-NoProfile', '-Command', `cmdkey /list | Select-String 'target=(${prefix}[^ ]*)' | % { cmdkey /delete:$($_.Matches[0].Groups[1].Value) } | Out-Null`]); } catch {}
  // the sandbox (fake tools, databases, logs) goes too; retries cover the app releasing its files
  try { fs.rmSync(sb, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 }); } catch {}
  const ok = results.filter((r) => r[1] === 'ok').length;
  console.log(`\n${ok} passed, ${results.length - ok} failed; screenshots in ${shots}`);
  process.exit(exitCode);
}
