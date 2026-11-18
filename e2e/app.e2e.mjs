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
import { designTests } from './design.e2e.mjs';

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
/** A double click: on a row of Sessions or Accounts it opens the item's full page (a click selects it for the pane). */
const dbl = async (css) => { await $(css); await page.dblclick(css); };
const type = async (css, t) => {
  await $(css);
  const enter = t.endsWith('\n');
  await page.fill(css, enter ? t.slice(0, -1) : t);
  if (enter) await page.press(css, 'Enter');
};
const text = async (css) => { await $(css); return page.innerText(css); };
const hasText = (css, t) => waitFor(async () => (await text(css)).includes(t), `"${t}" in ${css}`);
const nav = async (hash) => { await exec(`location.hash = arguments[0]`, [hash]); await sleep(400); };
/** The page at the address is the one drawn (its data read, its markup in place). */
const drawn = (what = 'the page') => waitFor(() => exec("return import('./core.js').then((c) => c.S.drawn === location.hash)"), what + ' drawn');
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
  await waitFor(() => exec(`return !!document.querySelector('nav.nav a')`), 'the window drawn');
  await exec(`localStorage.setItem('hm.firstrun', '1'); document.querySelector('[data-testid=firstrun-later]')?.click();`);

  await test('overview: the head with its readouts, the deck, the accounts runway, the year clock, reply timing and the request log', async () => {
    await $('[data-testid=heatmap]');
    assert.equal(await exec("return document.querySelectorAll('[data-testid=heatmap] .yc-cell').length"), 371, '53 weeks of days on the year clock');
    // the head: beside the range, three readouts for it (tokens, cost at list prices, the share read from cache); no readouts panel
    await hasText('[data-testid=kpi-cost]', '$0.01');
    await hasText('[data-testid=kpi-tokens]', 'tokens');
    await hasText('[data-testid=kpi-hit]', 'cached');
    assert.ok(await exec("return !!document.querySelector('.page-head .actions [data-testid=kpi-tokens]') && !!document.querySelector('.page-head .actions [data-testid=range-today]') && !document.querySelector('[aria-label=\"Period readouts\"]')"), 'the range and its readouts sit in the head');
    // the deck, in the drawing's order: the account to use next with Start and Start in folder…, what needs attention, what runs now
    assert.deepEqual(await exec("return [...document.querySelectorAll('.ov-deck > section h2')].map((h) => h.textContent)"), ['Use next', 'Needs attention', 'Running now'], 'three cards, in this order');
    await hasText('[data-testid=use-next]', 'codex-main');
    await $('[data-testid=use-next] [data-testid=start-best]');
    await click('[data-testid=start-folder]');
    await $('[data-testid=sd-dir]');
    await click('.dlg .dh [data-act=dialog-close]');
    await $('[data-testid=running-now]');
    await $('[data-testid=attention]');
    // the runway: one row per account; its limits as bars of what is left, one lane on a linear seven days, one verdict
    await $('[data-testid=lane-codex-main]');
    assert.equal(await exec("return document.querySelector('[data-testid=lane-codex-main] .ov-lim b').textContent"), '79%', 'the bar says what is left');
    await hasText('[data-testid=lane-codex-main] .ov-lim', 'Wk');
    await hasText('[data-testid=verdict-codex-main]', 'Lasts until');
    assert.ok(await exec("const l = document.querySelector('[data-testid=lane-codex-main] .ov-lane'); return !!l.querySelector('.fill') && !!l.querySelector('.tick') && !l.querySelector('.starve')"), 'its lane is filled up to the reset\'s tick');
    assert.deepEqual(await exec("return [...document.querySelectorAll('.ov-axis span')].map((t) => t.textContent)"), ['Now', '+7 d'], 'a linear 7-day lane');
    // the latest replies carry no tokens-per-second; Request log opens the whole log in a dialog, every number column right-aligned
    await hasText('[data-testid=latest-replies]', 'claude-opus-5-5');
    assert.ok(!(await exec("return document.querySelector('[data-testid=latest-replies] thead').textContent.includes('Speed')")), 'no speed column on the Overview');
    await click('[data-testid=request-log-open]');
    await $('[data-testid=request-log] tbody tr');
    const off = await exec(`const t = document.querySelector('[data-testid=request-log]'); const r = t.querySelector('tbody tr');
      return [...t.querySelectorAll('thead th.num')].map((th) => Math.abs(th.getBoundingClientRect().right - r.children[th.cellIndex].getBoundingClientRect().right));`);
    assert.ok(off.length === 6 && off.every((d) => d < 1), 'number headers aligned: ' + off);
    await $('[data-testid=request-log] [data-testid=ttft]');
    await waitFor(async () => (await exec("return document.querySelectorAll('[data-testid=request-log] tbody tr').length")) === 5, 'five requests');
    await hasText('[data-testid=request-log]', 'claude-opus-5-5');
    await exec("document.querySelector('[data-act=dialog-close]').click()");
    await waitFor(async () => !(await exec("return !!document.querySelector('[data-testid=request-log]')")), 'the log closed');
    // the year clock shades today
    await waitFor(async () => (await exec("return document.querySelectorAll('[data-testid=heatmap] .yc-cell:not(.lv-0)').length")) > 0, 'today shaded');
    await shot('00-home');
    // a tool in the table's head shows that tool's rows and replies; Antigravity too (it keeps no usage logs, so its numbers are zero)
    await click('[data-testid=tool-codex]');
    await waitFor(async () => (await exec("const l = [...document.querySelectorAll('.ov-table tbody tr')]; return l.length > 0 && l.every((x) => (x.dataset.name || '').startsWith('codex'))")), 'only codex rows');
    await click('[data-testid=tool-agy]');
    await hasText('[data-testid=latest-replies]', 'No replies in this range.');
    await click('[data-testid=tool-all]');
    await $('[data-testid=lane-claude-main]');
    // no Limits panel and no rings in the rail: the rings are the tray popup's
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=limits], aside.rail [data-testid=nav-limits]')")), 'no Limits panel, no rings in the rail');
    await hasText('[data-testid=activity]', 'Activity, 53 weeks');
    // a range picks the period; the readouts follow
    await click('[data-testid=range-7d]');
    await waitFor(async () => (await exec("return document.querySelector('[data-testid=range-7d]').getAttribute('aria-pressed')")) === 'true', '7 days picked');
    await hasText('[data-testid=kpi-tokens]', 'tokens');
    await exec("document.querySelector('[data-testid=activity]').scrollIntoView()");
    await shot('00c-overview-panels');
    await click('[data-testid=range-today]');
    // the legend sits behind one small "i", drawn with the marks it explains
    await click('[data-testid=legend]');
    await waitFor(async () => (await exec("return document.querySelectorAll('.menu .ov-legend').length")) === 4, 'the legend');
    await page.keyboard.press('Escape');
    // limits as what is left or what is used: one compact choice in the table's head, saved as the setting
    await click('[data-testid=used-pick]');
    await click('[data-testid=used-used]');
    await waitFor(async () => (await exec("return document.querySelector('[data-testid=lane-codex-main] .ov-lim b').textContent")) === '21%', 'the bars say what is used');
    await click('[data-testid=used-pick]');
    await click('[data-testid=used-left]');
    await waitFor(async () => (await exec("return document.querySelector('[data-testid=lane-codex-main] .ov-lim b').textContent")) === '79%', 'and what is left again');
    // a selected row is marked and names its route under the table; Escape lets it go
    await click('[data-testid=lane-codex-main] .ov-name');
    await hasText('.route-inspector-bar', 'codex-main');
    await waitFor(() => exec("return document.querySelector('[data-testid=lane-codex-main]').classList.contains('sel')"), 'the row marked');
    // a double-click on an account renames nothing: the rename box is for a project's name only
    await page.dblclick('[data-testid=lane-codex-main] .ov-name');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=rename-input]')")), 'no rename box on an account row');
    await page.keyboard.press('Escape');
    await waitFor(async () => !(await exec("return !!document.querySelector('.ov-table tr.sel')")), 'no row marked any more');
  });

  // The drawing's sample data (docs/project-review-2026-11/mockups/BRIEF.md), answered in place of the backend's, so
  // the Overview is measured, and photographed, in the state the drawing shows: eight accounts on four tools, four
  // sessions on two computers, one tool with an update. `long` adds an account whose name, plan and limits are long.
  const sampleOn = (opts = {}) => exec(`
    const t = window.__TAURI__, real = window.__ovReal || t.core, opts = arguments[0], H = 3600e3, now = Date.now();
    window.__ovReal = real;
    const tier = (id, label, left, hours) => ({ id, label, used: 100 - left, resets_at: hours ? now + hours * H : null });
    const acct = (p, name, harness, o = {}) => ({ ...p, name, label: name, harness, kind: 'subscription', main: false, home: 'C:/Users/alex/.' + name, expected_email: null, email_mismatch: false, route: null, running: [],
      ...o, identity: { ...p.identity, logged_in: true, needs_login: false, plan: null, email: 'alex@example.com', ...(o.identity || {}) }, quota: o.tiers ? { tiers: o.tiers, fetched_at: now - 60000 } : null });
    const run = (id, dir, min) => ({ pid: 1000 + min, session_id: id, cwd: 'C:/Projects/' + dir, started_at: now - min * 60000 });
    const profiles = (p) => [
      acct(p, 'work-max', 'claude', { identity: { plan: 'Max 20x' }, tiers: [tier('session', '5-hour', 64, 2.17), tier('week', 'Week', 41, 66.9), tier('week_fable', 'Week · Fable', 35, 66.9)], running: [run('s1', 'platform-gitops', 72), run('s2', 'platform-gitops', 24)] }),
      acct(p, 'claude-main', 'claude', { main: true, identity: { plan: 'Max 5x' }, tiers: [tier('session', '5-hour', 100, 0), tier('week', 'Week', 78, 122), tier('week_fable', 'Week · Fable', 80, 122)] }),
      acct(p, 'client-acme', 'claude', { identity: { needs_login: true } }),
      acct(p, 'team-ci', 'claude', { kind: 'api_key' }),
      acct(p, 'codex-main', 'codex', { main: true, identity: { plan: 'ChatGPT Pro' }, tiers: [tier('session', '5-hour', 90, 4), tier('week', 'Week', 0, 1.03)] }),
      acct(p, 'codex-personal', 'codex', { identity: { plan: 'Plus' }, tiers: [tier('session', '5-hour', 100, 0), tier('week', 'Week', 88, 144)], running: [run('s4', 'web-console', 6)] }),
      acct(p, 'opencode-main', 'opencode', { main: true }),
      acct(p, 'agy-main', 'agy', { main: true, identity: { plan: 'Google AI Pro' }, tiers: [tier('session', '5-hour', 100, 5), tier('week', 'Week', 100, 167.99)] }),
      ...(opts.long ? [acct(p, 'agy-with-a-very-long-account-name', 'agy', { identity: { plan: 'Google AI Ultra for Business, annual' }, tiers: [tier('session', '5-hour', 70, 3), tier('week', 'Week', 60, 100),
        tier('g1', 'Week · Gemini 3 Pro High and Low thinking', 55, 100), tier('g2', 'Week · Gemini 3 Flash and Flash Lite', 90, 100), tier('g3', 'Week · Claude models through Antigravity', 20, 100)] })] : []),
    ];
    const titles = { s1: ['work-max', 'Longhorn backup target for staging', 'platform-gitops'], s2: ['work-max', 'Bump ArgoCD and fix the sync waves', 'platform-gitops'], s3: ['claude-main', 'Invoice export: totals off by one cent', 'billing-api'], s4: ['codex-personal', 'Fix the flaky build', 'web-console'] };
    const fake = {
      state: (r) => ({ ...r, computer: 'WORKSTATION', app_updates: 1, settings: { ...r.settings, hidden_tools: ['pi', 'omp'], quota_display: 'left', theme: opts.theme || r.settings.theme, brand: opts.brand || r.settings.brand, text_size: opts.size || r.settings.text_size }, profiles: profiles(r.profiles[0]) }),
      home: (r) => ({ ...r, dash: { ...r.dash, cur: { ...r.dash.cur, tokens: 48200000, requests: 10472, cost: 61.4, hit: 0.94 },
        forecast: { 'work-max|week': { runs_out: r.dash.now + 45.25 * H, at_reset: 130 }, 'work-max|week_fable': { runs_out: r.dash.now + 50 * H, at_reset: 120 } } } }),
      usage: (r) => ({ ...r, accounts: [{ key: 'team-ci', cost: 4.2, tokens: 1 }] }),
      sessions: () => ({ total: 4, groups: [{ key: 'g', sessions: Object.entries(titles).map(([id, [profile, title, project_name]]) => ({ id, profile, title, project_name, open: true })) }] }),
      computers_status: (r) => ({ ...r, mode: 'folder', this: { id: 'pc-here', name: 'WORKSTATION' }, peers: [{ id: 'pc-laptop', name: 'LAPTOP', last_seen: now - 60000, running: [{ session_id: 's3', cwd: 'C:/Projects/billing-api', profile: 'claude-main', started_at: now - 41 * 60000 }] }] }),
      apps: (r) => ({ ...r, apps: r.apps.map((a) => (a.id === 'codex' ? { ...a, path: a.path || 'codex.exe', version: '0.169.0' } : a)), latest: { at: now, versions: { codex: '0.170.0' }, errors: {} } }),
    };
    t.core = { ...real, invoke: (cmd, a, o) => real.invoke(cmd, a, o).then((r) => (a && fake[a.op] ? fake[a.op](r) : r)) };`, [opts]);
  const sampleOff = () => exec("if (window.__ovReal) { window.__TAURI__.core = window.__ovReal; window.__ovReal = null; } localStorage.removeItem('hm.usenext');");
  /** The Overview drawn from the answers in force now: the sample's, or the backend's again. (Without a visit to
   * another page: the first-run report is said on the first visit to Accounts, which a later test makes.) */
  const overview = async () => { await exec("if (location.hash !== '#home') location.hash = '#home'; return import('./core.js').then((c) => c.render())"); await drawn('the Overview'); };
  /** A click and what the page shows right after it, in one turn of the page: a choice that only changes what is
   * shown is drawn at once from what was last read, where one that reads the backend again would not be drawn yet. */
  const clickNow = (css, read) => exec(`document.querySelector(arguments[0]).click(); ${read}`, [css]);

  await test('overview: the tool switch of Use next changes the recommendation, and the choice is remembered', async () => {
    await sampleOn();
    try {
      await overview();
      // across every tool: the account with the most room; the switch offers Any and each tool that has an account, by its icon
      await hasText('[data-testid=use-name]', 'agy-main');
      assert.deepEqual(await exec("return [...document.querySelectorAll('[data-act=home-use-tool]')].map((b) => b.dataset.v + ':' + (b.querySelector('img.ticon')?.getAttribute('src') || b.textContent))"),
        [':Any', 'claude:icons/claude.svg', 'codex:icons/codex.svg', 'opencode:icons/opencode.svg', 'agy:icons/agy.svg'], 'Any, then each tool that has an account, with its official icon');
      // one tool: its own best account, with its plan, one reason, Start and one runner-up, drawn without a new read
      const c = await clickNow('[data-testid=use-claude]', `const q = (s) => document.querySelector('[data-testid=use-next] ' + s);
        return { name: q('[data-testid=use-name]').textContent, why: q('[data-testid=use-why]').textContent, runner: q('[data-testid=use-runner]').textContent, pills: [...q('.ov-who').querySelectorAll('.pill')].map((p) => p.textContent),
          start: q('[data-testid=start-best]').dataset.name, folder: q('[data-testid=start-folder]').textContent, pressed: q('[data-testid=use-claude]').getAttribute('aria-pressed'), icon: q('.ov-who img').getAttribute('src'), primary: document.querySelectorAll('#sheet .btn.primary').length };`);
      assert.equal(c.name, 'claude-main', 'the Claude Code account with the most room');
      assert.equal(c.why, '78% of the week left · lasts until its reset', 'one reason line');
      assert.ok(/^Runner-up\s*work-max\s*41% left · ≈ out \w+$/.test(c.runner), 'one runner-up line: ' + c.runner);
      assert.deepEqual(c.pills, ['Max 5x', 'main'], 'its plan, and that it is the main profile');
      assert.deepEqual([c.start, c.folder, c.pressed, c.icon, c.primary], ['claude-main', 'Start in folder…', 'true', 'icons/claude.svg', 1], 'Start is the one primary button, Start in folder… beside it');
      // the page in the drawing's state, for review beside docs/project-review-2026-11/mockups/a-overview-{light,dark}.png
      await page.mouse.move(700, 22);
      await sleep(300);
      await shot('00a-overview-sample-light');
      await sampleOn({ theme: 'dark' });
      await overview();
      await waitFor(async () => (await exec('return getComputedStyle(document.body).backgroundColor')) === 'rgb(9, 17, 21)', 'the dark theme');
      await sleep(300);
      await shot('00b-overview-sample-dark');
      // and in VektraGrid, light and dark, and at the Large text size
      for (const [name, o] of [['00d-overview-sample-vg-light', { brand: 'vg' }], ['00e-overview-sample-vg-dark', { brand: 'vg', theme: 'dark' }], ['00f-overview-sample-large', { size: 'large' }]]) {
        await sampleOn(o);
        await overview();
        await sleep(300);
        await shot(name);
      }
      await sampleOn();
      await overview();
      // another tool, and the choice is kept for the next visit
      assert.equal(await clickNow('[data-testid=use-codex]', "return document.querySelector('[data-testid=use-name]').textContent"), 'codex-personal', 'the Codex account with the most room');
      assert.equal(await exec("return localStorage.getItem('hm.usenext')"), 'codex', 'remembered');
      await overview();
      await hasText('[data-testid=use-name]', 'codex-personal');
      // a tool whose plans have no limits to read: its account, with that reason
      assert.deepEqual(await clickNow('[data-testid=use-opencode]', "return [document.querySelector('[data-testid=use-name]').textContent, document.querySelector('[data-testid=use-why]').textContent]"), ['opencode-main', 'No plan limits to run out']);
      // "5h first" is the rule it always was (the 5-hour window that resets soonest with half its room left), among the chosen tool's accounts
      await click('[data-testid=use-claude]');
      assert.deepEqual(await clickNow('[data-testid=use-prefer]', "return [document.querySelector('[data-testid=use-name]').textContent, document.querySelector('[data-testid=use-prefer]').getAttribute('aria-pressed')]"), ['work-max', 'true']);
      await hasText('[data-testid=use-why]', '64% of the 5-hour window left · resets in 2 h');
      assert.equal(await clickNow('[data-testid=use-prefer]', "return document.querySelector('[data-testid=use-name]').textContent"), 'claude-main', 'and off again');
      await click('[data-testid=use-any]');
    } finally { await sampleOff(); await overview(); }
  });

  await test('overview: Needs attention has one entry per account, the most severe first, each with its one action', async () => {
    await sampleOn();
    try {
      await overview();
      await $('[data-testid=att-update-codex]');
      const d = await exec(`const q = (s) => document.querySelector(s);
        return { n: q('[data-testid=attention-count]').textContent, status: q('[data-testid=page-status]').textContent, title: q('[data-testid=att-work-max]').title, href: q('[data-testid=att-codex-main] a').getAttribute('href'),
          items: [...document.querySelectorAll('[data-testid=attention] .ov-item')].map((i) => [i.querySelector('.two b').textContent, i.querySelector('.two span').textContent, [...i.querySelectorAll('button, a')].map((b) => b.textContent).join('|'), !!i.querySelector('img.ticon')]) };`);
      assert.equal(d.n, '4', 'the header counts the entries');
      assert.deepEqual(d.items.map((i) => i[0]), ['codex-main', 'work-max', 'client-acme', 'Codex'], 'one entry per account: used up, then running out, then a login, then a tool update');
      assert.ok(/^Week used up · back \d\d:\d\d$/.test(d.items[0][1]), d.items[0][1]);
      assert.ok(/^≈ Out 2[12] h before reset$/.test(d.items[1][1]), d.items[1][1]);
      assert.ok(d.title.includes('Week ≈ runs out') && d.title.includes('Week · Fable ≈ runs out'), 'work-max is listed once, though two of its windows run out; both are on hover: ' + d.title);
      assert.deepEqual(d.items.slice(2).map((i) => i[1]), ['Login expired', 'Update 0.169.0 → 0.170.0']);
      assert.deepEqual(d.items.map((i) => i[2]), ['Open', 'Open', 'Log in', 'Update'], 'one action each, the one that works');
      assert.ok(d.items.every((i) => i[3]), 'each with its tool\'s icon');
      assert.equal(d.href, '#account?name=codex-main', 'Open goes to the account');
      assert.ok(/^codex-main is used up until \d\d:\d\d; work-max will run out \w+ \d\d:\d\d, before its reset\.$/.test(d.status), 'the status names the most urgent first: ' + d.status);
    } finally { await sampleOff(); await overview(); }
  });

  await test('overview: every account row is 48 px with nothing wrapped; a long name is cut, and limits past the third sit behind +N', async () => {
    await sampleOn({ long: true });
    try {
      await overview();
      const long = '[data-testid="lane-agy-with-a-very-long-account-name"]';
      await $(long);
      const d = await exec(`const rows = [...document.querySelectorAll('.ov-table tbody tr')], long = document.querySelector(arguments[0]), h = (e) => e.getBoundingClientRect().height, sheet = document.getElementById('sheet');
        return { rows: rows.map((r) => Math.round(h(r) * 2) / 2), names: rows.map((r) => r.dataset.name),
          // every word of a row sits on one line: a name, a pill, a limit's label and number, a verdict
          tall: [...document.querySelectorAll('.ov-table tbody :is(.ov-name, .pill, .ov-lim .t span, .ov-lim .t b, .ov-verdict, .ov-unread, .ov-more)')].filter((e) => h(e) > 22).map((e) => e.className + ' ' + e.textContent),
          // and nothing reaches into the next column
          over: rows.flatMap((r) => [...r.cells].filter((td) => [...td.querySelectorAll('*')].some((e) => e.getBoundingClientRect().right > td.getBoundingClientRect().right + 0.5)).map((td) => r.dataset.name + ' col ' + td.cellIndex)),
          lims: [...long.querySelectorAll('.ov-lim')].map((l) => l.querySelector('.t span').textContent), cut: (() => { const s = long.querySelectorAll('.ov-lim .t span')[2]; return [s.scrollWidth > s.clientWidth, s.closest('.ov-lim').title]; })(),
          more: [long.querySelector('.ov-more').textContent, long.querySelector('.ov-more').title], name: (() => { const n = long.querySelector('.ov-name'); return n.scrollWidth > n.clientWidth; })(),
          plan: (() => { const p = long.querySelector('.ov-plan'); return [p.getBoundingClientRect().width <= 104, p.title]; })(), wide: sheet.scrollWidth > sheet.clientWidth };`, [long]);
      assert.equal(d.names.length, 9, 'one row per account');
      assert.equal(new Set(d.names).size, 9, 'no account twice');
      assert.deepEqual([...new Set(d.rows)], [48], 'every row is 48 px: ' + d.rows);
      assert.deepEqual(d.tall, [], 'nothing wraps to a second line');
      assert.deepEqual(d.over, [], 'nothing reaches into the next column');
      assert.deepEqual(d.lims, ['5h', 'Wk', 'Gemini 3 Pro High and Low thinking'], 'three limits in the line');
      assert.ok(d.cut[0] && d.cut[1].startsWith('Week · Gemini 3 Pro High and Low thinking: 55% left'), 'a long label is cut with an ellipsis, whole on hover: ' + d.cut);
      assert.equal(d.more[0], '+2', 'the fourth and fifth behind +N');
      assert.ok(d.more[1].includes('Gemini 3 Flash and Flash Lite: 90% left') && d.more[1].includes('Claude models through Antigravity: 20% left'), 'listed on hover: ' + d.more[1]);
      assert.ok(d.name, 'a long account name is cut');
      assert.ok(d.plan[0] && d.plan[1] === 'Google AI Ultra for Business, annual', 'a long plan name is cut, whole on hover: ' + d.plan);
      assert.ok(!d.wide, 'nothing is wider than the page');
    } finally { await sampleOff(); await overview(); }
  });

  await test('overview: each account row says its state once and has the one action that works in it', async () => {
    await sampleOn();
    try {
      await overview();
      await $('[data-testid=lane-agy-main]');
      const read = `const row = (r) => ({ limits: [...r.querySelectorAll('.ov-lim')].map((l) => l.textContent.trim()).join(' ') || r.cells[1].textContent.trim(), verdict: r.querySelector('.ov-verdict').textContent, tone: r.querySelector('.ov-verdict').className.replace(/ov-verdict|trunc/g, '').trim(),
          lane: ['fill', 'warn', 'starve', 'tick'].filter((c) => r.querySelector('.ov-lane .' + c)).join(' ') || r.querySelector('.ov-lane').getAttribute('class'),
          acts: [...r.querySelectorAll('button, a')].map((b) => [b.textContent, b.dataset.act, b.disabled, b.title].join('|')) });
        return Object.fromEntries([...document.querySelectorAll('.ov-table tbody tr')].map((r) => [r.dataset.name, row(r)]));`;
      const d = await exec(read);
      const want = {
        'work-max': ['5h64% Wk41% Fable35%', /^≈ Out \w{3} \d\d:\d\d$/, 'warnT', 'fill warn starve tick', /^Start\|start\|false\|/],
        'claude-main': ['5h100% Wk78% Fable80%', /^Lasts until \w{3} \d\d:\d\d$/, '', 'fill tick', /^Start\|start\|false\|/],
        'client-acme': ['—', /^Login expired$/, 'negT', 'ov-lane none', /^Log in\|login\|false\|/],
        'team-ci': ['—', /^Pay per use · ≈ \$4\.20 today$/, 'plain', 'ov-lane none', /^Start\|start\|false\|/],
        'codex-main': ['5h90% Wk0%', /^Used up · back \d\d:\d\d$/, 'negT', 'fill warn starve tick', /^Start\|start\|true\|Used up · back \d\d:\d\d$/],
        'codex-personal': ['5h100% Wk88%', /^Lasts until \w{3} \d\d:\d\d$/, '', 'fill tick', /^Start\|start\|false\|/],
        'opencode-main': ['—', /^No plan limits$/, 'plain', 'ov-lane none', /^Start\|start\|false\|/],
        'agy-main': ['5h100% Wk100%', /^Lasts until next \w{3}$/, '', 'fill tick', /^Start\|start\|false\|/],
      };
      assert.deepEqual(Object.keys(d), Object.keys(want), 'one row per account, in the accounts\' order');
      for (const [name, [limits, verdict, tone, lane, act]] of Object.entries(want)) {
        const r = d[name];
        assert.equal(r.limits.replace(/\s+/g, ' '), limits, `${name}: its limits in one line`);
        assert.ok(verdict.test(r.verdict), `${name}: its verdict, one phrase: ${r.verdict}`);
        assert.equal(r.tone, tone, `${name}: the verdict's tone`);
        assert.equal(r.lane, lane, `${name}: its lane`);
        assert.equal(r.acts.length, 1, `${name}: one action: ${r.acts}`);
        assert.ok(act.test(r.acts[0]), `${name}: the action that works: ${r.acts[0]}`);
      }
      // eight accounts fit above the fold at 1280 × 800
      assert.ok(await exec("return document.querySelector('[data-testid=lane-agy-main]').getBoundingClientRect().bottom <= window.innerHeight"), 'the eighth row is on screen without scrolling');
      // the head line: the tools by icon, name and count; the running sessions counted per computer
      assert.deepEqual(await exec("return [...document.querySelectorAll('[data-act=home-tool]')].map((b) => b.textContent.replace(/\\s+/g, ' ').trim())"), ['All 8', 'Claude Code 4', 'Codex 2', 'OpenCode 1', 'Antigravity 1']);
      assert.equal((await text('[data-testid=running-count]')).trim(), '3 here · 1 on LAPTOP');
      const rows = await exec("return [...document.querySelectorAll('[data-testid=running-now] .ov-item')].map((i) => i.textContent.replace(/\\s+/g, ' ').trim())");
      assert.equal(rows.length, 4, 'a row per running session');
      assert.ok(rows.some((r) => /^Longhorn backup target for staging ?1 h 1[23] m ?platform-gitops ?work-max ?WORKSTATION ?Follow$/.test(r)), 'title, elapsed, project, account, computer, Follow: ' + rows.join(' / '));
      // a count shows that computer's sessions alone, and all of them again
      assert.equal(await clickNow('[data-testid=running-count] [data-v=pc-laptop]', "return [...document.querySelectorAll('[data-testid=running-now] .ov-item b')].map((b) => b.textContent).join()"), 'Invoice export: totals off by one cent');
      assert.equal(await clickNow('[data-testid=running-count] [data-v=pc-laptop]', "return document.querySelectorAll('[data-testid=running-now] .ov-item').length"), 4);
      // the window the lanes draw: the 5-hour window alone goes on a five-hour lane, with its own verdict; drawn at once
      await click('[data-testid=window-pick]');
      const five = await clickNow('[data-testid=window-5h]', "return [[...document.querySelectorAll('.ov-axis span')].map((t) => t.textContent).join(), document.querySelector('[data-testid=verdict-work-max]').textContent, document.querySelector('[data-testid=window-pick]').textContent]");
      assert.deepEqual(five.slice(0, 1), ['Now,+5 h']);
      assert.ok(/^Lasts until \d\d:\d\d$/.test(five[1]) && five[2].startsWith('5-hour only'), 'the 5-hour window\'s own verdict: ' + five);
      await click('[data-testid=window-pick]');
      await click('[data-testid=window-both]');
      await hasText('[data-testid=verdict-work-max]', '≈ Out');
    } finally { await sampleOff(); await overview(); }
  });

  await test('sidebar: Setup folds and stays as left, a page inside opens it, the pinned projects fold, the tray popup', async () => {
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
    // the rail carries no rings, readouts or limits popup: the limits live in the tray icon's own window
    assert.ok(!(await exec("return !!document.querySelector('#tray-root')")), 'no limits popup in the window');
    await click('[data-testid=nav-mcp]');
    await waitFor(async () => (await exec('return location.hash')) === '#mcp', 'MCP servers opened from the fold');
    // nothing is pinned yet, so there is no Pinned fold under Projects (the projects test folds it, with a pin)
    assert.ok(await exec("return document.querySelector('[data-testid=fold-pins]').hidden"), 'no fold without a pin');
  });

  // (here, before any other test opens Accounts: the report is made once, at the first launch)
  await test('the first-run report is said on the first visit to Accounts, and only once', async () => {
    await nav('#accounts');
    await hasText('#toast', 'installed tools');
    await nav('#home');
    await nav('#accounts');
    await $('[data-testid=accounts-table]');
    await sleep(600);
    assert.ok(!(await exec("return document.getElementById('notice').textContent")).includes('installed tools'), 'not said again');
  });

  await test('accounts list shows the main profiles of installed tools', async () => {
    await nav('#accounts');
    await $('[data-testid=accounts-table]');
    await $('[data-testid=row-claude-main]');
    await $('[data-testid=row-codex-main]');
    await hasText('[data-testid=row-codex-main]', 'ChatGPT');
    // every account with plan limits has a ring in the tray popup, read or not: what the tray's window draws
    const tray = await exec("return Promise.all([import('./accounts.js'), import('./core.js')]).then(([a, c]) => a.trayHtml(a.accountViews(c.S.state, {}, Date.now(), null)));");
    assert.ok(tray.includes('nav-limit-claude-main') && tray.includes('nav-limit-codex-main'), 'a ring for each account');
    // the week: its number in the ring and over its bar; the bar's title says the whole of it
    await hasText('[data-testid=row-codex-main] .a-ring', '79%');
    await hasText('[data-testid=row-codex-main] .lims', 'Wk');
    assert.ok((await exec("return document.querySelector('[data-testid=row-codex-main] .lim').title")).startsWith('Week: 79% left (21% used), resets '), 'the week bar says what is left, what is used and when it resets');
    await shot('01-accounts');
  });

  await test('add a Claude account: in-app sign-in with a pasted code, then plan and limits', async () => {
    await click('[data-testid=add-account]');
    await type('[data-testid=add-name]', 'work-max');
    await type('[data-testid=add-email]', 'alex@example.com');
    await hasText('[data-testid=add-cli]', 'hrm add work-max --email alex@example.com');
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
    await waitFor(async () => (await exec("return [...document.querySelectorAll('[data-testid=row-work-max] .lim')].map((l) => l.title).join(' | ')")).includes('36% left'), 'the limits read after the sign-in');
    assert.ok(fs.existsSync(path.join(home, '.claude-work-max', '.credentials.json')));
    assert.ok(fs.existsSync(path.join(home, '.claude-work-max', 'skills', 'pdf', 'SKILL.md')), 'skills linked from the main profile');
    // the account with the most room says so on its limits
    assert.ok((await exec("return document.querySelector('[data-testid=best-note]').title")).includes('Most room of any account'), 'the advice is on the limits of the account to start next');
    await shot('04-accounts-with-limits');
    // the README's screenshot: the same page with a made-up computer name and the fixture's year-2099 resets as real ones
    await exec(`const swap = [[new RegExp(arguments[0], 'gi'), 'DEVBOX']];
      let resets = 0; // the first is the 5-hour window, the rest are weekly
      swap.push([/\\d{5} d \\d+ h/g, () => (resets++ ? '4 d 6 h' : '2 h 14 m')]);
      const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      for (let n; (n = w.nextNode());) for (const [a, b] of swap) n.nodeValue = n.nodeValue.replace(a, b);
      document.getElementById('notice').innerHTML = '';
      document.documentElement.dataset.theme = 'dark';`, [os.hostname()]); // the README shows the dark look, Harness
    await sleep(400);
    await page.screenshot({ path: path.join(shots, 'readme.png') });
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
    await click('[data-testid=add-tool-codex]');
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
    // (the tool is the icon before the account, its name in the title)
    const groups = await exec(`return [...document.querySelectorAll('[data-testid=group]')].map(g => g.innerText + ' ' + [...g.querySelectorAll('.a-acct')].map((t) => t.title).join(' '))`);
    const g = groups.find((x) => x.includes('platform-gitops'));
    assert.ok(g, 'a platform-gitops group');
    assert.ok(g.includes('Claude Code') && g.includes('Codex'), 'both tools in one project');
    // as in the design: a red dot for a session where a command failed
    assert.ok(await exec("return !!document.querySelector('[data-testid=session-s-main-1] .a-dot.bad')"), 'the session with a failed command is marked');
    // the account of a session: its name and the login it holds (at 1280 the narrow column keeps the login in the title)
    assert.ok((await exec("return document.querySelector('[data-testid=acct-cx-1]').textContent")).includes('alex@example.org'), 'the Codex session names its login');
    assert.ok(await exec("return !!document.querySelector('[data-testid=acct-cx-1] img[src*=codex]')"), 'with the icon of its tool before it');
    assert.ok((await exec("return document.querySelector('[data-testid=acct-cx-1]').title")).includes('alex@example.org'), 'and its title does');
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
    await waitFor(async () => (await exec(`return document.querySelectorAll('.a-srow').length`)) === 1 && (await exec('return location.hash')).includes('q=longhorn'), 'the top bar search asks the index and shows one session');
    await page.fill('[data-testid=find]', '');
    await exec(`document.getElementById('find').dispatchEvent(new Event('input'))`);
    await waitFor(async () => (await exec(`return document.querySelectorAll('.a-srow').length`)) > 1, 'every session is back');
  });

  await test('projects: one list, each project on one row with its share, its numbers and its actions', async () => {
    await nav('#projects?days=all');
    await $('[data-testid=project-cards] .a-pcard');
    const d = await exec(`const rows = [...document.querySelectorAll('.a-pcard')], head = document.querySelector('[data-testid=project-dist] .a-prow.head');
      // every place outside the tiles that names the project (the Busiest tile names it too)
      const named = [...document.querySelectorAll('#sheet .page-body > :not(.account-stats) *')].filter((e) => !e.children.length && e.textContent.trim() === 'platform-gitops');
      const x = (el) => [...el.children].map((c) => Math.round(c.getBoundingClientRect().right));
      return { keys: rows.map((r) => r.dataset.key), named: named.length, lists: document.querySelectorAll('[data-testid=project-cards]').length, inOne: rows.every((r) => r.closest('[data-testid=project-dist]')),
        old: !!document.querySelector('.a-pcards, a.a-nm, [data-act=p-dist]'), cols: [x(head), x(rows[0])],
        parts: rows.map((r) => [!!r.querySelector('.a-share svg rect'), r.querySelectorAll(':scope > .num').length, !!r.querySelector('.a-area'), !!r.querySelector('.a-path'), !!r.querySelector('[data-act=tag-edit]'), !!r.querySelector('[data-act=project-pin]'), !!r.querySelector('[data-act=project-menu]')].join()),
        h: rows.map((r) => Math.round(r.getBoundingClientRect().height)) };`);
    assert.ok(d.keys.length >= 2 && new Set(d.keys).size === d.keys.length, 'a row per project: ' + d.keys);
    assert.equal(d.named, 1, 'platform-gitops is listed once');
    assert.ok(d.lists === 1 && d.inOne && !d.old, 'one list, in the card that says where the tokens and the cost went; no second table, no cards');
    assert.ok(d.parts.every((p) => p === 'true,3,true,true,true,true,true'), 'each row: the share bar, cost, tokens and sessions, the sparkline, the path, the tags, the pin and the menu: ' + d.parts);
    assert.deepEqual(d.cols[0], d.cols[1], 'the columns\' names sit over their columns');
    assert.ok(d.h.every((h) => h === 52), 'rows 52 px: ' + d.h);
    // the share follows the metric; the shares of the rows (and the others line, when there is one) add up to all
    await click('[data-testid=pmetric-sessions]');
    await hasText('[data-testid=project-dist] .a-prow.head', 'Share of sessions');
    const sum = await exec("return [...document.querySelectorAll('[data-testid=project-cards] .a-share .num')].reduce((a, e) => a + parseInt(e.textContent, 10), 0)");
    assert.ok(Math.abs(sum - 100) <= 2, 'the shares add up: ' + sum);
    // the ⋯ of a row: open its sessions, pin, rename, merge, reset name
    await click('[data-testid^=pcard-] [data-act=project-menu]');
    assert.deepEqual(await exec("return [...document.querySelectorAll('.menu [role=menuitem]')].map((b) => b.textContent.trim())"), ['Open its sessions', 'Pin to the sidebar', 'Rename project', 'Merge into another project', 'Reset name']);
    await page.keyboard.press('Escape');
    await click('[data-testid=pmetric-tokens]');
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
    // with a pin, Projects has its Pinned fold in the sidebar; the pin carries no bare count
    await click('[data-testid=fold-pins]');
    await waitFor(async () => exec("return document.getElementById('nav-pins').hidden"), 'pins folded');
    await click('[data-testid=fold-pins]');
    await waitFor(async () => !(await exec("return document.getElementById('nav-pins').hidden")), 'and shown');
    assert.equal((await exec("return document.querySelector('[data-testid=nav-pins] .nav-subitem .nav-label').parentElement.innerText")).replace(/[✕\s]+$/, ''), 'platform-gitops', 'a pinned project: its name, and a badge only when a session runs');
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
    await page.dblclick('[data-testid^=pcard-] .a-pname');
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
    await hasText('[data-testid=project-stats]', 'list prices');
    assert.ok(await exec("return [...document.querySelectorAll('.a-srow')].every((r) => r.innerText.includes('platform-gitops'))"), 'only the project\'s sessions');
    await shot('06e-project-sessions');
    // the header's pin icon pins and unpins too
    await click('.page-head [data-act=project-pin]');
    await waitFor(async () => (await exec("return document.querySelector('[data-testid=nav-pins]').innerText")).includes('platform-gitops'), 'pinned from the header');
    await click('.page-head [data-act=project-pin]');
    await waitFor(async () => !(await exec("return document.querySelector('[data-testid=nav-pins]').innerText")).includes('platform-gitops'), 'unpinned from the header');
    await nav('#sessions');
    await hasText('[data-testid=page-title]', 'Sessions');
    await hasText('[data-testid=sessions-count]', 'in the last 7 days');
    // the list has no project rail of its own (the sidebar has the pinned projects): the Project picker comes with it
    await $('[data-testid=pick-project]');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=rail], .a-rail')")), 'no project rail on the page');
  });

  await test('reading a session: steps, failed command, answer and changes', async () => {
    await nav('#sessions');
    await dbl('[data-testid=read-s-main-1]');
    await hasText('[data-testid=session-title]', 'Longhorn backup target');
    // the steps inline in the chat, folded: each says what it ran; a click unfolds its output
    await hasText('[data-testid=chat] .a-steps', 'argocd app sync longhorn-staging');
    assert.ok(!(await text('[data-testid=chat]')).includes('connection closed'), 'output folded');
    await click('[data-testid=chat] .a-step:has(.a-bad) [data-act=step-open]');
    await hasText('[data-testid=chat]', 'connection closed');
    // the conversation as a chat: the prompt on the right, then the steps, then the answer, in the order they happened
    await hasText('[data-testid=chat] .a-msg.you', 'Add a Longhorn backup target');
    await hasText('[data-testid=chat] .a-msg.ai', 'What changed');
    const order = await exec("return [...document.querySelectorAll('#chat > .a-msg, #chat > .a-steps')].map((e) => e.className.split(' ')[0] + (e.classList.contains('you') ? ':you' : e.classList.contains('ai') ? ':ai' : ''))");
    assert.deepEqual(order, ['a-msg:you', 'a-steps', 'a-msg:ai'], 'prompt, steps, answer');
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
    const live = () => exec("return import('./sessions.js').then((v) => import('./core.js').then((c) => v.watchLive({ ...c.S.cache.transcript.meta, open: true })))");
    await live();
    const atEnd = () => exec("const c = document.getElementById('chat'); return c.scrollHeight - c.scrollTop - c.clientHeight < 40");
    const shown = () => exec("return !document.querySelector('.a-toend').hidden");
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
    await click('[data-testid=session-s-main-1] .a-tagbtn');
    await type('[data-testid=tag-new]', 'release\n');
    await waitFor(async () => exec("return [...document.querySelectorAll('#tg-list input:checked')].map((i) => i.value).includes('release')"), 'release ticked');
    await click('[data-testid=tag-save]');
    await hasText('[data-testid=session-s-main-1]', 'release');
    assert.ok(JSON.parse(hrm('tags', '--json')).some((t) => t.tag === 'release' && t.sessions === 1), 'the CLI lists it');
    // Group by Tag: a release group and an untagged one; the chip filters to the tagged session
    await click('[data-testid=group-tag]');
    await waitFor(async () => (await exec("return [...document.querySelectorAll('[data-testid=group] .a-gname')].map((e) => e.textContent.trim())")).join(',').includes('release'), 'a release group');
    await hasText('#sheet', 'untagged');
    // (the toast sits over the row at this window size, so the chip is clicked by script)
    await exec("document.querySelector('[data-testid=session-s-main-1] .a-tags a.a-tag').click()");
    await $('[data-testid=chip-tag]');
    await waitFor(async () => (await exec("return document.querySelectorAll('.a-srow').length")) === 1, 'only the tagged session');
    // the Tag picker lists the tag, marked as the one in force
    await click('[data-testid=pick-tag]');
    await $('.menu [data-testid=pick-tag-release].on');
    await shot('06f-sessions-tags');
    await page.keyboard.press('Escape');
    // a project's tag reaches its sessions: tag platform-gitops from the Projects page, then filter by it
    await nav('#projects');
    await $('[data-testid=project-cards]');
    await exec("[...document.querySelectorAll('[data-testid^=pcard-]')].find((c) => c.innerText.includes('platform-gitops')).querySelector('.a-tagbtn').click()");
    await type('[data-testid=tag-new]', 'client-x\n');
    await click('[data-testid=tag-save]');
    await waitFor(async () => (await exec("return [...document.querySelectorAll('[data-testid^=pcard-]')].find((c) => c.innerText.includes('platform-gitops')).innerText")).includes('client-x'), 'the card shows it');
    await nav('#sessions?tag=client-x');
    await waitFor(async () => (await exec("return document.querySelectorAll('.a-srow').length")) >= 2, 'every session of the project carries the project tag');
    assert.ok(hrm('sessions', '--tag', 'client-x').includes('platform-gitops'), 'the CLI filters by tag too');
    await click('[data-testid=chip-tag]');
    await waitFor(async () => !(await exec("return location.hash")).includes('tag='), 'the ✕ of the chip takes the tag off');
  });

  await test('sessions: one filter line, and the Project and Tag pickers filter as the page\'s own rail did', async () => {
    const hash = () => exec('return location.hash');
    const items = () => exec("return [...document.querySelectorAll('.menu [role=menuitem]')].map((b) => b.innerText.replace(/\\s+/g, ' ').trim())");
    await nav('#sessions?group=date&days=30');
    await $('[data-testid=pick-project]');
    // at 1280 the filters are one line: the grouping, the range, the two pickers
    const line = await exec(`const f = document.querySelector('[data-testid=filters]'), kids = [...f.children].filter((e) => e.getBoundingClientRect().width);
      return { w: window.innerWidth, mids: kids.map((e) => { const r = e.getBoundingClientRect(); return Math.round(r.top + r.height / 2); }), low: [...f.querySelectorAll('button')].filter((b) => b.closest('.seg') ? b.closest('.seg').getBoundingClientRect().height < 28 : b.getBoundingClientRect().height < 28).length,
        text: f.innerText.replace(/\\s+/g, ' ') };`);
    assert.equal(line.w, 1280);
    assert.ok(Math.max(...line.mids) - Math.min(...line.mids) <= 1, 'one line: ' + line.mids);
    assert.equal(line.low, 0, 'every control at least 28 px tall');
    assert.ok(/Group by ?Project ?Date ?Account ?Tool ?Tag ?7 d ?30 d ?All ?Project ▾ ?Tag ▾/.test(line.text), line.text);
    // the Project picker lists what the rail listed: Live now and All projects first, the projects with their counts, every project last
    await click('[data-testid=pick-project]');
    await $('.menu [data-testid=pick-live]');
    const list = await items();
    assert.equal(list[0], 'Live now');
    assert.ok(/^All projects \d+$/.test(list[1]), list[1]);
    assert.ok(list.some((t) => /^platform-gitops \d+$/.test(t)), 'a project with its count: ' + list);
    assert.equal(list[list.length - 1], 'Every project, with costs →');
    assert.ok(await exec("return document.querySelector('.menu [data-testid=pick-all]').classList.contains('on')"), 'All projects is the choice in force');
    // a project: its sessions, with the grouping and the range kept in the address
    await page.click('.menu [role=menuitem]:has-text("platform-gitops")');
    await hasText('[data-testid=project-title]', 'platform-gitops');
    assert.equal(await hash(), '#sessions?group=date&days=30&project=' + encodeURIComponent('github.com/example/platform-gitops'));
    // Live now: the filter, said by its chip; the ✕ takes it off
    await nav('#sessions?group=date&days=30');
    await click('[data-testid=pick-project]');
    await click('.menu [data-testid=pick-live]');
    await $('[data-testid=chip-live]');
    assert.equal(await hash(), '#sessions?group=date&days=30&live=1');
    await click('[data-testid=chip-live]');
    await waitFor(async () => (await hash()) === '#sessions?group=date&days=30', 'the live filter off');
    // the Tag picker: every tag with its chip and its count; one filters to what carries it
    await click('[data-testid=pick-tag]');
    await $('.menu [data-testid=pick-tag-release]');
    assert.ok((await items()).some((t) => /^release \d+$/.test(t)) && (await exec("return !!document.querySelector('.menu [data-testid=pick-tag-release] .a-tag')")), 'a tag as its chip, with its count');
    await click('.menu [data-testid=pick-tag-release]');
    await $('[data-testid=chip-tag]');
    assert.equal(await hash(), '#sessions?group=date&days=30&tag=release');
    await waitFor(async () => (await exec("return document.querySelectorAll('.a-srow').length")) === 1, 'only the tagged session');
    // All projects takes every filter off
    await click('[data-testid=pick-project]');
    await click('.menu [data-testid=pick-all]');
    await waitFor(async () => (await hash()) === '#sessions?group=date&days=30', 'every project again');
    // Every project, with costs → opens Projects
    await click('[data-testid=pick-project]');
    await click('.menu [data-testid=pick-projects]');
    await waitFor(async () => (await hash()) === '#projects', 'the Projects page');
  });

  await test('OpenCode and Antigravity sessions are listed and readable', async () => {
    await nav('#sessions');
    await waitFor(async () => (await exec("return [...new Set([...document.querySelectorAll('.a-srow .a-acct')].map((e) => e.title.split(' · ')[0]))].sort().join()")) === 'Antigravity,Claude Code,Codex,OpenCode', 'sessions of the four tools');
    const noFolder = await exec("return [...document.querySelectorAll('[data-testid=group]')].find((g) => g.textContent.includes('No folder')).textContent");
    assert.ok(!noFolder.includes('github.com'), 'a session without a folder has no repository');
    assert.ok(noFolder.includes('1 msg') || noFolder.includes('2 msgs'));
    await dbl('[data-testid=read-ses_oc1]');
    await hasText('[data-testid=session-title]', 'Fix the flaky build');
    await hasText('[data-testid=chat]', 'cargo test --workspace');
    await hasText('[data-testid=chat]', 'The build is stable again.');
    await nav('#sessions');
    await dbl('[data-testid=read-agy-conv-1]');
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
    assert.equal(await exec("return document.querySelectorAll('#chat .a-step, #chat .a-more').length"), 11, 'first 5, a link, last 5');
    await hasText('[data-testid=more-steps]', 'Show 1,990 more steps');
    await click('[data-testid=more-steps]');
    await waitFor(async () => (await exec("return document.querySelectorAll('#chat .a-step').length")) === 2000, 'all steps shown');
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

  await test('session: the head\'s actions sit on one row at 1280: one primary, Export as Markdown beside it, the rest behind ⋯', async () => {
    await nav('#session?profile=claude-main&id=s-main-1');
    await hasText('[data-testid=session-title]', 'Longhorn');
    const head = () => exec(`const a = document.querySelector('.page-head .actions'), bs = [...a.querySelectorAll('button')], sheet = document.getElementById('sheet');
      return { w: window.innerWidth, tops: [...new Set(bs.map((b) => Math.round(b.getBoundingClientRect().top)))], heights: [...new Set(bs.map((b) => Math.round(b.getBoundingClientRect().height)))],
        inside: a.getBoundingClientRect().right <= sheet.getBoundingClientRect().right && sheet.scrollWidth <= sheet.clientWidth, labels: bs.map((b) => b.textContent.trim()),
        primary: [...document.querySelectorAll('#sheet .btn.primary, #sheet .btn-primary')].filter((b) => b.getBoundingClientRect().width).map((b) => b.dataset.act) };`);
    const d = await head();
    assert.equal(d.w, 1280);
    assert.ok(d.tops.length === 1 && d.inside, 'the actions on one row, inside the page: ' + JSON.stringify(d));
    assert.deepEqual(d.heights, [30], 'each 30 px tall');
    assert.ok(d.labels.length === 4 && d.labels[0] === 'Export as Markdown' && d.labels[1].startsWith('Resume in') && d.labels[2] === '▾' && d.labels[3] === '⋯', 'Export, Resume with its ▾, ⋯: ' + d.labels);
    assert.deepEqual(d.primary, ['resume', 'terminal-menu'], 'one primary on the page: Resume, with the ▾ that is part of it');
    // a title of three lines moves nothing: the name wraps, the actions stay one row
    await exec("document.querySelector('[data-testid=session-title]').textContent = 'Longhorn backup target for staging, and every other cluster too '.repeat(4)");
    const long = await head();
    assert.ok(long.tops.length === 1 && long.inside, 'still one row beside a long title: ' + JSON.stringify(long));
    // the rest is behind ⋯; the ▾ chooses the terminal
    await click('[data-testid=session-menu]');
    await hasText('.menu', 'Open the log file folder');
    await page.keyboard.press('Escape');
    await click('.page-head [data-act=terminal-menu]');
    await hasText('.menu', 'Open sessions in a console window');
    await page.keyboard.press('Escape');
    // the tiles, the conversation and its three views are where they were
    assert.equal(await exec("return document.querySelectorAll('#sheet .account-stat').length"), 5, 'five tiles');
    assert.ok(await exec("return !!document.querySelector('[data-testid=conv-panel] [data-testid=chat]') && !!document.querySelector('[data-testid=follow-end]')"), 'the conversation, with its ↓ Latest');
    // a session running here (the sandbox has no console, so the transcript is answered as open): the primary is
    // Follow live, and Resume is behind ⋯ with the terminal choice
    await exec(`const t = window.__TAURI__; window.__core = t.core;
      t.core = { ...t.core, invoke: (cmd, a, o) => window.__core.invoke(cmd, a, o).then((r) => (a && a.op === 'transcript' ? { ...r, meta: { ...r.meta, open: true, source_path: '' } } : r)) };`);
    try {
      await nav('#sessions');
      await nav('#session?profile=claude-main&id=s-main-1');
      await $('[data-testid=follow-live]');
      const live = await head();
      assert.deepEqual(live.labels, ['Export as Markdown', 'Follow live', '⋯'], 'a running session: Export, Follow live, ⋯');
      assert.ok(live.tops.length === 1 && live.inside, 'on one row');
      assert.deepEqual(live.primary, ['follow-end'], 'Follow live is the one primary');
      await hasText('#sheet .a-pill.live', 'LIVE');
      await click('[data-testid=session-menu]');
      assert.deepEqual(await exec("return [...document.querySelectorAll('.menu [role=menuitem]')].map((b) => b.textContent.trim().replace(/Resume in .*/, 'Resume in …'))"),
        ['Resume in …', 'Open sessions in Windows Terminal', 'Open sessions in a console window', 'Open the log file folder'], 'Resume, the terminal and the log folder behind ⋯');
      await page.keyboard.press('Escape');
    } finally { await exec('window.__TAURI__.core = window.__core'); await nav('#sessions'); }
  });

  await test('accounts: a card and a chip per tool, a header that folds, Add account as a dialog; a session with its Conversation panel', async () => {
    await nav('#accounts');
    await $('[data-testid=accounts-table]');
    const d = await exec(`return {
        groups: [...document.querySelectorAll('[data-testid=accounts-table] .tool-group-header h2')].map((t) => t.textContent),
        chips: [...document.querySelectorAll('.a-filters [data-act=acct-tool]')].map((a) => a.textContent.trim().replace(/ \\d+$/, '')),
        icons: [...document.querySelectorAll('.a-filters [data-act=acct-tool] img.ticon, .tool-group-header img.ticon')].map((i) => i.getAttribute('src')),
      };`);
    assert.deepEqual(d.groups, ['Claude Code', 'Codex', 'OpenCode', 'Antigravity', 'Pi', 'Oh My Pi'], 'accounts grouped by tool, one card each');
    assert.deepEqual(d.chips, ['All', 'Claude Code', 'Codex', 'OpenCode', 'Antigravity', 'Pi', 'Oh My Pi'], 'a chip for each tool, with its count');
    const svgs = ['claude', 'codex', 'opencode', 'agy', 'pi', 'omp'].map((t) => `icons/${t}.svg`);
    assert.deepEqual(d.icons, [...svgs, ...svgs], 'the official icon of each tool, on its chip and on its group');
    // a tool chip shows only that tool; a group header folds its accounts away
    await click('[data-testid=chip-codex]');
    await waitFor(async () => (await exec("return document.querySelectorAll('[data-testid=accounts-table] .tool-group-card').length")) === 1, 'only Codex');
    await click('[data-testid=chip-all]');
    await click('[data-testid=group-claude] [data-act=acct-fold]');
    await waitFor(async () => !(await exec("return !!document.querySelector('[data-testid=row-claude-main]')")), 'Claude Code folded');
    await click('[data-testid=group-claude] [data-act=acct-fold]');
    await $('[data-testid=row-claude-main]');
    await click('[data-testid=add-account]');
    await $('.modal-card .kind-pick-card.active');
    await exec("document.querySelector('[data-act=dialog-close]').click()");
    await nav('#session?profile=claude-main&id=s-main-1');
    await $('[data-testid=chat]');
    const r = await exec("return { panel: !!document.querySelector('[data-testid=conv-panel]'), modes: [...document.querySelectorAll('[data-tour=modes] button')].map((a) => a.textContent.trim()), resume: document.querySelector('[data-testid=resume]').textContent.trim() }");
    assert.ok(r.panel, 'the Conversation panel');
    assert.deepEqual(r.modes, ['All', 'Changes', 'Failed']);
    assert.ok(r.resume.startsWith('Resume in'));
  });

  // The design's measurements (the 1.10 shell, the type scale, the page heads, the badges): tests of their own
  await designTests({ test, exec, nav, $, click, waitFor, assert, page, shot });

  // Each page has the design page's sections, in its order: the titles of the design (rendered from the canvas)
  // must appear among the page's headings in that order; headings named by data (accounts, routes) may sit between.
  await test('every page has the sections of its design page, in order', async () => {
    const want = {
      '#home': ['Overview', 'Use next', 'Needs attention', 'Running now', 'Accounts runway', 'Activity, 53 weeks', 'Latest replies'],
      '#accounts': ['Accounts', 'Claude Code', 'Codex', 'OpenCode', 'Antigravity', 'Tokens per day, by account'],
      '#account?name=claude-main': ['claude-main', 'Limits', 'Login and folder', 'Tokens per day · 30 days', 'By model · 30 days', 'Recent sessions'],
      '#projects': ['Projects', 'Where the tokens and the cost went'],
      '#sessions': ['Sessions', 'platform-gitops'],
      '#session?profile=claude-main&id=s-main-1': ['Longhorn backup target for staging', 'Conversation'],
      '#computers': ['Computers', 'How your computers sync'],
      '#routing': ['API keys & routing', 'Local router', 'Routes', 'Providers', 'Recent requests through the router'],
      '#mcp': ['MCP servers', 'Write to each tool'], '#skills': ['Skills', 'Installed'], '#prompts': ['System prompts', 'The file now'],
      '#apps': ['Apps', 'Installed', 'Harness Manager'],
      '#settings':['Settings', 'Appearance'], '#settings?tab=limits': ['Settings', 'Limits'], '#settings?tab=backups': ['Settings', 'Backups'],
      '#cli': ['CLI reference', 'Accounts', 'Run', 'Usage and limits', 'Sessions and projects', 'Shared setup', 'Routing and setup'],
    };
    // the replies panel as the design opens it (an earlier test left the full log open)
    await nav('#home');
    await $('[data-testid=activity]');
    if (await exec("return !!document.querySelector('[data-testid=request-log]')")) await click('[data-testid=request-log-open]');
    for (const [h, titles] of Object.entries(want)) {
      await nav(h);
      await drawn(h);
      await waitFor(async () => {
        const got = await exec("return [...document.querySelectorAll('#sheet h1, #sheet h2, #sheet .panel-title')].map((e) => e.textContent.replace(/\\s+/g, ' ').trim())");
        let i = 0;
        for (const g of got) if (i < titles.length && g.startsWith(titles[i])) i++;
        return i === titles.length;
      }, `${h}: ${titles.join(' / ')}`);
    }
    // Settings: the sections in the drawing's order, and as many rows in each as its head counts
    await nav('#settings');
    await $('[data-testid=sec-appearance]');
    assert.deepEqual(await exec("return [...document.querySelectorAll('[data-testid=set-nav] a')].map((a) => a.textContent.trim())"), ['Appearance', 'Window and terminal', 'Updates', 'Limits', 'Connections', 'Log', 'Maintenance', 'Backups']);
    const counts = [];
    for (const id of ['appearance', 'window', 'updates', 'limits', 'connections', 'log', 'maintenance', 'backups']) {
      await nav('#settings?tab=' + id);
      await $(`[data-testid=sec-${id}]`);
      const [rows, says] = await exec("return [document.querySelectorAll('.set-pane .srow[data-key^=setting-]').length, document.querySelector('.set-pane .pane-head .muted').textContent]");
      assert.equal(says, `${rows} settings`, `${id}: the head counts its rows`);
      counts.push(rows);
    }
    assert.deepEqual(counts, [5, 3, 2, 2, 2, 3, 3, 6], 'settings per section: Appearance has Text size too');
  });

  await test('usage lives on Accounts: the chart, each row\'s tokens and cost, the range, and the old address', async () => {
    await nav('#accounts');
    // the two charts are one panel under the groups, folded until asked for
    const fold = '[data-testid=usage-panel] [data-act=acct-fold]';
    const open = async () => (await exec('return document.querySelector(arguments[0]).getAttribute("aria-expanded")', [fold])) === 'true';
    await $(fold);
    if (await open()) await click(fold); // (left open by an earlier run of this window)
    await waitFor(async () => !(await open()) && !(await exec("return !!document.querySelector('[data-testid=usage-ledger], [data-testid=models-panel]')")), 'the charts folded');
    assert.ok(await exec("const p = document.querySelector('[data-testid=usage-panel]').getBoundingClientRect(), g = document.querySelector('[data-testid=accounts-table]').getBoundingClientRect(); return p.top >= g.bottom"), 'under the groups');
    await click(fold);
    await $('[data-testid=usage-panel] [data-testid=usage-ledger]');
    await hasText('[data-testid=usage-panel] [data-testid=models-panel]', 'claude-opus-5-5');
    // open is remembered: on another page and back, the charts are still there
    await nav('#home');
    await nav('#accounts');
    await $('[data-testid=usage-panel] [data-testid=usage-ledger]');
    // the Claude session priced from the catalogue (an estimate, so it carries ≈); Codex's gpt-6 too
    await waitFor(async () => /≈ \$\d+\.\d\d/.test(await text('[data-testid=cost-claude-main]')), 'a cost on the Claude row');
    await hasText('[data-testid=group-codex] .tool-group-header', 'tokens');
    await click('[data-testid=u-days-30]');
    await hasText('[data-testid=row-claude-main] .spark', '30 days');
    await hasText('[data-testid=group-codex] .tool-group-header', 'in 30 days');
    await click('[data-testid=u-days-7]');
    await hasText('[data-testid=row-claude-main] .spark', '7 days');
    await exec("document.querySelector('[data-testid=usage-panel]').scrollIntoView({ block: 'end' })");
    await shot('09-usage');
    // folded again, and that is remembered too
    await click(fold);
    await waitFor(async () => !(await open()), 'folded again');
    await nav('#home');
    await nav('#accounts');
    await $(fold);
    assert.ok(!(await open()), 'the fold is remembered');
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
    // a routed account's daily spend cap: set in its settings, shown on the accounts list
    hrm('add', 'capped', '--route', 'cheap');
    await nav('#account?name=capped&tab=settings');
    await type('[data-testid=cap-input]', '5');
    await click('[data-testid=cap-save]');
    await waitFor(() => (JSON.parse(fs.readFileSync(path.join(data, 'profiles.json'), 'utf8')).profiles.find((x) => x.name === 'capped') || {}).daily_cap_usd === 5, 'cap saved');
    await nav('#accounts');
    await hasText('[data-testid=row-capped]', '$5.00 daily cap');
    await shot('10b-accounts-cap');
    hrm('remove', 'capped');
  });

  // ---------- Accounts and the page of one account, as the 1.10 drawing (docs/project-review-2026-11/mockups/a-accounts.html) ----------
  /** While fn runs, the state the window reads is changed first by `change` (a function body over `st`, run in the window). */
  const withState = async (change, fn) => {
    await exec(`const t = window.__TAURI__, change = new Function('st', arguments[0]); window.__core = t.core;
      t.core = { ...t.core, invoke: (cmd, a, o) => window.__core.invoke(cmd, a, o).then((r) => { if (a && a.op === 'state') change(r); return r; }) };`, [change]);
    try { await fn(); } finally { await exec('window.__TAURI__.core = window.__core'); }
  };
  /** Open Accounts again from another page, so it is drawn from a fresh read. */
  const reopenAccounts = async () => { await nav('#home'); await nav('#accounts'); await drawn('accounts'); await $('[data-testid=accounts-table] .acct-row'); };
  /** What the rows measure: each one's height, the lines of every piece of text in it, and whether a column runs into the next. */
  const rowFacts = () => exec(`const z = document.documentElement.currentCSSZoom || 1, out = { heights: [], wrapped: [], overlap: [] };
    for (const row of document.querySelectorAll('.acct-row')) {
      const box = row.getBoundingClientRect(), kids = [...row.children];
      out.heights.push(Math.round(box.height / z));
      for (const el of row.querySelectorAll('*')) {
        if (![...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())) continue;
        const r = document.createRange(); r.selectNodeContents(el);
        if (new Set([...r.getClientRects()].map((x) => Math.round(x.top))).size > 1) out.wrapped.push(row.dataset.name + ': ' + el.textContent.trim());
      }
      kids.forEach((k, i) => { const r = k.getBoundingClientRect(), next = kids[i + 1] ? kids[i + 1].getBoundingClientRect().left : box.right; if (r.right > next + 0.5 || k.scrollHeight > box.height) out.overlap.push(row.dataset.name + ': column ' + (i + 1)); });
    }
    return out;`);

  await test('accounts: every row is 56 px with nothing wrapped; at most three limits in one line, a long name cut with its whole name in the title, the rest behind +N', async () => {
    await reopenAccounts();
    const d = await rowFacts();
    assert.ok(d.heights.length >= 6, 'the rows were measured: ' + d.heights.length);
    assert.deepEqual([...new Set(d.heights)], [56], 'every row is 56 px');
    assert.deepEqual(d.wrapped, [], 'no text in a row wraps');
    assert.deepEqual(d.overlap, [], 'no column runs into the next');
    // an account with five windows, one of them with a long name, and one logged in whose login has expired
    const soon = Date.now() + 3 * 3600e3;
    await withState(`const p = st.profiles.find((x) => x.name === 'work-max'), t = (id, label, used) => ({ id, label, used, resets_at: ${soon} });
      p.quota = { fetched_at: Date.now(), tiers: [t('session', '5-hour session', 36), t('week', 'Week', 59), t('week:claude and gpt models', 'Week · Claude and GPT models', 65), t('week:sonnet', 'Week · Sonnet', 100), t('week:opus', 'Week · Opus', 12)] };
      p.identity.plan = 'Enterprise premium seat with a very long plan name';`, async () => {
      await reopenAccounts();
      const row = '[data-testid=row-work-max]';
      const w = await exec(`const row = document.querySelector(arguments[0]), lims = [...row.querySelectorAll('.lim')], cut = (e) => e.scrollWidth > e.clientWidth, plan = row.querySelector('.pill.plan');
        return { labels: lims.map((l) => l.querySelector('.t span').textContent), cut: lims.map((l) => cut(l.querySelector('.t span'))), titles: lims.map((l) => l.title.split(',')[0]),
          more: row.querySelector('.lim-more').textContent, moreTitle: row.querySelector('.lim-more').title, plan: [cut(plan), plan.title],
          fits: row.querySelector('.lims').getBoundingClientRect().right <= row.querySelector('.spark').getBoundingClientRect().left };`, [row]);
      assert.deepEqual(w.labels, ['5h', 'Wk', 'Claude and GPT models'], 'three limits, by their short names');
      assert.deepEqual(w.cut, [false, false, true], 'the long name is cut with an ellipsis');
      assert.deepEqual(w.titles, ['5-hour session: 64% left (36% used)', 'Week: 41% left (59% used)', 'Week · Claude and GPT models: 35% left (65% used)'], 'each bar says the whole of it in its title');
      assert.equal(w.more, '+2', 'the other two go behind +2');
      assert.ok(w.moreTitle.includes('Week · Sonnet: 0% left (100% used)') && w.moreTitle.includes('Week · Opus: 88% left'), 'and are named in its title: ' + w.moreTitle);
      assert.deepEqual(w.plan, [true, 'Enterprise premium seat with a very long plan name'], 'a long plan name is cut too, whole in its title');
      assert.ok(w.fits, 'the limits stay inside their column');
      const e = await rowFacts();
      assert.deepEqual([[...new Set(e.heights)], e.wrapped, e.overlap], [[56], [], []], 'the rows keep their height, and nothing wraps or overlaps');
      await shot('04b-accounts-long-labels');
    });
    await reopenAccounts();
    // every control of the page is at least 28 px tall (a choice of a segmented control is 24 px inside its 30 px frame, as drawn)
    const low = await exec(`return [...document.querySelectorAll('#sheet button, #sheet a[href], #sheet input, #sheet select')].filter((e) => !e.closest('.seg') && e.getBoundingClientRect().height < 28).map((e) => (e.dataset.testid || e.className) + ' ' + e.getBoundingClientRect().height)
      .concat([...document.querySelectorAll('#sheet .seg')].filter((e) => e.getBoundingClientRect().height < 28).map((e) => 'seg ' + e.getBoundingClientRect().height))`);
    assert.deepEqual(low, [], 'controls under 28 px');
    // the VektraGrid look, light and dark: the rows hold there too (shots for review; the Harness light look is put back)
    try {
      for (const theme of ['light', 'dark']) {
        await exec("document.documentElement.dataset.brand = 'vg'; document.documentElement.dataset.theme = arguments[0]", [theme]);
        await sleep(300);
        const v = await rowFacts();
        assert.deepEqual([[...new Set(v.heights)], v.wrapped, v.overlap], [[56], [], []], 'VektraGrid ' + theme + ': 56 px rows, nothing wrapped');
        await shot('04d-accounts-vektragrid-' + theme);
      }
    } finally { await exec("document.documentElement.dataset.brand = 'hm'; document.documentElement.dataset.theme = 'light'"); }
  });

  await test('accounts: one button per row for its state (Start, or Log in in the warning outline), and the ⋯ menu holds every other action, each of which works', async () => {
    hrm('add', 'side-route', '--route', 'cheap');
    await reopenAccounts();
    // the one button: Start for an account that can start, Log in for one that is logged out (the sandbox's claude-main has no login)
    const acts = await exec(`return Object.fromEntries([...document.querySelectorAll('.acct-row')].map((r) => [r.dataset.name, [...r.querySelectorAll('.acts button')].map((b) => b.textContent.trim()).join(' ')]))`);
    assert.equal(acts['claude-main'], 'Log in ⋯', 'a logged-out account: Log in');
    for (const n of ['work-max', 'team-ci', 'codex-main', 'codex-personal', 'side-route']) assert.equal(acts[n], 'Start ⋯', n + ': Start');
    await page.mouse.move(640, 5); // no hover colour on what is measured
    const look = await exec(`const cs = (s) => getComputedStyle(document.querySelector(s)), l = cs('[data-testid=login-claude-main]'), s = cs('[data-testid=start-work-max]');
      return { login: [l.borderTopColor, l.color], start: [s.borderTopColor, s.color], size: [l.height, l.width, s.height, s.width], title: document.querySelector('[data-testid=login-claude-main]').title };`);
    assert.deepEqual(look.login, ['rgba(138, 79, 0, 0.35)', 'rgb(138, 79, 0)'], 'Log in wears the warning outline');
    assert.notDeepEqual(look.login, look.start, 'so it does not read as Start');
    assert.deepEqual(look.size, ['28px', '64px', '28px', '64px'], 'both 28 px tall and 64 wide');
    assert.ok(look.title.startsWith('Not logged in yet'), 'and its title says why: ' + look.title);
    // an account whose login has expired gets Log in too
    await withState("st.profiles.find((x) => x.name === 'codex-personal').identity.needs_login = true;", async () => {
      await reopenAccounts();
      await hasText('[data-testid=login-codex-personal]', 'Log in');
      assert.ok((await exec("return document.querySelector('[data-testid=login-codex-personal]').title")).startsWith('Its login has expired'), 'an expired login says so');
      await hasText('[data-testid=row-codex-personal] .unread.trunc', '— Login expired');
      assert.ok(!(await exec("return !!document.querySelector('[data-testid=start-codex-personal]')")), 'and no Start beside it');
    });
    await reopenAccounts();
    // Log in opens the sign-in
    await click('[data-testid=login-claude-main]');
    await $('[data-testid=login-code]');
    await click('.dlg .dh [data-act=dialog-close]');

    // the ⋯ menu, per kind of account: every other action, Remove… last and in the danger colour
    const menu = async (name) => { await click(`[data-testid="menu-${name}"]`); await $('.menu.acct-menu'); return exec("return [...document.querySelectorAll('.menu button')].map((b) => b.textContent.trim())"); };
    const pick = async (name, label) => { await menu(name); await page.click(`.menu button:text-is("${label}")`); };
    assert.deepEqual(await menu('work-max'), ['Start in a folder…', 'Refresh limits', 'Rename', 'Edit', 'Open folder', 'Log in again', 'Sync from the main profile', 'Remove…'], 'a Claude subscription');
    const m = await exec(`const m = document.querySelector('.menu'), b = document.querySelector('[data-testid=menu-work-max]'), mr = m.getBoundingClientRect(), br = b.getBoundingClientRect(), last = m.querySelector('button:last-child');
      return { right: Math.round(mr.right - br.right), below: Math.round(mr.top - br.bottom), items: [...new Set([...m.querySelectorAll('button')].map((x) => Math.round(x.getBoundingClientRect().height)))], font: getComputedStyle(last).fontSize,
        danger: [last.classList.contains('danger'), getComputedStyle(last).color], line: getComputedStyle(last, '::before').borderTopWidth, open: b.getAttribute('aria-expanded') };`);
    assert.deepEqual([m.right, m.below, m.items, m.font], [0, 4, [30], '13px'], 'under its button, right edges together; 30 px items in 13 px');
    assert.deepEqual([m.danger, m.line, m.open], [[true, 'rgb(180, 40, 24)'], '1px', 'true'], 'Remove… in the danger colour after a line; the button says the menu is open');
    await page.keyboard.press('Escape');
    assert.deepEqual(await menu('codex-main'), ['Start in a folder…', 'Refresh limits', 'Rename', 'Edit', 'Open folder', 'Log in again', 'Hide main profile…'], 'a main profile is hidden, not removed');
    await shot('04c-accounts-menu');
    await page.keyboard.press('Escape');
    assert.deepEqual(await menu('claude-main'), ['Start in a folder…', 'Rename', 'Edit', 'Open folder', 'Hide main profile…'], 'logged out: no limits to refresh, and Log in is the row\'s button');
    await page.keyboard.press('Escape');
    assert.deepEqual(await menu('team-ci'), ['Start in a folder…', 'Rename', 'Edit', 'Open folder', 'Sync from the main profile', 'Remove…'], 'an API key: no limits to refresh, no login');
    await page.keyboard.press('Escape');
    assert.deepEqual(await menu('side-route'), ['Start in a folder…', 'Rename', 'Edit', 'Open folder', 'Daily spend cap…', 'Sync from the main profile', 'Remove…'], 'a routed account: its spending cap');
    await page.keyboard.press('Escape');

    // each one works
    await pick('codex-personal', 'Start in a folder…');
    await hasText('.modal-head', 'Start codex-personal in a folder');
    await click('.dlg .dh [data-act=dialog-close]');
    await pick('codex-main', 'Refresh limits');
    await waitFor(async () => /new record|up to date/.test(await exec("return document.getElementById('toast')?.textContent || ''")), 'Refresh limits reported');
    await pick('side-route', 'Rename');
    await type('[data-testid=fd-in]', 'Side route');
    await click('[data-testid=fd-ok]');
    await hasText('[data-testid=row-side-route] .who', 'Side route');
    assert.equal(JSON.parse(hrm('list', '--json')).find((x) => x.name === 'side-route').label, 'Side route', 'renamed, the id stays');
    await pick('side-route', 'Daily spend cap…');
    await type('[data-testid=fd-in]', '3');
    await page.press('[data-testid=fd-in]', 'Enter');
    await hasText('[data-testid=row-side-route] .lims', '$3.00 daily cap');
    assert.equal(JSON.parse(fs.readFileSync(path.join(data, 'profiles.json'), 'utf8')).profiles.find((x) => x.name === 'side-route').daily_cap_usd, 3, 'the cap saved');
    await pick('side-route', 'Edit');
    await waitFor(async () => (await exec('return location.hash')) === '#account?name=side-route&tab=settings', 'Edit opens its settings');
    await $('[data-testid=rename-input]');
    await reopenAccounts();
    // (Open folder would open a file manager window: the call is caught here instead)
    await exec(`const t = window.__TAURI__; window.__core = t.core; window.__opened = [];
      t.core = { ...t.core, invoke: (cmd, a, o) => (a && a.op === 'open_folder' ? (window.__opened.push(a.args.path), Promise.resolve(true)) : window.__core.invoke(cmd, a, o)) };`);
    try {
      await pick('team-ci', 'Open folder');
      await waitFor(async () => (await exec('return window.__opened.length')) === 1, 'Open folder asked for the folder');
      assert.equal(await exec('return window.__opened[0]'), JSON.parse(hrm('show', 'team-ci', '--json')).home, 'the account\'s own folder');
    } finally { await exec('window.__TAURI__.core = window.__core'); }
    const logins = () => readLines(termLog).filter((t) => JSON.stringify(t.args) === JSON.stringify(['login', 'codex-personal'])).length, before = logins();
    await pick('codex-personal', 'Log in again');
    await $('[data-testid=login-status]');
    await waitFor(() => logins() === before + 1, 'Log in again opened the sign-in terminal');
    await click('[data-testid=login-done]');
    await reopenAccounts();
    await pick('work-max', 'Sync from the main profile');
    await hasText('#toast', 'Accounts are in step');
    await pick('side-route', 'Remove…');
    await hasText('.modal-head', 'Remove Side route');
    await click('[data-testid=remove-confirm]');
    await waitFor(async () => !(await exec("return !!document.querySelector('[data-testid=row-side-route]')")), 'removed');
    assert.ok(!JSON.parse(hrm('list', '--json')).some((x) => x.name === 'side-route'), 'gone from the list of accounts');
    // a double click on the row itself (not on a button) still opens the account's page (a click selects it for the pane)
    await dbl('[data-testid=row-team-ci] .who');
    await waitFor(async () => (await exec('return location.hash')) === '#account?name=team-ci', 'the row opens its account');
  });

  await test('account page: a logged-out account has Log in as its primary and Start disabled with the reason; a Limits card with nothing read holds the reason and the fix; the head\'s actions stay on one line', async () => {
    const head = () => exec(`const q = (s) => document.querySelector(s), acts = [...document.querySelectorAll('.page-head .actions > *')], start = q('[data-testid=detail-start]'), body = q('.page-body').getBoundingClientRect();
      return { primary: [...document.querySelectorAll('.page-head .btn.primary')].map((b) => b.textContent.trim()), buttons: acts.map((b) => b.textContent.trim()), start: [start.disabled, start.title],
        tops: [...new Set(acts.map((b) => Math.round(b.getBoundingClientRect().top)))].length, inside: acts.at(-1).getBoundingClientRect().right <= body.right - 13, name: Math.round(q('.page-head h1').getBoundingClientRect().height) };`);
    const limits = () => exec(`const c = document.querySelector('[data-testid=limits-card]'), cols = c.parentElement, fix = c.querySelector('[data-testid=limits-fix]'), w = cols.getBoundingClientRect().width;
      return { text: c.innerText.replace(/\\s+/g, ' ').trim(), fix: fix ? fix.textContent.trim() : null, bars: c.querySelectorAll('.account-limit').length, solo: cols.classList.contains('solo'),
        widths: [...cols.children].map((k) => Math.round((k.getBoundingClientRect().width / w) * 100)), height: Math.round(c.getBoundingClientRect().height) };`);
    // logged out (the sandbox's claude-main has no login)
    await nav('#account?name=claude-main');
    await drawn('the account');
    await $('[data-testid=detail-login]');
    let h = await head();
    assert.deepEqual(h.primary, ['Log in'], 'the one primary is the action that works: Log in');
    assert.deepEqual(h.buttons, ['Start in a folder…', 'Start', 'Log in', '⋯'], 'Start stays, as a secondary');
    assert.deepEqual(h.start, [true, 'Not logged in: log in first'], 'disabled, with the reason in its title');
    assert.deepEqual([h.tops, h.inside, h.name], [1, true, 24], 'the actions on one line, inside the page; the name on one line');
    let l = await limits();
    assert.ok(l.text.startsWith('Limits — Not read: this account is not logged in.'), 'the Limits card says why nothing is read: ' + l.text);
    assert.equal(l.fix, 'Log in', 'and holds the action that fixes it');
    assert.deepEqual([l.solo, l.widths, l.bars], [true, [100, 100], 0], 'one line across the page, the login card under it: no half of the row is empty');
    assert.ok(l.height <= 60, 'the card is as tall as its one line: ' + l.height);
    await shot('05c-account-logged-out');
    try {
      await exec("document.documentElement.dataset.brand = 'vg'; document.documentElement.dataset.theme = 'dark'");
      await sleep(300);
      await shot('05d-account-vektragrid-dark');
    } finally { await exec("document.documentElement.dataset.brand = 'hm'; document.documentElement.dataset.theme = 'light'"); }
    // every tab of the page: no text under 12 px, and every control at least 28 px tall (the back line is the shared head's)
    for (const tab of ['overview', 'sessions', 'projects', 'settings']) {
      await click(`[data-testid=tab-${tab}]`);
      await drawn('the ' + tab + ' tab');
      const t = await exec(`const small = [], low = [];
        for (const el of document.querySelectorAll('#sheet *')) {
          const r = el.getBoundingClientRect();
          if (!r.width || !r.height) continue;
          if ([...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim()) && parseFloat(getComputedStyle(el).fontSize) < 12) small.push(getComputedStyle(el).fontSize + ' ' + el.className + ' ' + el.textContent.trim().slice(0, 24));
          if (el.matches('button, a[href], input, select') && !el.matches('.back-chip') && r.height < 28) low.push(Math.round(r.height) + 'px ' + (el.dataset.testid || el.className) + ' ' + el.textContent.trim().slice(0, 24));
        }
        return { small, low };`);
      assert.deepEqual(t, { small: [], low: [] }, tab + ': text under 12 px, controls under 28 px');
    }
    await click('[data-testid=tab-overview]');
    await drawn('the overview tab');
    assert.equal(await exec("return document.querySelector('[data-testid=limits-fix]').dataset.act"), 'login', 'the card\'s Log in is the same action');
    await click('[data-testid=detail-login]');
    await $('[data-testid=login-code]');
    await click('.dlg .dh [data-act=dialog-close]');
    // logged in, limits read: Start is the primary, the limits beside the login card
    await nav('#account?name=work-max');
    await drawn('the account');
    await $('[data-testid=limits-card] .account-limit');
    h = await head();
    assert.deepEqual([h.primary, h.buttons, h.start[0]], [['Start work-max'], ['Start in a folder…', 'Start work-max', '⋯'], false], 'Start <name> is the primary of an account that can start');
    assert.deepEqual([h.tops, h.inside], [1, true], 'on one line');
    l = await limits();
    assert.ok(!l.solo && l.bars >= 2 && l.fix === null, 'its limits as bars, beside the login card');
    // logged in, nothing read yet: Refresh limits fixes it
    await nav('#account?name=codex-personal');
    await drawn('the account');
    await $('[data-testid=limits-fix]');
    l = await limits();
    assert.ok(l.text.startsWith('Limits — Not read yet') && l.fix === 'Refresh limits' && l.solo, 'not read yet: Refresh limits: ' + l.text);
    await click('[data-testid=limits-fix]');
    await waitFor(async () => /new record|up to date/.test(await exec("return document.getElementById('toast')?.textContent || ''")), 'Refresh limits reported');
    // an API key has no limits, and nothing to fix
    await nav('#account?name=team-ci');
    await drawn('the account');
    await $('[data-testid=limits-card]');
    l = await limits();
    assert.ok(l.text.includes('billed per token') && l.fix === null && l.solo, 'an API key: the plain fact: ' + l.text);
    // a long account name does not push the actions onto a second line
    await exec(`const t = window.__TAURI__; window.__core = t.core;
      t.core = { ...t.core, invoke: (cmd, a, o) => window.__core.invoke(cmd, a, o).then((r) => { if (a && a.op === 'detail') r.view.label = 'An account with a very long name that would not fit beside its buttons'; return r; }) };`);
    try {
      await nav('#account?name=work-max&tab=sessions');
      await drawn('the account');
      await hasText('[data-testid=detail-name]', 'An account with a very long name');
      h = await head();
      assert.deepEqual([h.tops, h.inside, h.name], [1, true, 24], 'a long name is cut, the actions stay on one line');
      assert.ok(await exec("const b = document.querySelector('[data-testid=detail-start]').getBoundingClientRect(); return b.width <= 260"), 'and so is the name on Start');
    } finally { await exec('window.__TAURI__.core = window.__core'); }
  });

  await test('add account: nothing in the dialog is under 12 px, and every control is at least 28 px tall', async () => {
    await nav('#accounts');
    await click('[data-testid=add-account]');
    for (const kind of ['subscription', 'api_key', 'routed']) {
      await click(`[data-testid=kind-${kind}]`);
      await $(`[data-testid=kind-${kind}].active`);
      const d = await exec(`const card = document.querySelector('.modal-card'), small = [], low = []; let read = 0;
        for (const el of card.querySelectorAll('*')) {
          const r = el.getBoundingClientRect();
          if (!r.width || !r.height) continue;
          if ([...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim()) || el.matches('input, select')) { read++; const fs = parseFloat(getComputedStyle(el).fontSize); if (fs < 12) small.push(fs + 'px ' + el.className + ' ' + el.textContent.trim().slice(0, 24)); }
          if (el.matches('button, input, select') && r.height < 28) low.push(Math.round(r.height) + 'px ' + (el.dataset.testid || el.className || el.id));
        }
        return { small, low, read, title: getComputedStyle(card.querySelector('.modal-head h2')).fontSize, field: getComputedStyle(card.querySelector('#ad-name')).fontSize };`);
      assert.ok(d.read > 20, kind + ': the dialog was measured');
      assert.deepEqual(d.small, [], kind + ': text under 12 px');
      assert.deepEqual(d.low, [], kind + ': controls under 28 px');
      assert.deepEqual([d.title, d.field], ['15px', '13px'], kind + ': on the type scale (its title 15, what is typed 13)');
    }
    await click('.dlg .dh [data-act=dialog-close]');
  });

  // ---------- 1.11: the detail pane on Sessions and Accounts, and the groundwork in the same files ----------
  /** Run fn while the window's backend calls are recorded (`ops()` empties the record: "op", or "op:name" for a call
   * about one account or session). `change(op, a, r)` may alter an answer, `fake(op, a)` may give one without asking. */
  const withOps = async (fn, change = '', fake = '') => {
    await exec(`const t = window.__TAURI__, change = new Function('op', 'a', 'r', arguments[0] || ''), fake = new Function('op', 'a', arguments[1] || ''); window.__core = t.core; window.__ops = [];
      t.core = { ...t.core, invoke: (cmd, a, o) => { if (!a || !a.op) return window.__core.invoke(cmd, a, o);
        const args = a.args || {}; window.__ops.push(a.op + (args.name || args.id ? ':' + (args.name || args.id) : ''));
        const f = fake(a.op, args); if (f !== undefined) return Promise.resolve(f);
        return window.__core.invoke(cmd, a, o).then((r) => { change(a.op, args, r); return r; }); } };`, [change, fake]);
    try { return await fn(); } finally { await exec('window.__TAURI__.core = window.__core'); }
  };
  const ops = () => exec('return window.__ops.splice(0)');
  /** What a click asks the backend, by the end of the turn it is handled in: the reads of that click and of nothing
   * else (a background refresh draws in a turn of its own). */
  const asks = (css) => exec("return new Promise((done) => { window.__ops.length = 0; document.querySelector(arguments[0]).click(); setTimeout(() => done(window.__ops.slice()), 0); })", [css]);
  /** The pane and the list beside it, as measured. */
  const paneFacts = () => exec(`const q = (s) => document.querySelector(s), p = q('#pane'), sheet = q('#sheet'), body = q('#sheet > .page-body'), z = document.documentElement.currentCSSZoom || 1, at = document.activeElement.closest('[data-sel]');
    const box = (e) => e.getBoundingClientRect();
    return { hash: location.hash, pane: !!p, role: p && p.getAttribute('role'), label: p && p.getAttribute('aria-label'), width: p && Math.round(box(p).width / z), position: p && getComputedStyle(p).position,
      edges: p && [Math.round(box(sheet).right - box(p).right), Math.round(box(p).top - box(sheet).top), Math.round(box(sheet).bottom - box(p).bottom)],
      list: Math.round(box(body).width / z), sheet: Math.round(box(sheet).width / z), current: [...document.querySelectorAll('#sheet [data-sel][aria-current]')].map((r) => r.dataset.sel), focus: at ? at.dataset.sel : document.activeElement.id || document.activeElement.tagName,
      primaries: [...document.querySelectorAll('#sheet .btn.primary')].filter((b) => box(b).width).map((b) => (b.closest('#pane') ? 'pane:' : 'page:') + b.textContent.trim()) };`);
  /** What the pane must measure on every page: no text under 12 px, no control under 28 px, nothing wider than the
   * pane, and nothing of it above its own top edge. */
  const paneRules = () => exec(`const p = document.getElementById('pane'), z = document.documentElement.currentCSSZoom || 1, small = [], low = [], above = [];
    for (const el of p.querySelectorAll('*')) {
      const r = el.getBoundingClientRect();
      if (!r.width || !r.height) continue;
      if ([...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim()) && parseFloat(getComputedStyle(el).fontSize) < 12) small.push(el.tagName + '.' + (el.className.baseVal ?? el.className) + ' ' + el.textContent.trim().slice(0, 20));
      if (el.matches('button, a[href], input, select') && Math.round(r.height / z) < 28) low.push((el.dataset.testid || el.className) + ' ' + r.height / z);
      if (r.top + p.scrollTop * z < p.getBoundingClientRect().top - 1) above.push(el.dataset.testid || el.className);
    }
    return { small, low, wide: p.scrollWidth > p.clientWidth, above };`);
  /** What a click on a row asks the backend, by the time the page has drawn the address with the selection in it. */
  const selectAsks = (css) => exec("return new Promise((done) => { window.__ops.length = 0; addEventListener('hashchange', () => setTimeout(() => done(window.__ops.slice()), 0), { once: true }); document.querySelector(arguments[0]).click(); })", [css]);
  const gone = (what) => waitFor(async () => !(await exec("return !!document.getElementById('pane')")), what);
  const hashIs = (h, what) => waitFor(async () => (await exec('return location.hash')) === h, what + ' (' + h + ')');
  const SES = 'claude-main/s-main-1', SES_HASH = '#sessions?sel=claude-main%2Fs-main-1';

  await test('detail pane, sessions: a click selects the row, puts sel in the address and shows the session beside the drawn columns; the arrows move the selection; Back clears it; Esc closes the pane and the rows are the 1.10 rows again; Enter, a double click and Open transcript open the full page', async () => {
    await exec("localStorage.removeItem('hm.pane.sessions')");
    await nav('#home');
    await nav('#sessions');
    await drawn('sessions');
    await $('[data-testid=session-s-main-1]');
    const rowOf = () => exec("const r = document.querySelector('[data-testid=session-s-main-1]'); return { cells: r.children.length, act: !!r.querySelector('.a-act [data-act=resume]'), where: !!r.querySelector('.a-where'), mail: !!r.querySelector('.a-acct small'), head: document.querySelector('.a-lh').children.length, primary: document.querySelector('[data-testid=session-new]').classList.contains('primary') }");
    const before = await rowOf();
    assert.ok(before.act && before.where && before.primary, 'the 1.10 row, with its Resume: ' + JSON.stringify(before));
    await withOps(async () => {
      assert.deepEqual(await selectAsks('[data-testid=read-s-main-1]'), ['transcript:s-main-1', 'session_stats:s-main-1'], 'selecting a row reads the session for the pane, by the reads its page uses, and not the list again');
      await $('#pane');
      await hasText('[data-testid=pane-reply]', 'What changed');
    });
    const f = await paneFacts();
    assert.equal(f.hash, SES_HASH, 'the selection is in the address');
    assert.deepEqual([f.role, f.label, f.width, f.position], ['complementary', 'Longhorn backup target for staging', 380, 'static'], 'the pane: a region named by the session, 380 px, beside the list');
    assert.deepEqual([f.list, f.edges], [f.sheet - 380, [0, 0, 0]], 'from under the top bar to the bottom, the list in the rest');
    assert.deepEqual([f.current, f.focus], [[SES], SES], 'the row is the current one, and the focus stays on it, in the list');
    assert.deepEqual(f.primaries, ['pane:Resume'], 'one primary on the screen: the pane\'s');
    const open = await rowOf();
    assert.deepEqual([open.act, open.where, open.mail, open.primary, open.head, open.cells], [false, false, false, false, before.head - 1, before.cells - 1], 'beside the pane: the drawn columns, no action of its own, New session a secondary');
    // what the pane says: the state, the account with its tool's icon, the four tiles, the last steps with their marks, the last reply
    await hasText('[data-testid=pane-state]', 'ended');
    await hasText('[data-testid=pane-state]', '1 failed');
    assert.ok(await exec("return !!document.querySelector('[data-testid=pane-account] img[src*=claude]') && !!document.querySelector('#pane .pane-top img[src*=claude]')"), 'the tool\'s own icon, in the head and by the account');
    const p = await exec(`const q = (s) => [...document.querySelectorAll(s)]; return { tiles: q('#pane .pane-tile').map((t) => t.innerText.replace(/\\s+/g, ' ').trim()), marks: q('#pane .pn-mark').map((m) => m.getAttribute('aria-label')), steps: document.querySelector('[data-testid=pane-steps]').innerText.replace(/\\s+/g, ' '),
      kv: document.querySelector('#pane .pane-kv').innerText.replace(/\\s+/g, ' '), acts: q('#pane .pane-acts > *').map((b) => b.textContent.trim()), open: document.querySelector('[data-testid=pane-open]').getAttribute('href') };`);
    assert.deepEqual(p.tiles.map((t) => t.replace(/ [\d.,k%M—]+$/, '')), ['Tokens', 'Read from cache', 'Failed commands', 'Files changed'], 'four tiles: ' + p.tiles);
    assert.ok(p.tiles[2].endsWith(' 1') && p.tiles[3].endsWith(' 1'), 'one failed command, one file changed: ' + p.tiles);
    assert.deepEqual(p.marks, ['done', 'failed'], 'each step with its mark, named');
    assert.ok(p.steps.includes('Edit apps/longhorn/values-staging.yaml +2 −1') && p.steps.includes('Bash argocd app sync longhorn-staging failed'), 'the last steps, newest last: ' + p.steps);
    assert.ok(/Account claude-main Tool Claude Code Model claude-opus-5-5 Started Today \d\d:\d\d · ended \d\d:\d\d · \d+ m/.test(p.kv), 'the facts: ' + p.kv);
    assert.deepEqual([p.acts, p.open], [['Resume', 'Open transcript', 'Export as Markdown', '⋯'], '#session?profile=claude-main&id=s-main-1'], 'the primary with Open transcript, then Export and ⋯');
    assert.deepEqual(await paneRules(), { small: [], low: [], wide: false, above: [] }, 'nothing under 12 px, no control under 28 px, nothing wider than the pane');
    // ⋯ holds what the session page's ⋯ holds, and Esc closes the menu, not the pane
    await click('[data-testid=pane-menu]');
    assert.deepEqual(await exec("return [...document.querySelectorAll('.menu [role=menuitem]')].map((b) => b.textContent.trim())"), ['Open sessions in Windows Terminal', 'Open sessions in a console window', 'Open the log file folder'], 'the terminal and the log folder behind ⋯');
    await page.keyboard.press('Escape');
    await waitFor(async () => !(await exec("return !!document.querySelector('.menu')")), 'the menu closed');
    assert.ok((await paneFacts()).pane, 'and the pane stayed');
    await shot('21-sessions-pane-light');
    for (const [brand, theme] of [['hm', 'dark'], ['vg', 'light']]) {
      await exec('document.documentElement.dataset.brand = arguments[0]; document.documentElement.dataset.theme = arguments[1]', [brand, theme]);
      await sleep(300);
      await shot(`21-sessions-pane-${brand === 'vg' ? 'vektragrid-' : ''}${theme}`);
    }
    await exec("document.documentElement.dataset.brand = 'hm'; document.documentElement.dataset.theme = 'light'");
    // Export as Markdown works from the pane (the native save dialog cannot be driven; it is answered with a path)
    const exported = path.join(sb, 'export-pane.md');
    await exec('window.__TAURI__.dialog.save = async (o) => { window.__saveAs = o.defaultPath; return arguments[0]; }', [exported]);
    await click('[data-testid=pane-export]');
    await waitFor(() => fs.existsSync(exported), 'the export file');
    assert.ok(fs.readFileSync(exported, 'utf8').startsWith('# Longhorn backup target for staging'), 'the session, as Markdown');
    assert.equal(await exec('return window.__saveAs'), 'Longhorn backup target for staging.md', 'offered under the title of the session shown');
    // Down and Up move the selection (and the focus with it) without a step in the history
    await page.focus('[data-testid=read-s-main-1]');
    const len = await exec('return history.length');
    await page.keyboard.press('ArrowDown');
    await waitFor(async () => { const g = await paneFacts(); return g.current.length === 1 && g.current[0] !== SES && g.focus === g.current[0] && g.label !== 'Longhorn backup target for staging'; }, 'Down selects the next row, shows it and takes the focus there');
    await page.keyboard.press('ArrowUp');
    await waitFor(async () => { const g = await paneFacts(); return g.current[0] === SES && g.focus === SES; }, 'Up selects the row above');
    assert.equal(await exec('return history.length'), len, 'moving the selection adds no step to the history');
    // Back clears the selection
    await exec('history.back()');
    await hashIs('#sessions', 'Back clears the selection');
    await gone('the pane goes with the selection');
    assert.deepEqual(await rowOf(), before, 'the 1.10 rows');
    // Esc closes the pane: the selection stays in the address, the rows are the 1.10 rows, the list has the full width
    await click('[data-testid=read-s-main-1]');
    await $('#pane');
    await page.keyboard.press('Escape');
    await gone('Esc closes the pane');
    const closed = await paneFacts();
    assert.deepEqual([closed.hash, closed.current], [SES_HASH, [SES]], 'the selection stays');
    assert.ok(closed.sheet - closed.list <= 16, 'the list takes the full width again: ' + closed.list + ' of ' + closed.sheet);
    assert.deepEqual(await rowOf(), before, 'and the rows are the 1.10 rows again');
    // closed is remembered for the page: the address with a selection opens no pane, until a click on a row
    assert.equal(await exec("return localStorage.getItem('hm.pane.sessions')"), '0');
    await nav('#home');
    await nav(SES_HASH);
    await drawn('sessions');
    assert.deepEqual([(await paneFacts()).pane, (await paneFacts()).current], [false, [SES]], 'closed stays closed, the row stays selected');
    await click('[data-testid=session-s-main-1] .num');
    await $('#pane');
    assert.equal(await exec("return localStorage.getItem('hm.pane.sessions')"), '1', 'a click on a row opens it again');
    // the pane's ✕ closes it too, and the focus goes back to the row
    await page.focus('[data-testid=pane-close]');
    await page.keyboard.press('Enter');
    await gone('✕ closes the pane');
    assert.equal((await paneFacts()).focus, SES, 'the focus is back on the row');
    await click('[data-testid=read-s-main-1]');
    await $('#pane');
    // a reload keeps the selection and the pane
    await page.reload();
    await waitFor(() => exec("return !!document.getElementById('pane') && !!document.querySelector('[data-testid=pane-reply]')"), 'the pane after a reload', 30000);
    assert.deepEqual([(await paneFacts()).hash, (await paneFacts()).label], [SES_HASH, 'Longhorn backup target for staging'], 'the same session');
    // Enter on the selected row, a double click, and Open transcript go to the full page; Back returns to the pane
    await page.focus('[data-testid=read-s-main-1]');
    await page.keyboard.press('Enter');
    await hashIs('#session?profile=claude-main&id=s-main-1', 'Enter on the selected row opens the session');
    await hasText('[data-testid=session-title]', 'Longhorn');
    await exec('history.back()');
    await $('#pane');
    await dbl('[data-testid=session-s-main-1] .num');
    await hashIs('#session?profile=claude-main&id=s-main-1', 'a double click opens the session');
    await exec('history.back()');
    await click('[data-testid=pane-open]');
    await hashIs('#session?profile=claude-main&id=s-main-1', 'Open transcript opens the session');
    await hasText('[data-testid=session-title]', 'Longhorn');
  });

  await test('detail pane, sessions: its one primary per state: Follow live for a session running here, Resume here or the reason for one from another computer (which says where the rest is), the reason for an archived one', async () => {
    // the sandbox has no running console, no second computer and no archive here: the list's answer says so for four sessions
    await exec("localStorage.removeItem('hm.pane.sessions')");
    await withOps(async () => {
      await nav('#home');
      await nav('#sessions');
      await drawn('sessions');
      const pick = async (id) => { await ops(); await click(`[data-testid="read-${id}"]`); await waitFor(async () => ((await paneFacts()).current[0] || '').endsWith('/' + id) && (await paneFacts()).pane, id + ' in the pane'); };
      const pane = () => exec(`const q = (t) => document.querySelector('#pane [data-testid=' + t + ']'), tx = (t) => (q(t) ? q(t).textContent.trim() : null);
        return { primary: tx('pane-primary'), reason: tx('pane-reason'), state: tx('pane-state'), rest: tx('pane-rest'), export: !!q('pane-export'), open: q('pane-open').getAttribute('href') };`);
      // running here: Follow live, and the strip says it is live and followed, with the time of its last step
      await pick('s-main-1');
      const live = await pane();
      assert.equal(live.primary, 'Follow live');
      assert.ok(/^live · following\s*last step \d\d:\d\d$/.test(live.state), 'the state strip: ' + live.state);
      assert.deepEqual((await paneFacts()).primaries, ['pane:Follow live'], 'one primary');
      assert.equal(await exec("return document.querySelector('[data-testid=pane-primary]').getAttribute('href')"), '#session?profile=claude-main&id=s-main-1', 'which opens its page, following');
      await click('[data-testid=pane-menu]');
      assert.deepEqual(await exec("return [...document.querySelectorAll('.menu [role=menuitem]')].map((b) => b.textContent.trim().replace(/Resume in .*/, 'Resume in …'))"),
        ['Resume in …', 'Open sessions in Windows Terminal', 'Open sessions in a console window', 'Open the log file folder'], 'Resume, the terminal and the log folder behind ⋯, as on its page');
      await page.keyboard.press('Escape');
      // from another computer, resumable here: Resume here; what the index knows is shown, the rest is said to be there, and nothing is fetched
      await pick('cx-1');
      const far = await pane();
      assert.deepEqual([far.primary, far.export], ['Resume here', false]);
      assert.ok(far.rest.includes('in the transcript on LAPTOP'), 'where the rest is: ' + far.rest);
      assert.ok(far.open.includes('device=dev-laptop'), 'Open transcript opens the other computer\'s: ' + far.open);
      await sleep(400);
      assert.ok(!(await ops()).some((o) => o.startsWith('transcript')), 'its transcript is not fetched for the pane');
      await hasText('#pane .pane-top', 'LAPTOP');
      // from another computer, not resumable here: the reason in the primary's place
      await pick('ses_oc1');
      const no = await pane();
      assert.deepEqual([no.primary, no.reason, (await paneFacts()).primaries], [null, 'running on LAPTOP', []], 'the reason, and no primary');
      assert.ok((await exec("return document.querySelector('[data-testid=pane-reason]').title")).includes('close it there first'), 'the whole reason on hover');
      // archived: read only
      await pick('agy-conv-1');
      const old = await pane();
      assert.deepEqual([old.primary, old.reason], [null, 'archived: it can be read, not resumed']);
      assert.ok(old.state.startsWith('archived · ended'), old.state);
    }, `if (op === 'sessions') for (const g of r.groups) for (const s of g.sessions) {
        if (s.id === 's-main-1') s.open = true;
        if (s.id === 'cx-1') Object.assign(s, { device: 'dev-laptop', device_name: 'LAPTOP', resumable: true });
        if (s.id === 'ses_oc1') Object.assign(s, { device: 'dev-laptop', device_name: 'LAPTOP', resumable: 'running on LAPTOP: close it there first' });
        if (s.id === 'agy-conv-1') s.archived = true;
      }`);
    await nav('#home');
  });

  await test('detail pane, sessions: the pane of a running session follows it as its tool writes, while the list is not drawn again', async () => {
    const file = path.join(home, '.claude', 'projects', 'p', 's-pane-live.jsonl');
    const t0 = new Date().toISOString();
    const pair = (i, q, a) => [{ type: 'user', sessionId: 's-pane-live', cwd: repo, timestamp: t0, message: { role: 'user', content: q } },
      { type: 'assistant', sessionId: 's-pane-live', cwd: repo, timestamp: t0, message: { id: 'pl' + i, model: 'claude-opus-5-5', usage: { input_tokens: 5, output_tokens: 5 }, content: [{ type: 'text', text: a }] } }];
    try {
      write(file, jl(pair(0, 'Watch the pane', 'First answer.')));
      await exec("localStorage.removeItem('hm.pane.sessions')");
      // (the sandbox has no running console, so the list's answer says this session is open)
      await withOps(async () => {
        await nav('#home');
        await nav('#sessions');
        await waitFor(() => exec("return !!document.querySelector('[data-testid=session-s-pane-live]')"), 'the new session in the list', 20000);
        await click('[data-testid=read-s-pane-live]');
        await hasText('[data-testid=pane-primary]', 'Follow live');
        await hasText('[data-testid=pane-reply]', 'First answer.');
        // every row is marked: a list drawn again from nothing would lose the marks
        await exec("for (const r of document.querySelectorAll('.a-srow')) r.__kept = true; document.querySelector('.a-smain').__kept = true");
        await waitFor(async () => (await ops()).includes('file_stamp'), 'the live watch looks at its log file');
        await sleep(2200);
        await ops();
        fs.appendFileSync(file, jl(pair(1, 'And now?', 'Written while you watched.')));
        await hasText('[data-testid=pane-reply]', 'Written while you watched.');
        const asked = await ops();
        assert.ok(asked.includes('file_stamp') && asked.includes('transcript:s-pane-live'), 'the watch saw the file change and the pane read the transcript again: ' + asked);
        assert.ok(await exec("return document.querySelector('.a-smain').__kept === true && [...document.querySelectorAll('.a-srow')].every((r) => r.__kept)"), 'the list and its rows are the ones that were there');
        assert.equal((await paneFacts()).current[0], 'claude-main/s-pane-live', 'the selection stayed');
      }, "if (op === 'sessions') for (const g of r.groups) for (const s of g.sessions) if (s.id === 's-pane-live') s.open = true;");
    } finally { fs.rmSync(file, { force: true }); await nav('#home'); }
  });

  await test('detail pane, accounts: a click selects the account and puts sel in the address; the pane has ONE primary per state (Start; Log in, with Start disabled and the reason), the row\'s whole menu, its tabs and Remove at the foot; Esc gives the 1.10 rows back; Enter and Open go to the account\'s page', async () => {
    await exec("localStorage.removeItem('hm.pane.accounts')");
    await reopenAccounts();
    const rowOf = () => exec(`const r = document.querySelector('[data-testid=row-codex-main]'), z = document.documentElement.currentCSSZoom || 1;
      return { cells: r.children.length, acts: [...r.querySelectorAll('.acts button')].map((b) => b.textContent.trim()), spark: !!r.querySelector('.spark'), height: Math.round(r.getBoundingClientRect().height / z),
        head: [...document.querySelectorAll('.page-head .actions .btn')].map((b) => b.textContent.trim() + (b.classList.contains('primary') ? ' (primary)' : '')), cols: [...document.querySelectorAll('.acct-cols .label')].map((l) => l.textContent) };`);
    const before = await rowOf();
    assert.deepEqual([before.cells, before.acts, before.spark, before.height, before.cols], [7, ['Start', '⋯'], true, 56, []], 'the 1.10 row: ' + JSON.stringify(before));
    assert.equal(before.head.at(-1), 'Add account (primary)');
    // the row's menu, to compare with the pane's
    await click('[data-testid=menu-codex-main]');
    await $('.menu.acct-menu');
    const rowMenu = await exec("return [...document.querySelectorAll('.menu button')].map((b) => b.textContent.trim())");
    await page.keyboard.press('Escape');
    let f;
    await withOps(async () => {
      assert.deepEqual(await selectAsks('[data-testid=row-codex-main] .who'), ['detail:codex-main'], 'selecting a row reads the account for the pane, and not the list again');
      await $('#pane');
      await $('[data-testid=pane-recent] .pane-mini');
      f = await paneFacts();
      await shot('22-accounts-pane-light');
      // in a low window the pane scrolls on its own, and its head (the name, with its pills under it when they have
      // no room beside it) stays whole inside it
      await page.setViewportSize({ width: 1280, height: 600 });
      await waitFor(async () => (await exec("const p = document.getElementById('pane'); return p.scrollHeight > p.clientHeight")), 'the pane scrolls on its own in a low window');
      assert.deepEqual(await paneRules(), { small: [], low: [], wide: false, above: [] }, 'in a low window nothing of the pane is cut');
      await page.setViewportSize({ width: 1280, height: 800 });
      // a tab of the pane is drawn from what was read
      assert.deepEqual(await asks('[data-testid=pane-tab-sessions]'), [], 'the Sessions tab asks the backend nothing');
      await $('[data-testid=pane-tabbody] [data-testid=session-cx-1]');
      assert.deepEqual(await asks('[data-testid=pane-tab-projects]'), [], 'nor does the Projects tab');
      await hasText('[data-testid=pane-tabbody]', 'platform-gitops');
      assert.deepEqual(await paneRules(), { small: [], low: [], wide: false, above: [] }, 'the Projects tab fits the pane');
      const tabs = await exec(`return [...document.querySelectorAll('#pane .pane-tabs > *')].map((t) => [t.tagName, t.textContent.trim(), t.getAttribute('aria-current'), getComputedStyle(t).textDecorationLine, t.getAttribute('href')])`);
      assert.deepEqual(tabs, [['BUTTON', 'Overview', null, 'none', null], ['BUTTON', 'Sessions', null, 'none', null], ['BUTTON', 'Projects', 'true', 'none', null], ['A', 'Settings', null, 'underline', '#account?name=codex-main&tab=settings']],
        'Overview, Sessions and Projects are tabs of the pane; Settings is a link to the account\'s page, drawn as a link');
      await click('[data-testid=pane-tab-overview]');
      await $('[data-testid=pane-limits]');
    });
    assert.deepEqual([f.hash, f.role, f.label, f.width, f.position, f.list], ['#accounts?sel=codex-main', 'complementary', 'codex-main', 380, 'static', f.sheet - 380], 'the selection in the address, the pane a region named by the account, beside the list');
    assert.deepEqual([f.current, f.focus, f.primaries], [['codex-main'], 'codex-main', ['pane:Start']], 'the row current and focused; one primary on the screen, the pane\'s Start');
    const open = await rowOf();
    assert.deepEqual([open.cells, open.acts, open.spark, open.height, open.cols, open.head], [5, [], false, 44, ['Account', 'Limits left', '7 days', 'State'], ['Export CSV', 'Add account']],
      'beside the pane: the drawn columns on 44 px rows, no action of its own, Add account a secondary');
    assert.ok((await text('[data-testid=state-codex-main]')).trim().length > 3, 'the state in words');
    const facts = await rowFacts();
    assert.deepEqual([[...new Set(facts.heights)], facts.wrapped, facts.overlap], [[44], [], []], 'every row 44 px, nothing wrapped, no column into the next');
    const p = await exec(`const q = (t) => document.querySelector('#pane [data-testid=' + t + ']'), tx = (t) => (q(t) ? q(t).innerText.replace(/\\s+/g, ' ').trim() : null);
      return { who: tx('pane-who'), limits: tx('pane-limits'), runway: q('pane-runway').textContent.replace(/\\s+/g, ' ').trim(), cli: tx('pane-cli'), folder: tx('pane-folder-path'), remove: [tx('pane-remove'), getComputedStyle(q('pane-remove')).color], why: tx('pane-why'), next: !!q('pane-next'),
        icon: !!document.querySelector('#pane .pane-top img[src*=codex]'), acts: [...document.querySelectorAll('#pane .pane-acts > *')].map((b) => b.textContent.trim()), tiles: [...document.querySelectorAll('#pane .pane-tile')].map((t) => t.innerText.replace(/\\s+/g, ' ').trim()),
        subs: [...document.querySelectorAll('#pane .pane-sub > :first-child')].map((s) => s.textContent), pills: [...document.querySelectorAll('#pane .pane-top .pill')].map((x) => x.textContent) };`);
    assert.ok(p.icon && p.who.startsWith('alex@example.org · ChatGPT'), 'the tool\'s icon, the login and the plan: ' + p.who);
    assert.ok(/^79% week Limits left resets in Week 79% /.test(p.limits), 'the ring with the limit windows, what is left and when each resets: ' + p.limits);
    assert.ok(p.runway.startsWith('Reset runway') && p.runway.includes('week window, now to +7 d') && p.runway.includes('resets ') && p.runway.includes('Now+2 d+4 d+6 d+7 d'), 'the runway lane, from now to +7 d with its reset: ' + p.runway);
    assert.deepEqual(p.subs, ['Reset runway', 'Last 7 days', 'Recent sessions'], 'the sections, in the drawing\'s order');
    assert.ok(/ tokens$/.test(p.tiles[0]) && /^(≈ \$[\d.,]+|—) at API list prices$/.test(p.tiles[1]), 'the range\'s tokens and ≈ cost: ' + p.tiles);
    assert.deepEqual([p.cli, p.acts], ['hrm run codex-main', ['Start', 'Start in folder…', '⋯']], 'the command line, and Start, Start in folder… and ⋯');
    assert.ok(p.folder.endsWith('.codex'), 'its folder: ' + p.folder);
    assert.deepEqual(p.remove, ['Hide main profile…', 'rgb(180, 40, 24)'], 'at the foot, in the danger colour (a main profile is hidden, not removed)');
    assert.ok(p.pills.includes('main') && p.next === p.pills.includes('Use next') && (!p.next || p.why.startsWith('Why this one:')), 'main, and Use next with its reason when it is the recommendation: ' + p.pills + ' / ' + p.why);
    assert.deepEqual(await paneRules(), { small: [], low: [], wide: false, above: [] }, 'nothing under 12 px, no control under 28 px, nothing wider than the pane');
    // the pane's ⋯ is the row's whole menu
    await click('[data-testid=pane-menu]');
    await $('.menu.acct-menu');
    assert.deepEqual(await exec("return [...document.querySelectorAll('.menu button')].map((b) => b.textContent.trim())"), rowMenu, 'the same menu as the row\'s');
    await page.keyboard.press('Escape');
    for (const [brand, theme] of [['hm', 'dark'], ['vg', 'dark']]) {
      await exec('document.documentElement.dataset.brand = arguments[0]; document.documentElement.dataset.theme = arguments[1]', [brand, theme]);
      await sleep(300);
      await shot(`22-accounts-pane-${brand === 'vg' ? 'vektragrid-' : ''}${theme}`);
    }
    await exec("document.documentElement.dataset.brand = 'hm'; document.documentElement.dataset.theme = 'light'");
    // a logged-out account (the sandbox's claude-main): Log in is the primary, Start is disabled and says why
    await click('[data-testid=row-claude-main] .who');
    await waitFor(async () => (await paneFacts()).label === 'claude-main', 'claude-main in the pane');
    const out = await exec(`const q = (t) => document.querySelector('#pane [data-testid=' + t + ']'), s = q('pane-start');
      return { primary: q('pane-primary').textContent.trim(), act: q('pane-primary').dataset.act, start: [s.textContent.trim(), s.disabled, s.title], pill: q('pane-out').textContent, why: q('pane-why').textContent.trim(), cli: q('pane-cli').textContent, limits: q('pane-limits').innerText.replace(/\\s+/g, ' ').trim(), runway: !!q('pane-runway') };`);
    assert.deepEqual([out.primary, out.act, out.start, out.pill], ['Log in', 'login', ['Start', true, 'Not logged in: log in first'], 'Logged out'], 'Log in is the primary; Start is disabled with the reason in its title');
    assert.deepEqual([out.why, out.cli, out.runway], ['Not logged in. Plan and limits are read after the first login.', 'hrm login claude-main', false], 'the banner says why, the command line is the one that works');
    assert.ok(out.limits.includes('5-hour — log in') && out.limits.includes('Week — log in'), 'unread limits are —, with the reason: ' + out.limits);
    assert.deepEqual((await paneFacts()).primaries, ['pane:Log in'], 'one primary');
    await page.keyboard.press('Escape');
    await gone('Esc closes the pane');
    assert.deepEqual([(await paneFacts()).hash, (await paneFacts()).current], ['#accounts?sel=claude-main', ['claude-main']], 'the selection stays in the address');
    assert.deepEqual(await rowOf(), before, 'and the rows are the 1.10 rows again');
    // Back clears the selection (the second selection replaced the first in the history)
    await exec('history.back()');
    await hashIs('#accounts', 'Back clears the selection');
    // an expired login and a limit used up say so in the banner
    await withOps(async () => {
      await nav('#home');
      await nav('#accounts?sel=codex-main');
      await drawn('accounts');
      await click('[data-testid=row-codex-main] .who');
      await hasText('[data-testid=pane-why]', 'Week used up. Back ');
      assert.equal((await text('[data-testid=state-codex-main]')).replace(/\s+/g, ' ').split(' back ')[0], 'Used up', 'and the row\'s state does');
      // three limit windows beside the pane: each on the row with its short name whole, each on its own line in the pane
      const lims = await exec("return [...document.querySelectorAll('[data-testid=row-codex-main] .lim .t')].map((t) => [t.textContent, t.firstElementChild.scrollWidth > t.firstElementChild.clientWidth])");
      assert.deepEqual(lims, [['5h100%', false], ['Wk0%', false], ['Fable88%', false]], 'the row\'s three limits, no name cut');
      assert.ok(/5-hour 100% .* Week 0% .* Week · Fable 88% /.test((await text('[data-testid=pane-limits]')).replace(/\s+/g, ' ')), 'the pane\'s three windows: ' + (await text('[data-testid=pane-limits]')).replace(/\s+/g, ' '));
      const facts = await rowFacts();
      assert.deepEqual([[...new Set(facts.heights)], facts.wrapped, facts.overlap], [[44], [], []], 'and the rows still hold: 44 px, nothing wrapped, no column into the next');
    }, `if (op === 'state') { const soon = Date.now() + 3600e3, q = r.profiles.find((x) => x.name === 'codex-main').quota, t = q.tiers.find((x) => x.id === 'week'); t.used = 100; t.resets_at = soon;
      q.tiers.unshift({ id: 'session', label: '5-hour session', used: 0, resets_at: soon }); q.tiers.push({ id: 'week:fable', label: 'Week · Fable', used: 12, resets_at: soon }); }`);
    await withOps(async () => {
      await nav('#home');
      await nav('#accounts?sel=codex-main');
      await drawn('accounts');
      await hasText('[data-testid=pane-why]', 'Login expired. Plan and limits not read since ');
      await hasText('[data-testid=pane-primary]', 'Log in');
      assert.equal(await exec("return document.querySelector('[data-testid=pane-start]').title"), 'Its login has expired: log in first');
    }, "if (op === 'state') r.profiles.find((x) => x.name === 'codex-main').identity.needs_login = true;");
    // Enter on the selected row and the pane's Open go to the account's page
    await reopenAccounts();
    await click('[data-testid=row-codex-main] .who');
    await $('#pane');
    await page.keyboard.press('Enter');
    await hashIs('#account?name=codex-main', 'Enter on the selected row opens the account');
    await exec('history.back()');
    await click('[data-testid=pane-open]');
    await hashIs('#account?name=codex-main', 'Open opens the account');
    await hasText('[data-testid=detail-name]', 'codex-main');
  });

  await test('detail pane: at Large text the pane lies over the list instead of squeezing it, on Sessions and on Accounts', async () => {
    const size = (v) => exec("return window.__TAURI__.core.invoke('call', { op: 'settings_set', args: { key: 'text_size', value: arguments[0] } }).then(() => import('./core.js')).then((c) => c.loadState())", [v]);
    await exec("localStorage.removeItem('hm.pane.sessions'); localStorage.removeItem('hm.pane.accounts')");
    try {
      await nav('#home');
      await nav(SES_HASH);
      await drawn('sessions');
      await $('#pane');
      const usual = await paneFacts();
      assert.deepEqual([usual.position, usual.list], ['static', usual.sheet - 380], 'at the usual size: beside the list');
      await size('large');
      await waitFor(async () => (await exec("return +getComputedStyle(document.documentElement).zoom")) === 1.12, 'Large');
      for (const [hash, name] of [[SES_HASH, 'sessions'], ['#accounts?sel=codex-main', 'accounts']]) {
        await nav(hash);
        await drawn(name);
        await $('#pane');
        const big = await paneFacts();
        assert.ok(big.sheet - 380 < 560, name + ': too narrow for both: ' + big.sheet);
        assert.deepEqual([big.position, big.list, big.width, big.edges], ['absolute', big.sheet, 380, [0, 0, 0]], name + ': the pane lies over the list from the right, and the list keeps its width');
        assert.deepEqual(await paneRules(), { small: [], low: [], wide: false, above: [] }, name + ': the pane holds at this size');
        await shot(`23-${name}-pane-large`);
        await page.keyboard.press('Escape');
        await gone(name + ': Esc closes it');
        await exec('localStorage.removeItem(arguments[0])', ['hm.pane.' + name]);
      }
    } finally { await size('comfortable'); await nav('#home'); }
  });

  await test('the tray popup loads the limits alone: none of the Sessions, Projects, Computers, resume or restore modules; Accounts still exports the shared names', async () => {
    // the popup's own page, loaded in a frame of this window: what it fetched is what its window loads
    const loaded = await exec(`return new Promise((done, fail) => { const f = document.createElement('iframe'); f.hidden = true; f.src = 'limits.html';
      f.onload = () => setTimeout(() => { const names = f.contentWindow.performance.getEntriesByType('resource').map((e) => e.name.split('/').pop().split('?')[0]); f.remove(); done(names); }, 800);
      f.onerror = () => fail(new Error('limits.html did not load')); document.body.append(f); })`);
    for (const need of ['popup.js', 'limits.js', 'core.js', 'lib.js']) assert.ok(loaded.includes(need), need + ' is loaded: ' + loaded);
    for (const none of ['sessions.js', 'projects.js', 'computers.js', 'resume.js', 'restore.js', 'accounts.js', 'pane.js']) assert.ok(!loaded.includes(none), none + ' is not loaded by the popup: ' + loaded);
    const missing = await exec("return import('./accounts.js').then((a) => ['TOOL', 'money', 'fmtDuration', 'fmtAbsoluteReset', 'showUsed', 'acctView', 'accountViews', 'eligible', 'w5of', 'wkof', 'metricsOf', 'renderRingSvg', 'winTrack', 'trayHtml', 'drawTray'].filter((n) => a[n] === undefined))");
    assert.deepEqual(missing, [], 'the names other modules import from accounts.js are still there');
    assert.ok(await exec("return Promise.all([import('./accounts.js'), import('./limits.js')]).then(([a, l]) => a.metricsOf === l.metricsOf && a.drawTray === l.drawTray)"), 'and they are the limits module\'s');
  });

  await test('a click that only changes what is shown asks the backend nothing: a step unfolded in a session, an Accounts group folded, the charts opened, a tool chosen', async () => {
    await nav('#session?profile=claude-main&id=s-main-1');
    await hasText('[data-testid=session-title]', 'Longhorn');
    await drawn('the session');
    await withOps(async () => {
      const step = '[data-testid=chat] .a-step:has(.a-bad) [data-act=step-open]';
      assert.deepEqual(await asks(step), [], 'unfolding a step reads nothing');
      await hasText('[data-testid=chat]', 'connection closed');
      assert.deepEqual(await asks('[data-testid=mode-failed]'), [], 'nor does choosing Failed');
      await waitFor(async () => !(await text('[data-testid=chat]')).includes('values-staging.yaml'), 'Failed lists only the failed command');
      assert.deepEqual(await asks('[data-testid=mode-all]'), [], 'nor All');
      await hasText('[data-testid=chat]', 'values-staging.yaml');
      assert.deepEqual(await asks(step), [], 'nor folding the step again');
      await waitFor(async () => !(await text('[data-testid=chat]')).includes('connection closed'), 'folded again');
      await nav('#accounts');
      await drawn('accounts');
      await $('[data-testid=row-codex-main]');
      const fold = '[data-testid=group-codex] [data-act=acct-fold]';
      assert.deepEqual(await asks(fold), [], 'folding a group reads nothing');
      await waitFor(async () => !(await exec("return !!document.querySelector('[data-testid=row-codex-main]')")), 'Codex folded');
      assert.deepEqual(await asks(fold), [], 'nor does opening it again');
      await $('[data-testid=row-codex-main]');
      const charts = '[data-testid=usage-panel] [data-act=acct-fold]', isOpen = () => exec("return document.querySelector(arguments[0]).getAttribute('aria-expanded')", [charts]);
      const was = await isOpen();
      assert.deepEqual(await asks(charts), [], 'nor the charts panel, opened');
      await waitFor(async () => (await isOpen()) !== was, 'the charts panel toggled');
      assert.deepEqual(await asks(charts), [], 'or folded');
      await waitFor(async () => (await isOpen()) === was, 'and back');
      assert.deepEqual(await asks('[data-testid=chip-codex]'), [], 'nor choosing a tool');
      await waitFor(async () => (await exec("return document.querySelectorAll('[data-testid=accounts-table] .tool-group-card').length")) === 1, 'only Codex');
      assert.deepEqual(await asks('[data-testid=chip-all]'), [], 'or every tool again');
    });
  });

  await test('Refresh limits reads live for every tool the core can ask (Claude Code and Antigravity) and falls back to the general refresh for the others', async () => {
    await nav('#accounts');
    await drawn('accounts');
    const kinds = await exec("return import('./core.js').then((c) => Object.fromEntries(c.S.state.profiles.map((p) => [p.name, p.harness + ' ' + p.kind])))");
    assert.deepEqual([kinds['claude-main'], kinds['agy-main'], kinds['codex-main']], ['claude subscription', 'agy subscription', 'codex subscription']);
    // (the reads themselves are answered here: the sandbox's tools have no live limits to give)
    await withOps(async () => {
      const refresh = async (name) => {
        await sleep(300);
        await ops();
        await exec("const b = document.createElement('button'); b.dataset.act = 'limits-refresh'; b.dataset.name = arguments[0]; b.hidden = true; document.body.append(b); b.click(); b.remove();", [name]);
        return waitFor(async () => { const o = (await ops()).filter((x) => x.startsWith('quota_live') || x === 'refresh'); return o.length ? o : null; }, 'a read of the limits of ' + name);
      };
      assert.deepEqual(await refresh('claude-main'), ['quota_live:claude-main'], 'Claude Code: read live');
      assert.deepEqual(await refresh('agy-main'), ['quota_live:agy-main'], 'Antigravity: read live too');
      assert.deepEqual(await refresh('codex-main'), ['refresh'], 'Codex: the general refresh');
    }, '', "if (op === 'quota_live') return { tiers: [], fetched_at: Date.now() }; if (op === 'refresh') return { new_rows: 0, quota_errors: [] };");
    await nav('#home');
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
    await waitFor(async () => !(await exec("return !!document.querySelector('#dialog-root .modal-card')")), 'the dialog closed');
    await click('[data-testid=t-mcp-fs-codex]');
    await waitFor(() => fs.readFileSync(path.join(home, '.codex', 'config.toml'), 'utf8').includes('[mcp_servers.fs]'), 'fs in config.toml');
    await shot('11-mcp');
    await waitFor(async () => (await exec(`const t = document.querySelector('[data-testid=mcp-matrix]'); return t.tHead.rows[0].cells[document.querySelector('[data-testid=t-mcp-fs-codex]').closest('td').cellIndex].title`)) === 'Codex: 1', 'the Codex count follows the tick');
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
    await waitFor(async () => exec(`return !document.querySelector('[data-testid=mcp-row-fs]') && !!document.querySelector('[data-testid=mcp-row-solo]')`), 'only solo shown');
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
    await $('[data-testid=mcp-next]:not([disabled])');
    const onPage = () => exec("return document.querySelectorAll('[data-testid=mcp-matrix] tbody tr[data-key]').length");
    assert.equal(await onPage(), 25, '25 on the first page');
    await click('[data-testid=mcp-next]');
    await waitFor(async () => (await onPage()) === 8, 'the rest on the second page');
    await select('[data-testid=mcp-per]', '50');
    await waitFor(async () => (await onPage()) === 33 && !(await exec("return !!document.querySelector('[data-testid=mcp-next]:not([disabled])')")), 'one page at 50');
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
    await $('#sk-unmanaged [data-testid=skill-row-loose]');
    await hasText('#sk-unmanaged', 'Not managed');
    await hasText('[data-testid=skill-row-loose]', 'Put here by hand');
    await click('[data-testid=skill-adopt-loose]');
    await waitFor(async () => (await exec("return !document.querySelector('#sk-unmanaged [data-testid=skill-row-loose]') && !!document.querySelector('[data-testid=skill-row-loose]')")), 'managed now');
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
    await $('[data-testid=sk-next]:not([disabled])');
    const onPage = () => exec("return document.querySelectorAll('[data-testid=skill-matrix] tbody tr[data-key]').length");
    assert.equal(await onPage(), 25, '25 on the first page');
    await click('[data-testid=sk-next]');
    await waitFor(async () => (await onPage()) < 25, 'the rest on the second page');
    await select('[data-testid=sk-per]', '50');
    await waitFor(async () => (await onPage()) > 25 && !(await exec("return !!document.querySelector('[data-testid=sk-next]:not([disabled])')")), 'one page at 50');
    await type('[data-testid=find]', 'bulk-07');
    await waitFor(async () => (await onPage()) === 1, 'the search narrows to one');
    await page.fill('[data-testid=find]', '');
    await exec(`document.getElementById('find').dispatchEvent(new Event('input'))`);
  });

  await test('skills: Discover lists the skills of each repository and installs one into the store', async () => {
    await nav('#skills');
    // a local skill for the source filter below, when this test runs on its own
    if (!(await exec("return !!document.querySelector('[data-testid=skill-row-my-skill]')"))) {
      await click('[data-testid=skill-install]');
      await type('[data-testid=sk-src]', path.join(home, 'my-skill'));
      await click('[data-testid=sk-save]');
      await $('[data-testid=skill-row-my-skill]');
    }
    await click('[data-testid=sk-tab-discover]');
    await hasText('[data-testid=discover]', 'Says hello in every language');
    await hasText('[data-testid=discover]', 'Reviews a change line by line');
    // three default repositories, one download each
    assert.equal(codeloadHits.filter((u) => u.includes('/zip/')).length, 3, 'one download per repository: ' + codeloadHits);
    await type('[data-testid=dc-search]', 'review');
    await waitFor(async () => exec(`return !document.querySelector('[data-testid=dc-hello]') && !!document.querySelector('[data-testid=dc-review]')`), 'search narrows the list');
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
    await waitFor(async () => exec(`return !document.querySelector('[data-testid=skill-row-hello]') && !!document.querySelector('[data-testid=skill-row-my-skill]')`), 'filtered to local skills');
    await select('#sk-filter', '');
    // a newer version at the source shows Update, which installs it
    write(path.join(repoDir, 'skills', 'hello', 'SKILL.md'), '---\nname: hello\ndescription: Says hello, now louder\n---\nHELLO.');
    execFileSync('pwsh', ['-NoProfile', '-Command', `Compress-Archive -Path '${repoDir}' -DestinationPath '${repoZip}' -Force`]);
    await click('[data-testid=skill-check]');
    await click('[data-testid=skill-update-hello]');
    await waitFor(() => fs.readFileSync(path.join(home, '.agents', 'skills', 'hello', 'SKILL.md'), 'utf8').includes('HELLO.'), 'updated in the store');
    await hasText('[data-testid=skill-row-hello]', 'now louder');
  });

  await test('skills: one line above the matrix, and it and the tools\' header stay in view while the rows scroll', async () => {
    await nav('#skills');
    await $('[data-testid=skill-line]');
    // enough rows to scroll, when this test runs on its own
    if ((await exec("return document.querySelectorAll('[data-testid=skill-matrix] tbody tr[data-key]').length")) < 20) {
      for (let i = 0; i < 24; i++) write(path.join(home, '.agents', 'skills', `tall-${String(i).padStart(2, '0')}`, 'SKILL.md'), `---\nname: tall-${i}\ndescription: Tall skill ${i}\n---\n`);
      await nav('#home');
      await nav('#skills');
      await click('[data-testid=skill-adopt-all]');
      await waitFor(async () => (await exec("return document.querySelectorAll('[data-testid=skill-matrix] tbody tr[data-key]').length")) >= 20, 'the skills managed');
    }
    // the store's folder, the source, the pager and the rows a page: one line at 1280, under the head
    const line = await exec(`const l = document.querySelector('[data-testid=skill-line]'), kids = [...l.children].filter((e) => e.getBoundingClientRect().width);
      return { mids: kids.map((e) => { const r = e.getBoundingClientRect(); return Math.round(r.top + r.height / 2); }), has: ['[data-testid=sk-tab-installed]', '.mono', '#sk-filter', '[data-testid=sk-next]', '[data-testid=sk-per]'].map((s) => !!l.querySelector(s)),
        between: [...document.querySelectorAll('#sheet .page-body > *')].indexOf(l), pagers: document.querySelectorAll('[data-testid=sk-next]').length, wide: l.scrollWidth <= l.clientWidth };`);
    assert.ok(Math.max(...line.mids) - Math.min(...line.mids) <= 1 && line.wide, 'one line: ' + JSON.stringify(line));
    assert.deepEqual(line.has, [true, true, true, true, true], 'Installed or Discover, the store\'s folder, the source, the pager, the rows a page');
    assert.equal(line.between, 1, 'the line comes right after the head');
    assert.equal(line.pagers, 1, 'one pager');
    // scrolled to the end of the list: the line is at the top of the page, the tools' header right under it, nothing over it
    await exec("const s = document.getElementById('sheet'); s.scrollTop = s.scrollHeight");
    const d = await exec(`const sheet = document.getElementById('sheet'), s = sheet.getBoundingClientRect(), one = document.querySelector('[data-testid=skill-line]').getBoundingClientRect();
      const ths = [...document.querySelectorAll('[data-testid=skill-matrix] thead th.b-tool-cell')], r = ths[0].getBoundingClientRect(), rows = [...document.querySelectorAll('[data-testid=skill-matrix] tbody tr')];
      const at = (th) => { const b = th.getBoundingClientRect(), e = document.elementFromPoint(b.left + b.width / 2, b.top + b.height / 2); return !!(e && e.closest('thead')); };
      return { scrolled: sheet.scrollTop, oneTop: one.top - s.top, thTop: r.top - one.bottom, seen: ths.map(at), labels: ths.map((t) => t.dataset.label), boxes: ths.every((t) => t.querySelector('[data-act=skill-bulk]').getBoundingClientRect().bottom <= s.bottom),
        lastIn: rows[rows.length - 1].getBoundingClientRect().bottom <= s.bottom + 1, firstGone: rows[0].getBoundingClientRect().bottom < r.bottom };`);
    assert.ok(d.scrolled > 300 && d.firstGone && d.lastIn, 'scrolled to the last row: ' + JSON.stringify(d));
    assert.ok(Math.abs(d.oneTop) <= 1, 'the line stays at the top of the page: ' + d.oneTop);
    assert.ok(Math.abs(d.thTop) <= 1, 'the header stays right under it: ' + d.thTop);
    assert.ok(d.seen.length >= 2 && d.seen.every(Boolean) && d.boxes, 'every tool\'s name and its tick are in view, with nothing over them: ' + d.seen);
    assert.ok(d.labels.includes('Claude Code') && d.labels.includes('Codex'), 'the columns keep their names: ' + d.labels);
    await shot('12c-skills-scrolled');
    await exec("document.getElementById('sheet').scrollTop = 0");
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
    await click('[data-testid=prompt-tabs] [data-v=codex]');
    await $('[data-testid=prompt-tabs] .active[data-v=codex]');
    await click('[data-testid=prompt-new]');
    await type('#pr-body', 'Be brief and cite files.');
    await click('[data-testid=prompt-enable]');
    await waitFor(() => fs.readFileSync(path.join(home, '.codex', 'AGENTS.md'), 'utf8') === 'Be brief and cite files.', 'AGENTS.md written');
    assert.ok(JSON.parse(hrm('prompts', 'list', '--json')).some((p) => p.body === 'hand-written instructions'), 'old content saved first');
    await shot('13-prompts');
  });

  await test('the list and setup pages keep the 1.10 rules: controls at least 28 px tall, one primary a screen, the head\'s actions on one row at 1280', async () => {
    // a prompt chosen in its editor, so System prompts is measured in the state with the most buttons
    await nav('#prompts');
    await drawn('prompts');
    if (await exec("return !!document.querySelector('[data-act=prompt-sel][data-testid]')")) { await click('[data-act=prompt-sel][data-testid]'); await $('#pr-body'); }
    for (const h of ['#sessions?days=all', '#session?profile=claude-main&id=s-main-1', '#projects?days=all', '#mcp', '#skills', '#prompts', '#routing', '#apps', '#cli']) {
      if (h !== '#prompts') { await nav(h); await drawn(h); }
      const d = await exec(`const sheet = document.getElementById('sheet'), box = (e) => e.getBoundingClientRect(), seen = (e) => box(e).width > 0 && box(e).height > 0;
        // what a pointer presses: a segmented choice counts as its frame; a tick box, a tag chip (20 px, in a line of text),
        // the tag button (drawn at 24, pressed over 30) and the shared "← Back" line are not measured here
        const low = [...sheet.querySelectorAll('button, select, summary, a[href], input:not([type=checkbox]):not([type=radio]), textarea')].filter(seen)
          .filter((e) => !e.matches('a.a-tag, .a-tagbtn, .back-chip')).map((e) => [e, box(e.closest('.seg') || e).height]).filter(([, hgt]) => hgt < 28)
          .map(([e, hgt]) => (e.dataset.testid || e.dataset.act || e.className || e.tagName) + ' ' + hgt.toFixed(1));
        const primary = new Set([...sheet.querySelectorAll('.btn.primary, .btn-primary')].filter(seen).map((b) => b.closest('.a-split') || b));
        const acts = [...sheet.querySelectorAll('.page-head .actions button')].filter(seen);
        return { low: [...new Set(low)], primary: primary.size, rows: new Set(acts.map((b) => Math.round(box(b).top + box(b).height / 2))).size, acts: acts.length, wide: sheet.scrollWidth - sheet.clientWidth, w: window.innerWidth };`);
      assert.equal(d.w, 1280);
      assert.deepEqual(d.low, [], `${h}: controls under 28 px`);
      assert.ok(d.primary <= 1, `${h}: at most one primary button on the page, not ${d.primary}`);
      assert.ok(d.rows <= 1, `${h}: the head's ${d.acts} actions on one row`);
      assert.ok(d.wide <= 0, `${h}: nothing wider than the page (${d.wide} px)`);
      // the same page in the VektraGrid theme, dark and light, for review (only the attributes the stylesheet reads
      // are changed, and put back: no setting is written)
      const name = h.slice(1).split('?')[0];
      for (const theme of ['dark', 'light']) {
        await exec("document.documentElement.dataset.brand = 'vg'; document.documentElement.dataset.theme = arguments[0]; return document.fonts.ready.then(() => 1)", [theme]);
        await sleep(150);
        await shot(`30-vg-${theme}-${name}`);
        // OpenCode's icon is black as a file: one shared rule shows it in white on a dark theme, wherever it is drawn
        const oc = await exec("const i = document.querySelector('#sheet img.ticon[src$=\"opencode.svg\"]'); return i ? getComputedStyle(i).filter : ''");
        if (oc) assert.equal(oc, theme === 'dark' ? 'invert(1)' : 'none', `${h}: the OpenCode icon on a ${theme} theme`);
      }
      await exec("document.documentElement.dataset.brand = 'hm'; document.documentElement.dataset.theme = 'light'");
    }
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
    await nav('#settings?tab=backups');
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
    await click('[data-testid=st-bkhours] [data-v="6"]');
    await waitFor(() => JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).backup_hours === 6, 'the interval saved');
    await shot('14b-backups');
  });

  await test('appearance: dark theme, limits as used, and a hidden tool left out of the pages', async () => {
    await nav('#settings');
    await click('[data-testid=st-theme] button[data-v=dark]');
    await waitFor(async () => (await exec(`return getComputedStyle(document.body).backgroundColor`)) === 'rgb(9, 17, 21)', 'the page turns to night');
    await shot('17-dark-settings');
    await nav('#accounts');
    await $('[data-testid=accounts-table]');
    await shot('17b-dark-accounts');
    await nav('#home');
    await $('[data-testid=activity]');
    await sleep(600);
    await shot('17c-dark-overview');
    await exec("document.querySelector('[data-testid=horizon]').scrollIntoView()");
    await shot('17d-dark-overview-horizon');
    await exec("document.querySelector('[data-testid=activity]').scrollIntoView()");
    await shot('17e-dark-overview-panels');
    // every other page in dark, to compare with the dark design pages
    for (const [h, n] of [['#account?name=claude-main', 'account'], ['#projects', 'projects'], ['#sessions', 'sessions'], ['#session?profile=claude-main&id=s-main-1', 'session'],
      ['#computers', 'computers'], ['#routing', 'routing'], ['#mcp', 'mcp'], ['#skills', 'skills'], ['#prompts', 'prompts'], ['#apps', 'apps'], ['#cli', 'cli']]) {
      await nav(h);
      await drawn(n);
      await sleep(500);
      await shot('18-dark-' + n);
    }
    await nav('#settings');
    await $('[data-testid=sec-appearance]');
    await click('[data-testid=st-theme] button[data-v=light]');
    await waitFor(async () => (await exec(`return getComputedStyle(document.body).backgroundColor`)) === 'rgb(242, 246, 248)', 'light again');
    // the VektraGrid theme: ivory ground, EB Garamond headings from the app's own font file; Harness again after
    await click('[data-testid=st-brand] button[data-v=vg]');
    await waitFor(async () => (await exec(`return getComputedStyle(document.body).backgroundColor`)) === 'rgb(239, 233, 222)', 'the ivory ground');
    await waitFor(async () => exec(`return document.fonts.check('20px "EB Garamond"') && getComputedStyle(document.querySelector('#sheet h1')).fontFamily.includes('EB Garamond')`), 'EB Garamond loaded for the headings');
    await shot('17f-vektragrid-settings');
    await click('[data-testid=st-brand] button[data-v=hm]');
    await waitFor(async () => (await exec(`return getComputedStyle(document.body).backgroundColor`)) === 'rgb(242, 246, 248)', 'Harness again');
    await click('[data-testid=st-quota] button[data-v=used]');
    await waitFor(() => JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).quota_display === 'used', 'saved');
    await nav('#accounts');
    // the rings and bars turn to what is used, and the page says so in words
    await hasText('[data-testid=used-note]', '% used');
    await hasText('[data-testid=row-codex-main] .a-ring', '21%');
    await hasText('[data-testid=row-codex-main] .lim', '21%');
    await nav('#settings');
    await click('[data-testid=st-quota] button[data-v=left]');
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
    await nav('#settings?tab=updates');
    await $('[data-testid=self-install]');
    await hasText('.sheet', 'Version 9.9.9 is available');
    await hasText('.sheet', 'Faster limits.');
    // the offer is out of date (the release is the version running, as after an update): installing says so and withdraws it
    latestVersion = await exec("return document.querySelector('.sheet').textContent.match(/you have (\\d+\\.\\d+\\.\\d+)/)[1]");
    await click('[data-testid=self-install]');
    await click('#confirm-ok');
    await hasText('#toast', `Harness Manager ${latestVersion} is the newest version`);
    await $('[data-testid=self-check]');
    assert.ok(await exec("return document.getElementById('settings-upd').hidden"), 'the dot is gone');
    latestVersion = '9.9.9';
    await click('[data-testid=self-check]');
    await $('[data-testid=self-install]');
  });

  await test('settings: change the terminal', async () => {
    await nav('#settings?tab=window');
    await click('[data-testid=st-term] button[data-v=console]');
    await waitFor(() => JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).terminal === 'console', 'settings saved');
    await waitFor(async () => (await exec("return document.querySelector('[data-testid=st-term] button.on')?.dataset.v")) === 'console', 'drawn with Console picked');
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

  await test('text size: Compact, Comfortable and Large zoom the whole window, the choice survives a restart of the page, and hrm sets it and refuses a wrong one', async () => {
    const saved = () => JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8'));
    const zoom = () => exec("return +getComputedStyle(document.documentElement).zoom");
    const pick = async (v, z) => {
      await click(`[data-testid=st-textsize] button[data-v=${v}]`);
      await waitFor(() => saved().text_size === v, v + ' saved');
      await waitFor(async () => (await zoom()) === z, `the window zoomed to ${z}`);
      await waitFor(async () => (await exec("return document.querySelector('[data-testid=st-textsize] button.on')?.dataset.v")) === v, v + ' shown as chosen');
    };
    try {
    await nav('#settings');
    await $('[data-testid=st-textsize]');
    assert.deepEqual(await exec("return [...document.querySelectorAll('[data-testid=st-textsize] button')].map((b) => b.textContent + ' ' + b.getAttribute('aria-pressed'))"), ['Compact false', 'Comfortable true', 'Large false'], 'three sizes, Comfortable unless chosen');
    await hasText('[data-key=setting-textsize]', 'Applies to every page.');
    assert.equal(await zoom(), 1);
    // the sidebar is 208 px of the design at every size: Large draws it over more of the window
    const share = () => exec("const r = document.querySelector('aside.rail').getBoundingClientRect(), f = document.querySelector('.frame').getBoundingClientRect(); return r.width / f.width");
    const at1 = await share();
    await pick('large', 1.12);
    assert.ok(Math.abs((await share()) / at1 - 1.12) < 0.02, 'everything is 12% larger: ' + (await share()) / at1);
    await exec("document.querySelector('[data-testid=sec-appearance]').scrollIntoView()");
    await shot('14c-settings-large');
    // a menu still opens under its button at this size, the tour still lights the part it names, and a dialog still fits the window
    await nav('#accounts');
    await click('[data-testid=menu-codex-main]');
    // (under it, or over it when the button is near the bottom of the window)
    const off = await exec("const m = document.querySelector('.menu').getBoundingClientRect(), b = document.querySelector('[data-testid=menu-codex-main]').getBoundingClientRect(), f = document.querySelector('.frame').getBoundingClientRect(); return [f.right - m.right, m.top, f.bottom - m.bottom, m.top - b.bottom, b.top - m.bottom, b.right - m.left].map(Math.round)");
    assert.ok(off[0] >= 0 && off[1] >= 0 && off[2] >= 0 && ((off[3] >= 0 && off[3] <= 8) || (off[4] >= 0 && off[4] <= 8)) && off[5] > 0, 'the menu sits by its button, inside the window: ' + off);
    await page.keyboard.press('Escape');
    await click('[data-testid=tour-start]');
    await $('.tour-spot');
    const spot = await exec("const s = document.querySelector('.tour-spot').getBoundingClientRect(), t = document.querySelector('[data-testid=add-account]').getBoundingClientRect(), z = document.documentElement.currentCSSZoom; return [s.left - t.left, s.top - t.top, s.right - t.right, s.bottom - t.bottom].map((d) => Math.round(Math.abs(d) / z))");
    assert.deepEqual(spot, [6, 6, 6, 6], 'the tour frames Add account, 6 px around it');
    await click('[data-testid=tour-skip]');
    await click('[data-testid=add-account]');
    const dlg = await exec("const m = document.querySelector('.modal-card'), r = m.getBoundingClientRect(); return [Math.round(r.top), Math.round(window.innerHeight - r.bottom), Math.round(parseFloat(getComputedStyle(m).maxHeight) * document.documentElement.currentCSSZoom - (window.innerHeight - 40 * document.documentElement.currentCSSZoom))]");
    assert.ok(dlg[0] >= 0 && dlg[1] >= 0 && Math.abs(dlg[2]) <= 1, 'the dialog is inside the window, as tall as the window allows at most: ' + dlg);
    await exec("document.querySelector('[data-act=dialog-close]').click()");
    // a restart of the page: the size is read from the settings again
    await page.reload();
    await waitFor(() => exec("return !!document.querySelector('#sheet .page-head')"), 'the window drawn again', 30000);
    await waitFor(async () => (await zoom()) === 1.12, 'Large after the restart');
    assert.ok(hrm('settings', 'text_size').includes('large'), 'the CLI reads it');
    assert.throws(() => hrm('settings', 'text_size', 'huge'), /compact, comfortable or large/, 'a wrong size is refused');
    assert.equal(saved().text_size, 'large', 'and changes nothing');
    await nav('#settings');
    await $('[data-testid=st-textsize]');
    await pick('compact', 0.92);
    assert.ok(Math.abs((await share()) / at1 - 0.92) < 0.02, 'everything is 8% smaller: ' + (await share()) / at1);
    await pick('comfortable', 1);
    } finally {
      // whatever happened above, the tests after this one run at the usual size
      await exec("return window.__TAURI__.core.invoke('call', { op: 'settings_set', args: { key: 'text_size', value: 'comfortable' } }).then(() => import('./core.js')).then((c) => c.loadState())");
    }
  });

  await test('a walkthrough on a page\'s first visit, and setting tooltips with their defaults', async () => {
    const tour = () => exec(`return document.querySelector('[data-testid=tour]')?.innerText || ''`);
    await nav('#home');
    await nav('#accounts');
    await $('[data-testid=accounts-table]');
    assert.ok(!(await tour()), 'no tour opens on its own');
    await click('[data-testid=tour-start]');
    await waitFor(async () => (await tour()).includes('1 of'), 'the ? starts the tour');
    assert.ok((await tour()).includes('Add an account'));
    assert.ok(await exec(`return !!document.querySelector('.tour-spot')`), 'the part is highlighted');
    await click('[data-testid=tour-next]');
    await waitFor(async () => (await tour()).includes('2 of'), 'Next moves on');
    await shot('15-tour');
    await page.keyboard.press('Escape');
    await waitFor(async () => !(await tour()), 'Esc ends it');
    await click('[data-testid=tour-start]');
    await waitFor(async () => (await tour()).includes('1 of'), 'the ? button replays it');
    await click('[data-testid=tour-skip]');
    await nav('#settings?tab=limits');
    await page.hover('[data-testid=tip-refresh]');
    await waitFor(async () => (await exec(`return getComputedStyle(document.getElementById('tip-refresh')).display`)) === 'block', 'tooltip shown');
    await hasText('#tip-refresh', 'Default: 5');
    await shot('16-settings-tooltip');
    await click('[data-testid=tour-start]');
    await waitFor(async () => (await tour()).includes('1 of'), '? starts the tour of this page');
    await click('[data-testid=tour-skip]');
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
    await click('.menu button.danger');
    await $('[data-testid=remove-confirm]');
    await click('[data-testid=remove-confirm]');
    await waitFor(async () => !(await exec(`return !!document.querySelector('[data-testid=row-codex-personal]')`)), 'row gone');
    assert.ok(fs.existsSync(codexHome), 'its folder stays unless asked');
    assert.ok(!JSON.parse(hrm('list', '--json')).some((p) => p.name === 'codex-personal'));
    // a main profile: no "delete its folder" choice, and the tool's own folder stays
    await click('[data-testid=menu-codex-main]');
    await click('.menu button.danger');
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
  await test('computers: the page in the off state, the chooser, no chip, and the Overview as before', async () => {
    await nav('#computers');
    await $('[data-testid=computers] [data-testid=sync-folder]');
    const d = await exec(`return {
      name: arguments[0],
      folderOn: document.querySelector('[data-testid=sync-folder]').classList.contains('on'), folderText: document.querySelector('[data-testid=sync-folder-path]').textContent,
      hub: document.querySelector('[data-testid=sync-hub]').innerText, hubLive: !document.querySelector('[data-testid=sync-hub] [data-testid=hub-join]').disabled && !document.querySelector('[data-testid=sync-hub]').classList.contains('on'),
      direct: document.querySelector('[data-testid=st-direct]').disabled, pair: document.querySelector('[data-testid=pair-head]').disabled,
      transcripts: document.querySelector('[data-testid=st-transcripts]').checked, presence: document.querySelector('[data-testid=st-presence]').checked,
      pills: [...document.querySelectorAll('#sheet .pill')].map((p) => p.textContent.trim()),
      cards: [...document.querySelectorAll('[data-testid=computer-cards] .pc h2')].map((b) => b.textContent.trim()),
      facts: [...document.querySelectorAll('[data-testid=computer-cards] .pc dt')].map((b) => b.textContent.trim()), count: document.querySelector('[data-testid=computer-cards] .pc .count').innerText.replace(/\\s+/g, ' '),
      syncNow: !!document.querySelector('[data-testid^=now-], [data-testid=last-round]'), chip: document.getElementById('pcs').hidden, status: document.querySelector('[data-testid=page-status]').textContent,
      switches: [...document.querySelectorAll('[data-testid=computers] .srow .what .row > b')].map((b) => b.textContent) }`,
    [await exec("return import('./core.js').then((c) => c.S.cache.computers.this.name)")]);
    assert.ok(d.name, 'this computer has a name');
    assert.equal(d.folderOn, false, 'sync is off: the folder is not the way in use');
    assert.ok(d.folderText.includes('No folder'), 'no folder yet');
    assert.ok(d.hub.includes('A hub you host') && d.hub.includes('Not joined') && d.hubLive, 'the hub is offered, not in use');
    assert.ok(!d.direct && !d.pair, 'the direct connection and pairing are live');
    assert.deepEqual(d.pills, ['This computer'], 'one pill: this computer, and no way in use');
    assert.ok(d.transcripts && d.presence, 'transcripts and presence default to on');
    assert.deepEqual(d.cards, [d.name], 'only this computer');
    assert.deepEqual(d.facts, ['System', 'App version', 'Last seen', 'Connection'], 'the four facts of a card');
    assert.match(d.count, /^\d+ sessions? running$/, 'and its one key number');
    assert.deepEqual(d.switches, ['Carry transcripts', 'Share what runs here', 'Connect directly'], 'three rows of switches');
    assert.equal(d.syncNow, false, 'no Sync now and no last round while sync is off');
    assert.equal(d.chip, true, 'no computers chip while sync is off');
    assert.equal(d.status, 'Sync is off: only this computer.', 'the status says the plain fact');
    await shot('19-computers');
    // this computer's ⋯: Rename… and Details…, and no Sync now while sync is off
    await click('[data-testid=computer-cards] .pc [data-act=pc-menu]');
    assert.deepEqual(await exec("return [...document.querySelectorAll('.menu button')].map((b) => b.textContent)"), ['Rename…', 'Details…']);
    await click('.menu [data-testid=pc-details]');
    await hasText('[data-testid=pc-details-body]', 'entries published');
    await page.keyboard.press('Escape');
    // a row's ⓘ says what its line of help leaves out, like every row of Settings
    await page.hover('[data-testid=tip-transcripts]');
    await waitFor(async () => (await exec(`return getComputedStyle(document.getElementById('tip-transcripts')).display`)) === 'block', 'tooltip shown');
    await page.mouse.move(640, 700);
    // with sync off the other pages are as before: no scope switch, no Computer column, no dots
    await nav('#home');
    await $('[data-testid=activity]');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=scope], [data-testid=activity-pc]')")), 'the Overview is as before: no computer switch');
    await nav('#sessions');
    await $('[data-testid=group]');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=pc-seg], .a-srow .a-pc')")), 'Sessions as before');
    await nav('#projects');
    await $('[data-testid=project-cards] .a-pcard');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=pcard-pcs]')")), 'Projects as before');
  });

  await test('computers: a page of its own in the sidebar, which the chip opens; its name, a switch, Pair and the first-run choice work from it; Settings has no Computers section', async () => {
    const saved = () => JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8'));
    await nav('#home');
    await click('[data-testid=nav-computers]');
    await waitFor(async () => (await exec('return location.hash')) === '#computers', 'opened from the sidebar');
    await hasText('[data-testid=page-title]', 'Computers');
    await $('[data-testid=computers] [data-testid=sync-folder]');
    assert.equal(await exec("return document.querySelector('[data-testid=nav-computers]').getAttribute('aria-current')"), 'page', 'its item is lit');
    assert.equal(await exec("return document.getElementById('pcs').getAttribute('href')"), '#computers', 'the computers chip in the top bar leads here');
    // this computer's name: Rename… in its ⋯ opens a dialog; saved, shown on its card, and put back
    const name = (await text('[data-testid=computer-cards] .pc h2')).trim();
    for (const n of ['E2E-BOX', name]) {
      await click('[data-testid=computer-cards] .pc [data-act=pc-menu]');
      await click('.menu [data-testid=pc-rename]');
      assert.equal(await exec("return document.getElementById('st-devname').value"), n === name ? 'E2E-BOX' : name, 'the dialog starts from the name now');
      await type('[data-testid=st-devname]', n);
      await click('[data-testid=st-devname-save]');
      await waitFor(() => saved().device_name === n, 'the name ' + n + ' saved');
      await waitFor(async () => !(await exec("return document.getElementById('dialog-root').innerHTML")), 'the dialog closed');
      await hasText('[data-testid=computer-cards] .pc h2', n);
    }
    // a switch saves at once, and again
    for (const on of [false, true]) {
      await exec("document.querySelector('[data-testid=st-presence]').dataset.old = '1'");
      await click('[data-testid=st-presence]');
      await waitFor(() => saved().sync_presence === on, 'presence ' + on);
      await waitFor(() => exec("const e = document.querySelector('[data-testid=st-presence]'); return !!e && !e.dataset.old"), 'redrawn');
    }
    // the head's one primary button opens the pairing dialog; First-run choices… offers both ways to sync
    assert.equal(await exec("return document.querySelectorAll('#sheet .page-head .btn.primary').length"), 1);
    await click('[data-testid=pair-head]');
    await $('[data-testid=pair-tabs]');
    await click('.dlg .dh [data-act=dialog-close]');
    await waitFor(async () => !(await exec("return document.getElementById('dialog-root').innerHTML")), 'the pairing dialog closed');
    await click('#sheet [data-act=firstrun-open]');
    await $('.modal-card [data-testid=firstrun-folder]');
    await $('.modal-card [data-testid=firstrun-hub]');
    await click('.modal-card [data-testid=firstrun-later]');
    await waitFor(async () => !(await exec("return document.getElementById('dialog-root').innerHTML")), 'the first-run dialog closed');
    // Settings carries no Computers section and no jump to one; the old address opens the page
    await nav('#settings');
    await $('[data-testid=sec-appearance]');
    const st = await exec("return { sec: !!document.querySelector('#sec-computers, #sheet [data-testid=computers]'), chips: [...document.querySelectorAll('[data-testid=set-nav] a')].map((b) => b.textContent.trim()) }");
    assert.ok(!st.sec && !st.chips.includes('Computers') && st.chips.includes('Backups'), JSON.stringify(st));
    await nav('#settings?tab=computers');
    await waitFor(async () => (await exec('return location.hash')) === '#computers', 'the old address opens the Computers page');
    await $('[data-testid=computers] [data-testid=sync-folder]');
  });

  await test('computers: the All computers · This computer choice is remembered on this computer, All by default', async () => {
    const r = await exec(`return import('./computers.js').then((m) => { localStorage.removeItem('hm.scope'); const a = m.scope(); m.setScope('here'); const b = m.scope(); const c = localStorage.getItem('hm.scope'); m.setScope('all'); return [a, b, c, m.scope()]; })`);
    assert.deepEqual(r, ['all', 'here', 'here', 'all']);
  });

  // Sync now is in this computer's ⋯ while sync is on (and on a paired computer's card)
  const syncNow = async () => { await click('[data-testid=computer-cards] .pc [data-act=pc-menu]'); await click('.menu [data-testid=computers-now]'); };
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
    await nav('#computers');
    await $('[data-testid=sync-folder].on');
    await hasText('[data-testid=page-status]', 'through a folder');
    assert.deepEqual(await exec("return [document.querySelector('[data-testid=sync-folder] .pill').textContent, document.querySelector('[data-testid=sync-folder-path]').textContent, document.querySelector('[data-testid=sync-folder-pick]').textContent]"), ['In use', sync, 'Change…'], 'the folder is the way in use, with its path');
    // Stop syncing… sits in the folder's ⋯ and asks first; Cancel leaves the folder on
    await click('[data-testid=sync-folder-menu]');
    await click('.menu [data-testid=sync-off]');
    await hasText('.modal-card', 'Leave the folder?');
    await click('.modal-card .modal-foot [data-act=dialog-close]');
    await $('[data-testid=sync-folder].on');
    await syncNow();
    await waitFor(async () => (await text('[data-testid=computer-cards]')).includes('LAPTOP'), 'the other computer listed');
    const cards = await exec(`return [...document.querySelectorAll('[data-testid=computer-cards] .pc')].map((d) => d.innerText.replace(/\\s+/g, ' '))`);
    assert.equal(cards.length, 2);
    assert.ok(/This computer.*Through the folder/.test(cards[0]), cards[0]);
    assert.ok(/LAPTOP.*System Windows.*App version 1\.3\.2.*Last seen (Now|.* ago).*Connection Through the folder.*1 session running/.test(cards[1]), cards[1]);
    assert.ok(fs.existsSync(path.join(sync, 'devices')) && fs.readdirSync(path.join(sync, 'devices')).length === 2, 'this computer wrote its own record beside the other');
    // another computer's ⋯: Sync now and Forget…; no address to edit for one that is not paired
    await click('[data-testid=pc-menu-' + dev + ']');
    assert.deepEqual(await exec("return [...document.querySelectorAll('.menu button')].map((b) => b.textContent + (b.classList.contains('danger') ? ' (danger)' : ''))"), ['Sync now', 'Details…', 'Forget… (danger)']);
    await click('.menu [data-testid=pc-details]');
    assert.match((await text('[data-testid=pc-details-body]')).replace(/\s+/g, ' '), /Reached folder · .*Not paired.*Last pull.*Journal 3 of 3 entries applied here/, 'Details…: how it is reached, that it is not paired, and how much of its journal is applied');
    await page.keyboard.press('Escape');
    await hasText('[data-testid=last-round]', 'Last round');
    await page.mouse.move(640, 700);
    await shot('19b-two-computers');
    // the chip: a dot per computer, both heard from lately
    const chip = await exec(`const el = document.getElementById('pcs'); return { hidden: el.hidden, dots: el.querySelectorAll('.cdot').length, on: el.querySelectorAll('.cdot.on').length, text: el.textContent, names: [...el.querySelectorAll('.cdot')].map((a) => a.title) }`);
    assert.equal(chip.hidden, false, 'the chip shows with sync on');
    assert.equal(chip.dots, 2, 'a dot per computer, as the reference draws it');
    assert.equal(chip.on, 2, 'both heard within three minutes');
    assert.equal(chip.text, '2 computers', chip.text);
    assert.ok(chip.names.some((n) => n.includes('LAPTOP')), 'the full name on hover: ' + chip.names);
    // the Overview: the computers' choice, the other computer's session under Running now with a count per computer, the
    // account's row naming the computer it runs on, the Activity switch and each computer's 53-week total
    await nav('#home');
    await $('[data-testid=scope]');
    await hasText('[data-testid=running-count]', '0 here · 1 on ');
    await hasText('[data-testid=running-count]', 'LAPTOP');
    await hasText('[data-testid=running-now] .ov-list', 'LAPTOP');
    assert.equal(await exec("return document.querySelectorAll('[data-testid=running-now] [data-act=home-live-pc]').length"), 1, 'the count of a computer with sessions shows that computer\'s alone');
    assert.ok((await exec("return document.querySelector('[data-testid=lane-claude-main]').title")).includes('LAPTOP'), 'the account running there names that computer on hover');
    assert.ok((await text('[data-testid=route]')).length > 0, 'and the selected route names its computers');
    assert.ok((await exec("return [...document.querySelectorAll('[data-testid=activity-pc] button')].map((a) => a.title).join(' ')")).includes('LAPTOP'), 'the year clock has a switch per computer, with the name on hover');
    await hasText('[data-testid=activity] .year-clock-pcs', 'LAPTOP');
    assert.ok((await exec("const s = document.getElementById('sheet'); return s.scrollWidth <= s.clientWidth")), 'nothing on the Overview is wider than the page');
    await shot('19c-overview-all-computers');
    await click('[data-testid=activity-pc-' + dev + ']');
    await hasText('[data-testid=activity] .year-clock-center', 'LAPTOP');
    await click('[data-testid=scope]');
    await click('[data-testid=scope-here]');
    await waitFor(async () => (await text('[data-testid=scope]')).includes('This computer'), 'This computer picked');
    assert.equal(await exec("return localStorage.getItem('hm.scope')"), 'here', 'remembered');
    await shot('19d-overview-this-computer');
    await click('[data-testid=scope]');
    await click('[data-testid=scope-all]');
    await waitFor(async () => (await text('[data-testid=scope]')).includes('All computers'), 'every computer again');
    // Sessions: the Computer column, the filter, and the session open on the other computer
    await nav('#sessions?days=all');
    await $('[data-testid=session-laptop-s1] .a-pc');
    assert.ok((await text('[data-testid=session-laptop-s1] .a-pc')).includes('LAPTOP'));
    assert.ok((await text('[data-testid=session-s-main-1] .a-pc')).trim().length > 0, 'this computer named on its own session');
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
    hrm('index', 'refresh'); // the new accounts' session files into the index now (the app's walk would, in time)
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
    await nav('#computers');
    await $('[data-testid=computers] [data-testid=st-direct]');
    const d = await exec(`return { direct: document.querySelector('[data-testid=st-direct]').disabled, pair: document.querySelector('[data-testid=pair-head]').disabled,
      port: document.querySelector('[data-testid=listen-port]').textContent, change: document.querySelector('[data-testid=listen-port-change]').textContent, beside: document.querySelector('[data-testid=listen-port-change]').parentElement.contains(document.querySelector('[data-testid=st-direct]')) }`);
    assert.ok(!d.direct && !d.pair, 'the switch and Pair are live');
    assert.ok(d.port && d.change.startsWith('Listen port') && d.beside, 'the listen port sits beside the switch: ' + JSON.stringify(d));
    // the port: its button opens a small dialog; 0 means a free port
    await click('[data-testid=listen-port-change]');
    await $('[data-testid=lp-v]');
    await page.fill('[data-testid=lp-v]', '0');
    await click('[data-testid=lp-ok]');
    await waitFor(async () => !(await exec("return document.getElementById('dialog-root').innerHTML")), 'the port dialog closed');
    await $('[data-testid=computers] [data-testid=st-direct]');
    await click('[data-testid=pair-head]');
    await $('[data-testid=pair-tabs]');
    assert.deepEqual(await exec("return [...document.querySelectorAll('[data-testid=pair-tabs] button')].map((a) => a.textContent)"), ['Show a code', 'Enter a code']);
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
    hrm2('index', 'refresh');
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
      await nav('#computers');
      await $('[data-testid=st-direct]');
      if (!(await exec("return document.querySelector('[data-testid=st-direct]').checked"))) {
        await click('[data-testid=st-direct]');
        await waitFor(() => exec("return document.querySelector('[data-testid=st-direct]')?.checked"), 'the direct connection on');
      }
      await click('[data-testid=pair-head]');
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
      await waitFor(async () => (await text(card + ' [data-testid=transport]').catch(() => '')).trim() === 'Direct connection', 'the card says Direct connection', 45000);
      await click(`[data-testid="pc-menu-${id2}"]`);
      await click('.menu [data-testid=pc-details]');
      assert.match((await text('[data-testid=pc-details-body]')).replace(/\s+/g, ' '), /Reached direct · .* · connected Paired /, 'Details… says how it is reached and when it was paired');
      await page.keyboard.press('Escape');
      await hasText('[data-testid=page-status]', 'LAPTOP is connected directly');
      await page.mouse.move(640, 700);
      await shot('42-paired-connected');
      await exec("document.documentElement.dataset.theme = 'dark'");
      await shot('42-dark-paired-connected');
      await exec("document.documentElement.dataset.theme = 'light'");
      const chip = await exec(`const el = document.getElementById('pcs'); return { hidden: el.hidden, on: el.querySelectorAll('.cdot.on').length }`);
      assert.ok(!chip.hidden && chip.on >= 2, 'the chip: this computer and the paired one on (an earlier test may have left a folder peer): ' + JSON.stringify(chip));
      // the second computer's session arrives after Sync now, the button on a paired computer's card
      await click(`[data-testid=now-${id2}]`);
      await nav('#sessions?days=all');
      await waitFor(() => exec("return !!document.querySelector('[data-testid=session-laptop-s9]')"), 'the session from the second computer', 30000);
      await hasText('[data-testid=session-laptop-s9] .a-pc', 'LAPTOP');
      await shot('42b-sessions-from-peer');
      // Forget: a confirm, then the pairing is gone, and with it the card's address field. The card goes too, unless
      // a sync folder still knows the computer (the folder test above leaves one on): then it stays as a folder computer
      await nav('#computers');
      // its address (a VPN name with the port): Edit address… in its ⋯ opens a dialog
      await click(`[data-testid="pc-menu-${id2}"]`);
      assert.deepEqual(await exec("return [...document.querySelectorAll('.menu button')].map((b) => b.textContent)"), ['Sync now', 'Edit address…', 'Details…', 'Forget…']);
      await click(`.menu [data-testid="addr-edit-${id2}"]`);
      await type(`[data-testid="addr-${id2}"]`, 'laptop.example:4455');
      await click(`[data-testid="addr-${id2}-save"]`);
      await waitFor(async () => ((await exec("return window.__TAURI__.core.invoke('call', { op: 'computers_status', args: {} })")).peers.find((x) => x.id === id2) || {}).address === 'laptop.example:4455', 'the address saved');
      await waitFor(async () => (await exec(`return document.querySelector('[data-testid="pc-menu-${id2}"]')?.dataset.address`)) === 'laptop.example:4455', 'the card drawn with it');
      await click(`[data-testid="pc-menu-${id2}"]`);
      await click(`.menu [data-testid="forget-${id2}"]`);
      await hasText('.modal-card', 'drops its certificate');
      await click('#confirm-ok');
      await waitFor(async () => !(await exec(`return document.querySelector('[data-testid="pc-menu-${id2}"]')?.dataset.direct`)), 'no longer paired', 15000);
      assert.ok(!/Direct/.test(await exec(`return (document.querySelector('[data-testid=computer-${id2}] [data-testid=transport]') || { textContent: '' }).textContent`)), 'not reached directly any more');
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
    // not cloned here: still offered, the dialog asks for a folder before it resumes
    await click('[data-testid=resume-here-laptop-s8]');
    await hasText('[data-testid=resume-here-root]', 'not cloned here: choose a folder');
    assert.ok(await exec("return document.querySelector('[data-testid=resume-here-ok]').disabled"), 'no folder, no resume');
    await click('.modal-foot [data-act=dialog-close]');
    await shot('41-sessions-resume-here');
    // reading it: fetched from the folder, shown through Claude Code's parser
    await dbl('[data-testid=read-laptop-s7]');
    await hasText('[data-testid=session-title]', 'Rotate the staging certificates (laptop)');
    await hasText('[data-testid=chat] .a-msg.ai', 'Certificates rotated on the laptop.');
    await hasText('[data-testid=session-from]', 'LAPTOP');
    assert.ok(fs.existsSync(path.join(data, 'transcripts', dev, 'claude', 'claude-main', 'laptop-s7.jsonl')), 'the fetched copy sits in the data folder');
    await shot('42-session-remote');
    // Resume here: the dialog names the local clone, then the fake claude gets the copy's path and --fork-session, in the clone
    await nav('#sessions?days=all');
    await click('[data-testid=resume-here-laptop-s7]');
    await hasText('[data-testid=resume-here-root]', 'platform-gitops');
    await hasText('[data-testid=resume-here-dialog]', 'the LAPTOP copy stays as it is');
    await $('[data-testid=resume-here-pick]');
    await shot('43-resume-here-dialog');
    await click('[data-testid=resume-here-ok]');
    const launch = await waitFor(() => readLines(termLog).find((t) => t.args && t.args[3] === '--resume' && String(t.args[4]).endsWith('laptop-s7.jsonl')), 'the fork launched');
    assert.deepEqual(launch.args.slice(0, 3).concat(launch.args.slice(5)), ['run', 'claude-main', '--', '--fork-session'], JSON.stringify(launch));
    assert.equal(launch.cwd, repo, 'opened in this computer\'s clone');
    assert.ok(fs.existsSync(launch.args[4]) && launch.args[4].startsWith(path.join(home, '.claude', 'projects')), 'the copy is under this computer\'s Claude Code folder: ' + launch.args[4]);
    // the copy is a session of this computer now, marked as continued from the laptop
    await nav('#sessions?days=all');
    await hasText('[data-testid=continued-laptop-s7]', 'continued from LAPTOP');
    // Computers: the Carry transcripts popover counts what this computer carried
    await nav('#computers');
    await $('[data-testid=computers] [data-testid=st-transcripts]');
    await hasText('#tip-transcripts', 'Carried');
    await hasText('#tip-transcripts', '20 MB');
  });

  // ---- the hub (WP-9) ----
  // "A hub you host" is live: the Join dialog (the address, a code or the admin token), the first-run choice with both
  // cards, and an unreachable hub shown as the operation's own error. HRM_E2E_HUB=1 starts hrm-hub (target/debug) as a
  // child process and joins it from the card, sees the recovery key once, invites a second data folder that joins with
  // hrm hub join, and sees its session after Sync now.
  await test('hub: the card is live, the Join dialog takes a code or the admin token, an unreachable hub is the error text, the first-run choice offers both cards', async () => {
    await nav('#computers');
    await $('[data-testid=computers] [data-testid=hub-join]');
    const d = await exec(`return { on: document.querySelector('[data-testid=sync-hub]').classList.contains('on'), url: document.querySelector('[data-testid=hub-url]').textContent,
      pills: [...document.querySelectorAll('[data-testid=sync-hub] .pill')].map((p) => p.textContent.trim()), ring: !!document.querySelector('[data-testid=ring-enter], [data-testid=sync-hub-menu]') }`);
    assert.ok(!d.on && d.url.includes('Not joined') && !d.pills.length && !d.ring, JSON.stringify(d));
    await click('[data-testid=hub-join]');
    await $('[data-testid=hub-url-in]');
    // the dialog checks the address and asks for a code or the token before any call
    await page.fill('[data-testid=hub-url-in]', 'hub.example.com');
    await click('[data-testid=hub-ok]');
    await hasText('[data-testid=hub-error]', 'https://');
    // a port nothing listens on: taken and released now (a fixed low port can hang on a filtered machine)
    const net = await import('node:net');
    const closed = await new Promise((r) => { const srv = net.createServer(); srv.listen(0, '127.0.0.1', () => { const p = srv.address().port; srv.close(() => r(p)); }); });
    await page.fill('[data-testid=hub-url-in]', `http://127.0.0.1:${closed}`);
    await click('[data-testid=hub-ok]');
    await hasText('[data-testid=hub-error]', 'six digits');
    // the code is formatted as typed; a hub that does not answer is the operation's own error, and Join can be tried again
    await page.type('[data-testid=hub-code]', '4829157');
    assert.equal(await exec("return document.getElementById('hub-code').value"), '482 915');
    await click('[data-testid=hub-ok]');
    await waitFor(async () => { const t = (await exec("return document.querySelector('[data-testid=hub-error]')?.textContent || ''")).trim(); return t.length > 0 && !t.includes('six digits'); }, "the operation's error (nothing listens at 127.0.0.1:9)", 30000);
    assert.equal(await exec("return document.getElementById('hub-ok').disabled"), false, 'Join can be tried again');
    await shot('51-hub-join-dialog');
    await page.keyboard.press('Escape');
    await waitFor(async () => !(await exec("return document.getElementById('dialog-root').innerHTML")), 'the dialog closed');
    // the first-run choice: both cards, Not now closes it, the hub card opens the Join dialog
    await exec(`return import('./computers.js').then((m) => m.firstRunDialog())`);
    await $('[data-testid=firstrun-hub]');
    assert.deepEqual(await exec("return [...document.querySelectorAll('#dialog-root .b-sync-choice > b')].map((b) => b.firstChild.textContent.trim())"), ['A shared folder', 'A hub you host']);
    await shot('52-first-run-choice');
    await click('[data-testid=firstrun-hub]');
    await $('[data-testid=hub-url-in]');
    await page.keyboard.press('Escape');
    await waitFor(async () => !(await exec("return document.getElementById('dialog-root').innerHTML")), 'the dialog closed');
  });

  if (process.env.HRM_E2E_HUB) await test('hub: joining from the card with the admin token, the recovery key once, Invite a computer, a second data folder joining with the code, its session after Sync now', async () => {
    const hubExe = path.join(appDir, 'hrm-hub.exe');
    assert.ok(fs.existsSync(hubExe), 'target/debug/hrm-hub.exe (cargo build --workspace with the hub crate)');
    const hubPort = 19000 + (process.pid % 500), hubUrl = `http://127.0.0.1:${hubPort}`, hubData = path.join(sb, 'hub');
    fs.mkdirSync(hubData, { recursive: true });
    const hub = spawn(hubExe, [], { env: { ...process.env, HUB_ADMIN_TOKEN: 'e2e-admin', HUB_DATA: hubData, HUB_BIND: `127.0.0.1:${hubPort}` }, stdio: 'ignore' });
    try {
      await waitFor(() => fetch(hubUrl + '/health').then((r) => (r.ok ? true : null)).catch(() => null), 'the hub up', 30000);
      // this computer joins with the admin token: the card turns on, and the recovery key is shown once
      await nav('#computers');
      await click('[data-testid=computers] [data-testid=hub-join]');
      await $('[data-testid=hub-url-in]');
      await page.fill('[data-testid=hub-url-in]', hubUrl);
      await page.fill('[data-testid=hub-admin]', 'e2e-admin');
      await click('[data-testid=hub-ok]');
      await $('[data-testid=ring-key]');
      const key = (await text('[data-testid=ring-key]')).trim();
      assert.match(key, /^([A-Z2-7]{4} ){7}[A-Z2-7]{4}$/, 'the recovery key: 8 groups of 4: ' + key);
      await shot('53-recovery-key');
      await click('[data-testid=ring-saved]');
      await waitFor(async () => !(await exec("return document.getElementById('dialog-root').innerHTML")), 'the key dialog closed');
      await waitFor(() => exec("return document.querySelector('[data-testid=sync-hub]')?.classList.contains('on')"), 'the hub is the way in use', 15000);
      assert.equal((await text('[data-testid=sync-hub] .pill')).trim(), 'In use');
      assert.match(await text('[data-testid=hub-line]'), /1 computer · (live|polling)/, 'its line counts the computers and says live or polling');
      assert.equal((await text('[data-testid=hub-url]')).trim(), hubUrl);
      await hasText('[data-testid=page-status]', 'through a hub');
      // the hub's ⋯: the recovery key, and Leave… in the danger colour; Invite a computer… is the button beside it
      await $('[data-testid=sync-hub] [data-testid=hub-invite]');
      await click('[data-testid=sync-hub-menu]');
      assert.deepEqual(await exec("return [...document.querySelectorAll('.menu button')].map((b) => b.textContent + (b.classList.contains('danger') ? ' (danger)' : ''))"), ['Show the recovery key…', 'Enter the recovery key…', 'Leave the hub… (danger)']);
      // Enter the recovery key… opens its dialog (this computer already holds the key, so it is closed again)
      await click('.menu [data-testid=ring-enter]');
      await $('[data-testid=ring-in]');
      await click('.modal-card .modal-foot [data-act=dialog-close]');
      // Show the recovery key… sits behind a confirm and shows the same key
      await click('[data-testid=sync-hub-menu]');
      await click('.menu [data-testid=ring-show]');
      await click('#confirm-ok');
      await hasText('[data-testid=ring-key]', key);
      await click('[data-testid=ring-saved]');
      // Invite a computer…: a six-digit code
      await click('[data-testid=hub-invite]');
      await waitFor(async () => /\d{3} \d{3}/.test(await text('[data-testid=invite-body]')), 'a code in the dialog', 15000);
      const code = (await text('[data-testid=invite-code]')).replace(/\D/g, '');
      await shot('54-invite-code');
      await page.keyboard.press('Escape');
      // the second computer: its own data folder, a Claude session indexed there, joins with the code and the recovery key, pushes
      const data3 = path.join(sb, 'data3'), home3 = path.join(sb, 'home3');
      fs.mkdirSync(home3, { recursive: true });
      write(path.join(data3, 'settings.json'), JSON.stringify({ theme: 'light', device_name: 'HUBPC' }));
      const env3 = { ...env, HRM_HOME: data3, HRM_PROFILES_HOME: home3 };
      const hrm3 = (...args) => execFileSync(path.join(appDir, 'hrm.exe'), args, { env: env3, encoding: 'utf8' });
      const cwd3 = 'C:\\hubpc\\Projects\\platform-gitops', t = new Date().toISOString();
      write(path.join(home3, '.claude', 'projects', 'p', 'hub-s5.jsonl'), jl([
        { type: 'user', sessionId: 'hub-s5', cwd: cwd3, timestamp: t, message: { role: 'user', content: 'Renew the hub certificate' } },
        { type: 'assistant', sessionId: 'hub-s5', cwd: cwd3, timestamp: t, message: { id: 'm5', model: 'claude-opus-5-5', usage: { input_tokens: 500, output_tokens: 60 }, content: [{ type: 'text', text: 'Renewed.' }] } },
        { type: 'ai-title', aiTitle: 'Renew the hub certificate', sessionId: 'hub-s5' },
      ]));
      hrm3('init');
      hrm3('index', 'refresh');
      const joined = hrm3('hub', 'join', hubUrl, '--code', code);
      assert.ok(joined.includes('joined ' + hubUrl), joined);
      assert.ok(hrm3('hub', 'status').includes('missing'), 'no ring key on the second computer yet');
      hrm3('ring', 'enter', key);
      hrm3('index', 'refresh'); // refreshed with sync on, the sessions are journaled (indexed before the join, they were not)
      hrm3('computers', 'now');
      const id3 = JSON.parse(fs.readFileSync(path.join(data3, 'settings.json'), 'utf8')).device_id;
      // Sync now here: the second computer's card through the hub, and its session
      await syncNow();
      await waitFor(() => exec(`return !!document.querySelector('[data-testid=computer-${id3}]')`), 'the second computer\'s card', 30000);
      await waitFor(async () => /hub/.test(await text(`[data-testid=computer-${id3}] [data-testid=transport]`).catch(() => '')), 'the card says hub');
      assert.ok((await text('[data-testid=hub-line]')).includes('2 computers'), await text('[data-testid=hub-line]'));
      await page.mouse.move(640, 700);
      await shot('55-hub-two-computers');
      await nav('#sessions?days=all');
      await waitFor(() => exec("return !!document.querySelector('[data-testid=session-hub-s5]')"), 'the session from the second computer', 30000);
      await hasText('[data-testid=session-hub-s5] .a-pc', 'HUBPC');
      await shot('55b-sessions-from-hub');
      // a used code is refused; Stop syncing leaves the hub
      assert.throws(() => hrm3('hub', 'join', hubUrl, '--code', code), /did not accept/);
      await nav('#computers');
      await click('[data-testid=sync-hub-menu]');
      await click('.menu [data-testid=hub-leave]');
      await hasText('.modal-card', 'Leave the hub?');
      await click('#confirm-ok');
      await waitFor(async () => (await exec("return document.querySelector('[data-testid=sync-hub]')?.classList.contains('on')")) === false, 'the hub is not in use after leaving', 15000);
    } finally { hub.kill(); }
  });

  // ---- backup, archive and welcome back (WP-8) ----
  // Backups hold the history and a restore merges it; old transcripts move to the archive while the session stays
  // listed and readable; a sync folder with history offers Welcome back when it is chosen.
  await test('backups and archive: the section, hrm archive now on an old session, Restore… from a backup, Welcome back', async () => {
    const { createHash } = await import('node:crypto');
    await nav('#settings?tab=backups');
    await $('[data-testid=sec-backups] [data-testid=archive-now]');
    const d = await exec(`return { title: document.querySelector('[data-testid=sec-backups] .pane-head h2').textContent.trim(),
      folder: document.querySelector('[data-testid=st-bk-folder]').checked, transcripts: document.querySelector('[data-testid=st-bk-transcripts]').checked,
      days: document.querySelector('[data-testid=st-archive-days] button.on')?.textContent, sync: document.querySelector('[data-testid=st-archive-sync]').checked,
      dir: document.querySelector('[data-testid=st-archive-dir]').textContent, restore: !!document.querySelector('[data-testid=restore-open]') }`);
    assert.equal(d.title, 'Backups');
    assert.ok(d.folder && !d.transcripts, 'backups go to the sync folder; transcripts stay out by default');
    assert.equal(d.days, '30 days');
    assert.ok(d.sync && d.dir.includes('No folder') && d.restore, JSON.stringify(d));
    await exec("document.querySelector('[data-testid=sec-backups]').scrollIntoView()");
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
    hrm('index', 'refresh'); // the moved file leaves the index now, not at the app's next walk
    // the row keeps its place with the pill, reads from the archive, and offers no Resume
    await nav('#sessions?days=all');
    await hasText('[data-testid=archived-s-old-1]', 'archived');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=session-s-old-1] [data-act=resume]')")), 'no Resume on an archived session');
    await shot('45-sessions-archived');
    await dbl('[data-testid=read-s-old-1]');
    await hasText('[data-testid=chat] .a-msg.ai', 'The old manifests are tidy.');
    await hasText('[data-testid=resume-reason]', 'archived');
    await shot('46-session-archived');
    // Restore… from a backup made now: the preview counts this computer's history and names it; merging adds nothing
    await nav('#settings?tab=backups');
    // the archived row is written by the CLI in its own process; on a busy computer the app's backup can be made
    // before it sees it, so the backup is tried up to three times
    let counts = '';
    for (let i = 0; i < 3; i++) {
      await click('[data-testid=backup-now]');
      await $('[data-testid=backup-Manual]');
      await click('[data-testid=restore-open]');
      await click('[data-testid=restore-src-Manual]');
      await hasText('[data-testid=restore-counts]', 'archived transcripts');
      counts = (await text('[data-testid=restore-counts]')).replace(/,/g, '');
      if (/\b1 archived/.test(counts)) break;
      await page.keyboard.press('Escape');
      await waitFor(async () => !(await exec("return document.getElementById('dialog-root').innerHTML")), 'the dialog closed');
      await sleep(2000);
    }
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
    await hasText('[data-testid=session-desk-s1] .a-pc', 'DESK');
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=session-desk-s1] [data-act=resume]')")), 'a past computer\'s session is not resumed here as a local one');
  });

  // ---- index and watcher (WP-10) ----
  // The pages read the sessions table; the folder watcher puts a changed file into it within a second and the
  // page redraws in place; the search is the index's (FTS5); the list comes in pages of 50.
  await test('index and watcher: a seeded file is on the Sessions page within two seconds, the search finds a word of a title, and the list pages at 50', async () => {
    await nav('#sessions?days=all');
    await $('[data-testid=session-s-main-1]');
    const t = new Date().toISOString();
    const f = path.join(home, '.claude', 'projects', 'p', 's-watched-1.jsonl');
    // no reload, no click: the watcher sees the file and the page redraws through the refreshed event
    const t0 = Date.now();
    write(f, jl([
      { type: 'user', sessionId: 's-watched-1', cwd: repo, timestamp: t, message: { role: 'user', content: 'Watch the Velero schedule drift' } },
      { type: 'assistant', sessionId: 's-watched-1', cwd: repo, timestamp: t, message: { id: 'w1', model: 'claude-opus-5-5', usage: { input_tokens: 40, output_tokens: 8 }, content: [{ type: 'text', text: 'Watched.' }] } },
    ]));
    await waitFor(() => exec("return !!document.querySelector('[data-testid=session-s-watched-1]')"), 'the new session on the page without a reload', 2000);
    console.log(`     indexed and drawn in ${Date.now() - t0} ms`);
    assert.equal(await text('[data-testid=page-title]'), 'Sessions', 'still the Sessions page');
    const st = JSON.parse(hrm('index', 'status', '--json'));
    assert.ok(st.watcher === true && st.sessions >= 5 && st.tools.some((x) => x.harness === 'claude'), JSON.stringify(st));
    // the search: a word of the title, through the top bar and the CLI
    await type('[data-testid=find]', 'velero');
    await waitFor(async () => (await exec("return [...document.querySelectorAll('.a-srow')].map((r) => r.dataset.testid)")).join() === 'session-s-watched-1', 'only the Velero session');
    assert.ok(hrm('sessions', '--search', 'velero').includes('s-watched-1'), 'the CLI searches the same index');
    await page.fill('[data-testid=find]', '');
    await exec(`document.getElementById('find').dispatchEvent(new Event('input'))`);
    await waitFor(async () => (await exec("return document.querySelectorAll('.a-srow').length")) > 1, 'every session is back');
    // paging: 60 more sessions, 50 on the first page and the rest on the next
    for (let i = 0; i < 60; i++) {
      const at = new Date(Date.now() - (i + 1) * 60000).toISOString();
      write(path.join(home, '.claude', 'projects', 'p', `s-page-${i}.jsonl`), jl([{ type: 'user', sessionId: `s-page-${i}`, cwd: repo, timestamp: at, message: { role: 'user', content: `Paged session ${i}` } }]));
    }
    hrm('index', 'refresh');
    await nav('#sessions?days=all&group=tool');
    await $('[data-testid=s-pager]');
    await waitFor(async () => (await exec("return document.querySelectorAll('.a-srow').length")) === 50, '50 rows on the first page');
    const count = await text('[data-testid=sessions-count]');
    assert.ok(+(count.replace(/,/g, '').match(/^(\d+) in all/) || [])[1] >= 61, count); // the 60 seeded here and every earlier test's
    await click('[data-testid=page-next]');
    await waitFor(async () => (await exec("return location.hash")).includes('page=1') && (await exec("return document.querySelectorAll('.a-srow').length")) < 50 && (await exec("return document.querySelectorAll('.a-srow').length")) > 0, 'the rest on the next page');
    await shot('60-sessions-paged');
    // a group's Show all fetches its rows beyond the page
    await nav('#sessions?days=all&group=tool');
    await $('[data-testid=s-more-claude]');
    await click('[data-testid=s-more-claude]');
    await waitFor(async () => (await exec("return document.querySelectorAll('.a-srow').length")) > 60, 'every Claude Code session in the group');
  });

  // ---- safety (1.9.15) ----
  // A redraw nobody asked for (a refresh, a poll) leaves alone what the reader is doing; a failure is said, not lost.
  // `refreshed` is what the backend emits after each import and after each change the folder watcher sees.
  const refreshed = () => exec("return window.__TAURI__.event.emit('refreshed', { new_rows: 0 })");
  const renders = () => exec("if (window.__renders === undefined) { window.__renders = 0; document.addEventListener('rendered', () => window.__renders++); } return window.__renders");
  const anAccount = () => JSON.parse(hrm('list', '--json'))[0].name;

  await test('a background refresh keeps what was typed into a field that has lost the focus, and Save stores it', async () => {
    const name = anAccount();
    await nav(`#account?name=${name}&tab=settings`);
    await $('#pf-email');
    await page.fill('#pf-email', 'typed@example.com');
    await page.press('#pf-email', 'Tab');
    assert.notEqual(await exec('return document.activeElement.id'), 'pf-email', 'the field lost the focus');
    const n = await renders();
    await refreshed();
    await waitFor(async () => (await renders()) > n, 'the background redraw', 15000);
    assert.equal(await exec("return document.getElementById('pf-email').value"), 'typed@example.com', 'what was typed is still in the field');
    await click('[data-act=save-email]');
    await waitFor(() => JSON.parse(hrm('list', '--json')).find((p) => p.name === name).expected_email === 'typed@example.com', 'the typed email saved');
    // moving to another account shows that account's own value, not the one typed here
    await page.fill('#pf-label', 'Typed and left');
    await page.press('#pf-label', 'Tab');
    const other = JSON.parse(hrm('list', '--json')).find((p) => p.name !== name);
    await nav(`#account?name=${other.name}&tab=settings`);
    await waitFor(async () => (await text('[data-testid=detail-name]')).includes(other.label), 'the other account');
    assert.equal(await exec("return document.getElementById('pf-label').value"), other.label, 'another page starts from its own values');
  });

  // ---- Settings and Computers as the 1.10 drawings draw them ----
  await test('settings: choosing a section shows only it and sets the address, a link to a section opens it, and Back returns to the one before', async () => {
    await nav('#settings');
    await $('[data-testid=sec-appearance]');
    const shown = () => exec("return { panes: [...document.querySelectorAll('#set-panes .set-pane')].map((p) => p.dataset.testid), on: [...document.querySelectorAll('[data-testid=set-nav] a.on')].map((a) => a.textContent.trim()), hash: location.hash }");
    assert.deepEqual(await shown(), { panes: ['sec-appearance'], on: ['Appearance'], hash: '#settings' }, 'the first section, when the address names none');
    await click('[data-testid=set-tab-connections]');
    await $('[data-testid=sec-connections] #st-port');
    assert.deepEqual(await shown(), { panes: ['sec-connections'], on: ['Connections'], hash: '#settings?tab=connections' });
    assert.ok(!(await exec("return !!document.querySelector('[data-testid=st-theme]')")), 'the rows of the other sections are not on the page');
    assert.deepEqual(await exec("return [document.querySelector('[data-testid=nav-settings]').getAttribute('aria-current'), document.querySelector('.set-pane .pane-head').innerText.replace(/\\s+/g, ' ').trim()]"), ['page', 'Connections 2 settings'], 'Settings stays lit in the sidebar; the card names the section and counts its rows');
    // a link to a section opens it
    await nav('#settings?tab=backups');
    await $('[data-testid=sec-backups] [data-testid=backup-now]');
    assert.deepEqual(await shown(), { panes: ['sec-backups'], on: ['Backups'], hash: '#settings?tab=backups' });
    // Back returns to the section before; a section this system does not have opens the first
    await exec('history.back()');
    await $('[data-testid=sec-connections]');
    assert.equal((await shown()).hash, '#settings?tab=connections');
    await nav('#settings?tab=nothing');
    await $('[data-testid=sec-appearance]');
    // Apps › Harness Manager leads to Updates
    await exec("return import('./core.js').then((c) => c.go('settings', { tab: 'updates' }))");
    await $('[data-testid=sec-updates] [data-key=setting-self]');
  });

  await test('settings: Search settings finds rows in other sections by label and by help, says when nothing matches, survives a redraw, and gives the section back when cleared', async () => {
    await nav('#settings');
    await $('[data-testid=sec-appearance]');
    const found = () => exec("return [...document.querySelectorAll('#set-panes .set-pane')].map((p) => (p.querySelector('h2')?.textContent || '') + ': ' + [...p.querySelectorAll('.srow .what .row > b')].map((b) => b.textContent).join(', '))");
    const search = async (t) => { await page.fill('[data-testid=set-find]', t); await sleep(150); return found(); };
    assert.deepEqual(await search('router'), ['Connections: Router port'], 'a row of another section, by its label');
    assert.deepEqual(await search('127.0.0.1'), ['Connections: Router port'], 'and by its line of help');
    assert.deepEqual(await search('TRAY'), ['Window and terminal: Close to the tray, Launch at sign-in'], 'in any case; the rows of one section under its name');
    assert.deepEqual(await exec("return [document.querySelector('.set-pane .pane-head .muted').textContent, document.querySelectorAll('[data-testid=set-nav] a.on').length]"), ['2 found', 0], 'the head counts the rows found; no section is lit while rows of several may show');
    const many = await search('log');
    assert.ok(many.length >= 3 && many.some((s) => s.startsWith('Limits: Live Claude limits')) && many.some((s) => s.startsWith('Log: Keep a log')), 'rows of several sections, each under its section: ' + many.join(' | '));
    // a found row works where it is, and the redraw after it keeps the words, the rows found and the focus
    await search('tray');
    await page.focus('[data-testid=set-find]');
    const before = JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).close_to_tray;
    await exec("document.querySelector('[data-testid=st-tray]').dataset.old = '1'; document.querySelector('[data-testid=st-tray]').click()");
    await waitFor(() => JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).close_to_tray === !before, 'the switch in a found row saved');
    await waitFor(() => exec("const e = document.querySelector('[data-testid=st-tray]'); return !!e && !e.dataset.old"), 'redrawn');
    assert.deepEqual([await exec("return document.getElementById('set-find').value"), await exec('return document.activeElement.id'), await found()], ['tray', 'set-find', ['Window and terminal: Close to the tray, Launch at sign-in']]);
    await exec("document.querySelector('[data-testid=st-tray]').click()");
    await waitFor(() => JSON.parse(fs.readFileSync(path.join(data, 'settings.json'), 'utf8')).close_to_tray === before, 'and put back');
    await exec("return import('./core.js').then((c) => c.render(true))");
    assert.equal(await exec("return document.getElementById('set-find').value"), 'tray', 'a background redraw keeps the words too');
    // nothing matches
    assert.deepEqual(await search('zzzz-nothing'), [': ']);
    assert.equal((await text('[data-testid=set-none]')).trim(), 'No setting matches.');
    await shot('14d-settings-search-none');
    await search('transcripts');
    await shot('14e-settings-search');
    // cleared, the chosen section is back; choosing a section from the list ends a search too
    assert.deepEqual(await search(''), ['Appearance: Theme, Light and dark, Limits shown as, Moving routes, Text size']);
    await search('router');
    await click('[data-testid=set-tab-log]');
    await $('[data-testid=sec-log] [data-testid=log-open]');
    assert.deepEqual([await exec("return document.getElementById('set-find').value"), await found()], ['', ['Log: Keep a log, Level, Log file']]);
  });

  await test('computers: the page as drawn, and every action opens its dialog: Pair, Rename, Details, Forget, Change, Stop syncing, Join, the recovery key, Listen port, First-run choices', async () => {
    // (the folder chosen by the Welcome back test is in use, with the computers found in it; Edit address… is in the
    // pairing test, Invite a computer… and Leave the hub… in the hub test)
    const dialog = async (title) => {
      await waitFor(async () => (await exec("return document.querySelector('#dialog-root .modal-card')?.getAttribute('aria-label') || ''")).startsWith(title), 'the dialog ' + title);
      await page.keyboard.press('Escape');
      await waitFor(async () => !(await exec("return document.getElementById('dialog-root').innerHTML")), title + ' closed');
    };
    const menu = async (button) => {
      await click(button);
      return exec("return [...document.querySelectorAll('.menu button')].map((b) => b.textContent + (b.classList.contains('danger') ? ' (danger)' : ''))");
    };
    await nav('#computers');
    await $('[data-testid=sync-folder].on');
    await $('[data-testid=computer-cards] .pc:nth-child(2)');
    await page.mouse.move(640, 700);
    const lay = await exec(`const r = (s) => document.querySelector(s).getBoundingClientRect(), c = [...document.querySelectorAll('.pc')].map((e) => e.getBoundingClientRect());
      return { n: c.length, row: c.slice(0, 3).every((x) => x.top === c[0].top), widths: [...new Set(c.map((x) => Math.round(x.width)))], first: [document.querySelector('.pc h2').textContent === arguments[0], document.querySelector('.pc .pill')?.textContent],
        heads: [...document.querySelectorAll('.pc')].every((e) => e.querySelector('.pc-dot') && e.querySelector('[data-act=pc-menu]') && e.querySelectorAll('dt').length === 4 && e.querySelector('.count .key')),
        ways: [r('[data-testid=sync-folder]').top === r('[data-testid=sync-hub]').top, Math.round(r('[data-testid=sync-folder]').width) === Math.round(r('[data-testid=sync-hub]').width)],
        order: r('[data-testid=computer-cards]').bottom <= r('[data-testid=computers]').top && r('.ways').bottom <= r('[data-key=setting-transcripts]').top,
        ctl: [...document.querySelectorAll('[data-testid=computers] .srow .ctl')].map((e) => Math.round(e.getBoundingClientRect().left)), primary: [...document.querySelectorAll('#sheet .btn.primary')].map((b) => b.textContent),
        low: [...document.querySelectorAll('#sheet button, #sheet .sw')].filter((e) => !e.closest('.b-tip') && e.getBoundingClientRect().height < 28).map((e) => e.textContent + ' ' + e.getBoundingClientRect().height),
        wide: document.getElementById('sheet').scrollWidth <= document.getElementById('sheet').clientWidth,
        sw: [...document.querySelectorAll('[data-testid=computers] .sw input')].map((i) => [i.getAttribute('aria-label'), i.getAttribute('role'), getComputedStyle(i.parentElement.querySelector('b'), '::after').content === (i.checked ? '"On"' : '"Off"')].join()) }`,
    [await exec("return import('./core.js').then((c) => c.S.cache.computers.this.name)")]);
    assert.deepEqual(lay.first, [true, 'This computer'], 'this computer first, marked');
    assert.ok(lay.n >= 2 && lay.row && lay.widths.length === 1 && lay.heads, 'a card per computer, three to a row and equally wide, each with its dot, its ⋯, four facts and a key number: ' + JSON.stringify(lay));
    assert.deepEqual(lay.ways, [true, true], 'the two ways side by side');
    assert.ok(lay.order, 'the cards, then how the computers sync: the ways over the switches');
    assert.ok(new Set(lay.ctl).size === 1 && lay.ctl.length === 3, 'three switches in one column: ' + lay.ctl);
    assert.deepEqual(lay.sw, ['Carry transcripts,switch,true', 'Share what runs here,switch,true', 'Connect directly,switch,true'], 'each switch is named and says On or Off');
    assert.deepEqual(lay.primary, ['Pair a computer…'], 'one primary button');
    assert.deepEqual(lay.low, [], 'every control at least 28 px tall');
    assert.ok(lay.wide, 'nothing wider than the page');
    await shot('60-light-computers');
    await exec("document.documentElement.dataset.theme = 'dark'");
    await shot('60-dark-computers');
    await exec("document.documentElement.dataset.theme = 'light'");
    // the head, and the ⋯ of this computer and of another
    await click('[data-testid=pair-head]');
    await dialog('Pair a computer');
    assert.deepEqual(await menu('[data-testid=computer-cards] .pc [data-act=pc-menu]'), ['Rename…', 'Sync now', 'Details…']);
    await click('.menu [data-testid=pc-rename]');
    await dialog('Rename this computer');
    await click('[data-testid=computer-cards] .pc [data-act=pc-menu]');
    await click('.menu [data-testid=pc-details]');
    await dialog('Details of ');
    const peer = await menu('[data-testid=computer-cards] .pc:nth-child(2) [data-act=pc-menu]');
    assert.ok(peer[0] === 'Sync now' && peer.at(-1) === 'Forget… (danger)', 'another computer: Sync now first, Forget… last and in the danger colour: ' + peer);
    await click('.menu [data-testid^=forget-]');
    await dialog('Forget ');
    // how the computers sync
    assert.deepEqual(await menu('[data-testid=sync-folder-menu]'), ['Stop syncing… (danger)']);
    await click('.menu [data-testid=sync-off]');
    await dialog('Stop syncing');
    await exec('window.__picked = 0; window.__open = window.__TAURI__.dialog.open; window.__TAURI__.dialog.open = async () => { window.__picked++; return null; }');
    try {
      await click('[data-testid=sync-folder-pick]');
      await waitFor(() => exec('return window.__picked === 1'), 'Change… asks for a folder');
    } finally { await exec('window.__TAURI__.dialog.open = window.__open'); }
    await click('[data-testid=sync-hub] [data-testid=hub-join]');
    await dialog('Join a hub');
    // (a computer that was on a hub keeps its recovery key: the hub's ⋯ shows it, behind a confirm, and takes another)
    if (await exec("return !!document.querySelector('[data-testid=sync-hub-menu]')")) {
      assert.deepEqual(await menu('[data-testid=sync-hub-menu]'), ['Show the recovery key…', 'Enter the recovery key…']);
      await click('.menu [data-testid=ring-show]');
      await dialog('Show the recovery key');
      await click('[data-testid=sync-hub-menu]');
      await click('.menu [data-testid=ring-enter]');
      await dialog('Enter the recovery key');
    }
    await click('[data-testid=listen-port-change]');
    await dialog('Listen port');
    await click('[data-testid=firstrun-open]');
    await dialog('Add this computer to your others');
  });

  await test('Enter on a button inside a keyboard row presses the button, not the row', async () => {
    await nav('#accounts');
    await $('.acct-row [data-act=start]');
    const name = await exec("return document.querySelector('.acct-row [data-act=start]').dataset.name");
    const launches = () => readLines(termLog).filter((t) => JSON.stringify(t.args) === JSON.stringify(['run', name])).length;
    const before = launches();
    // the keyboard's way there: the row, then Tab to its Start
    await page.focus(`[data-testid="row-${name}"]`);
    for (let i = 0; i < 4 && (await exec('return document.activeElement.dataset.act')) !== 'start'; i++) await page.keyboard.press('Tab');
    assert.equal(await exec('return document.activeElement.dataset.testid'), 'start-' + name, 'Tab reached the row\'s Start');
    await page.keyboard.press('Enter');
    await waitFor(async () => launches() === before + 1 || (await exec('return location.hash')) !== '#accounts', 'Enter did something');
    assert.equal(await exec('return location.hash'), '#accounts', 'the page did not move to the account');
    assert.equal(launches(), before + 1, 'the terminal log got the launch');
    // the row itself still answers Enter: it selects the row (as a click does), and Enter on the selected row opens the account
    await page.focus(`[data-testid="row-${name}"]`);
    await page.keyboard.press('Enter');
    await waitFor(async () => (await exec('return location.hash')) === '#accounts?sel=' + name, 'Enter on the row selects it');
    await waitFor(async () => (await exec("const a = document.activeElement; return a.getAttribute('aria-current') && a.dataset.sel")) === name, 'the row is drawn as the selected one, with the focus on it');
    await page.keyboard.press('Enter');
    await waitFor(async () => (await exec('return location.hash')).startsWith('#account?name='), 'Enter on the selected row opens the account');
  });

  await test('Enter opens a Needs attention item, and a chosen filter chip says it is pressed', async () => {
    // two tools with an update, as a check for updates leaves it, their versions not read yet: the Overview lists
    // that under Needs attention as one entry, whose button opens Apps
    await exec(`const t = window.__TAURI__; window.__core = t.core;
      t.core = { ...t.core, invoke: (cmd, a, o) => window.__core.invoke(cmd, a, o).then((r) => (a && a.op === 'state' ? { ...r, app_updates: 2 } : a && a.op === 'apps' ? { ...r, latest: null } : r)) };`);
    try {
      await nav('#accounts');
      await nav('#home');
      const item = '[data-testid=attention] [data-testid=att-updates] a[href="#apps"]';
      await $(item);
      await page.focus(item);
      await page.keyboard.press('Enter');
      await waitFor(async () => (await exec('return location.hash')) === '#apps', 'the Apps page opened');
    } finally { await exec('window.__TAURI__.core = window.__core'); }
    const pressed = (css) => exec('return [...document.querySelectorAll(arguments[0])].map((b) => b.getAttribute("aria-pressed"))', [css]);
    await nav('#home');
    await $('[data-testid=tool-all]');
    assert.deepEqual([...new Set(await pressed('[data-act=home-tool]'))], ['true', 'false'], 'the Overview\'s tool chips: All pressed, the others not');
    await nav('#accounts');
    await $('[data-testid=chip-all]');
    assert.deepEqual([...new Set(await pressed('[data-act=acct-tool]'))], ['true', 'false'], 'the Accounts tool chips: All pressed, the others not');
  });

  await test('a read that fails behind a page on screen leaves the page and says so in the notice bar', async () => {
    const name = anAccount();
    await nav(`#account?name=${name}&tab=settings`);
    await $('#pf-email');
    // the reads of this account fail while this is on, as a locked database would make them
    const locked = (on) => exec(`const t = window.__TAURI__; window.__core ||= t.core;
      t.core = arguments[0] ? { ...t.core, invoke: (cmd, a, o) => (a && a.op === 'detail' ? Promise.reject(new Error('the database is locked')) : window.__core.invoke(cmd, a, o)) } : window.__core;`, [on]);
    const said = () => exec("return document.getElementById('notice').textContent.includes('the database is locked')");
    await locked(true);
    try {
      await refreshed();
      const shown = await waitFor(async () => ((await exec("return !!document.querySelector('#sheet [data-act=reload]')")) ? 'the error page' : (await said()) ? 'the notice' : ''), 'the failed read shown');
      assert.equal(shown, 'the notice', 'the failure is in the notice bar');
      assert.ok(await exec("return !!document.getElementById('pf-email')"), 'the page is still on screen');
      // the read works again: the notice goes with the next redraw
      await locked(false);
      await refreshed();
      await waitFor(async () => !(await said()), 'the notice gone once the page reads again');
      // moving to a page whose read fails has no page to keep: the error page, with Try again
      await locked(true);
      await nav(`#account?name=${name}&tab=overview`);
      await $('#sheet [data-act=reload]');
    } finally { await locked(false); }
    await click('#sheet [data-act=reload]');
    await $('[data-testid=detail-name]');
  });

  await test('a menu item that fails says so in the notice bar', async () => {
    await nav('#accounts');
    await $('.acct-row [data-act=account-menu]');
    await exec(`const t = window.__TAURI__; window.__core = t.core;
      t.core = { ...t.core, invoke: (cmd, a, o) => (a && a.op === 'open_folder' ? Promise.reject(new Error('the folder is gone')) : window.__core.invoke(cmd, a, o)) };`);
    try {
      await click('.acct-row [data-act=account-menu]');
      await page.click('.menu button:has-text("Open folder")');
      await hasText('#notice', 'the folder is gone');
    } finally { await exec('window.__TAURI__.core = window.__core'); }
  });

  await test('a project name being typed in place is not wiped by a background refresh', async () => {
    await nav('#projects?days=all');
    await click('[data-testid=project-cards] [data-act=project-rename]');
    await page.fill('input.a-rename', 'Half typed');
    await refreshed();
    await sleep(6000); // a background redraw comes within four seconds, and its reads take a moment
    assert.equal(await exec("return (document.querySelector('input.a-rename') || {}).value"), 'Half typed', 'the field is still open with what was typed');
    await page.keyboard.press('Escape');
    await waitFor(async () => !(await exec("return !!document.querySelector('input.a-rename')")), 'the rename closed');
    await $('[data-testid=project-cards] .a-pcard');
  });

  await test('the Claude sign-in dialog is drawn again only when what it shows changes', async () => {
    await nav('#accounts');
    await click('[data-testid=add-account]');
    await type('[data-testid=add-name]', 'steady-me');
    await click('[data-testid=add-create]');
    try {
      await $('[data-testid=login-code]');
      // focus outside the code field (inside it, the dialog was always left alone)
      await page.focus('#lg-cancel');
      await exec("window.__lg = document.getElementById('lg-cancel')");
      await sleep(2500); // two answers of the sign-in poll
      assert.ok(await exec('return window.__lg.isConnected && document.activeElement === window.__lg'), 'the same dialog, the focus where it was');
      await click('.dlg .dh [data-act=dialog-close]');
    } finally { hrm('remove', 'steady-me', '--delete-home'); }
    await nav('#home');
    await nav('#accounts');
  });

  await test('hidden in the tray, the window draws nothing; shown again, it catches up', async () => {
    const win = (call) => exec(`return window.__TAURI__.window.getCurrentWindow().${call}`);
    await nav('#accounts');
    await $('[data-testid=accounts-table]');
    await win('close()'); // what the title bar's X does
    // what the tray icon does: show the window and give it the focus
    const show = () => exec("return window.__TAURI__.core.invoke('call', { op: 'open_main', args: {} })");
    let n;
    try {
      await waitFor(async () => (await win('isVisible()')) === false, 'hidden');
      // (WebView2 goes on calling a page hidden this way visible, so the app asks the window instead)
      await sleep(5000); // a redraw already on its way when the window hid ends
      n = await renders();
      await refreshed();
      await sleep(6000);
      assert.equal(await renders(), n, 'no redraw while hidden');
    } finally { await show(); }
    // the focus the tray's show gives the window is the app's sign that it is back; Windows does not always give it
    // to a window a test shows, so the pointer moves over the window too, the app's other sign
    await page.mouse.move(400, 300);
    await page.mouse.move(420, 320);
    await waitFor(async () => (await renders()) > n, 'the redraw that waited, once the window shows', 15000);
  });

  // (last: its two folders are projects of every later list)
  await test('two projects of one name are told apart: in the Sessions groups, the Project picker and the Projects rows', async () => {
    // two folders named api, neither a repository: nothing but their paths tells them apart
    const dirs = ['acme', 'beta'].map((d) => path.join(home, 'Work', d, 'api'));
    const files = dirs.map((dir, i) => {
      fs.mkdirSync(dir, { recursive: true });
      const f = path.join(home, '.claude', 'projects', 'p', `s-twin-${i}.jsonl`);
      write(f, jl([{ type: 'user', sessionId: `s-twin-${i}`, cwd: dir, timestamp: new Date().toISOString(), message: { role: 'user', content: `Twinned session ${i}` } }]));
      return f;
    });
    try {
      hrm('index', 'refresh');
      const want = ['acme\\api', 'beta\\api'];
      // Sessions, grouped by project: two groups named api, each with the last two folders of its path beside the name
      await nav('#sessions?days=all&q=twinned');
      const heads = () => exec("return [...document.querySelectorAll('[data-testid=group]')].map((g) => [g.querySelector('.a-gname').textContent.trim(), (g.querySelector('[data-testid=group-hint]') || {}).textContent || ''])");
      await waitFor(async () => (await heads()).filter(([n]) => n === 'api').length === 2, 'two groups named api');
      assert.deepEqual((await heads()).filter(([n]) => n === 'api').map(([, h]) => h).sort(), want, 'a different second label on each');
      await shot('61-sessions-same-name');
      // the Project picker: the two carry it too; a project whose name is its own carries nothing
      await click('[data-testid=pick-project]');
      if (await exec("return !!document.querySelector('.menu [data-testid=pick-more]')")) await click('.menu [data-testid=pick-more]');
      await $('.menu [data-testid=pick-all]');
      const picks = await exec("return [...document.querySelectorAll('.menu [role=menuitem]')].map((b) => [(b.querySelector('.a-pkn > .trunc') || {}).textContent || '', (b.querySelector('.a-pkn small') || {}).textContent || ''])");
      assert.deepEqual(picks.filter(([n]) => n === 'api').map(([, h]) => h).sort(), want, 'in the picker: ' + JSON.stringify(picks));
      // every project of the menu: a name that is its own carries nothing; a name two share never reads the same twice
      const projects = picks.filter(([n]) => n), count = (n) => projects.filter(([m]) => m === n).length;
      assert.ok(projects.some(([n]) => count(n) === 1) && projects.every(([n, h]) => count(n) > 1 || h === ''), 'no second label where the name is enough: ' + JSON.stringify(projects));
      assert.equal(new Set(projects.map((p) => p.join(' '))).size, projects.length, 'no two items read the same: ' + JSON.stringify(projects));
      await shot('61b-picker-same-name');
      await page.keyboard.press('Escape');
      // Projects: each on its own row, the folders beside the name and its own path under it
      await nav('#projects?days=all&per=50');
      const rows = () => exec("return [...document.querySelectorAll('.a-pcard')].filter((r) => r.querySelector('.a-pname').textContent === 'api').map((r) => [(r.querySelector('[data-testid=dup-hint]') || {}).textContent || '', r.querySelector('.a-path').textContent])");
      await waitFor(async () => (await rows()).length === 2, 'two rows named api');
      const r = await rows();
      assert.deepEqual(r.map(([h]) => h).sort(), want, 'beside the name: ' + JSON.stringify(r));
      assert.ok(r[0][1] !== r[1][1] && r.every(([, p]) => p.endsWith('api')), 'and each its own path under it: ' + JSON.stringify(r));
      const all = await exec("return [...document.querySelectorAll('.a-pcard')].map((r) => [r.querySelector('.a-pname').textContent, (r.querySelector('[data-testid=dup-hint]') || {}).textContent || '', r.querySelector('.a-path').textContent].join(' | '))");
      assert.equal(new Set(all).size, all.length, 'no two rows read the same: ' + JSON.stringify(all));
      await shot('61c-projects-same-name');
    } finally { for (const f of files) fs.rmSync(f, { force: true }); hrm('index', 'refresh'); }
  });

  // ---------- the command palette (palette.js): Ctrl K, "Run a command" in the top bar ----------
  const palOpen = async () => { await page.keyboard.press('Control+k'); await $('[data-testid=palette]'); };
  const palGone = () => waitFor(async () => !(await exec("return !!document.querySelector('[data-testid=palette]')")), 'the palette closed');
  /** The rows' titles, in order; palChosen is the chosen row's place. */
  const palRows = () => exec("return [...document.querySelectorAll('[data-testid=pal-row]')].map((r) => r.querySelector('.trunc > b').textContent)");
  const palChosen = () => exec("return [...document.querySelectorAll('[data-testid=pal-row]')].findIndex((r) => r.classList.contains('on'))");
  const palType = (t) => page.fill('[data-testid=pal-q]', t);
  const addressIs = (h) => waitFor(async () => (await exec('return location.hash')) === h, 'the address ' + h);
  /** The top bar's search emptied: what an earlier test typed into it stays in it from page to page. */
  const clearFind = () => exec("const f = document.getElementById('find'); if (f.value) { f.value = ''; f.dispatchEvent(new Event('input')); }");

  await test('command palette: Ctrl K opens it, also from a field, as a dialog with a listbox; arrows move, a resting pointer does not; Esc returns the focus; / moves to the page filter and is a character in a field', async () => {
    await nav('#accounts');
    await clearFind();
    await $('[data-testid=accounts-table]');
    // the top bar: the search keeps its job and its words, its hint is "/", and "Run a command" with Ctrl K stands beside it
    assert.deepEqual(await exec("return [document.querySelector('.search-box .kbd').textContent, document.getElementById('find').placeholder, document.querySelector('[data-testid=palette-open]').textContent]"), ['/', 'Filter accounts', 'Run a commandCtrl K']);
    await shot('70-palette-topbar');
    // a narrow window keeps the button's key and drops its words, and the top bar holds everything
    const bar = () => exec("const t = document.querySelector('.topbar'), b = document.querySelector('[data-testid=palette-open]'); return [innerWidth, getComputedStyle(b.querySelector('.cmd-label')).display !== 'none', b.querySelector('kbd').getBoundingClientRect().width > 0, b.getBoundingClientRect().height, t.scrollWidth <= t.clientWidth, b.getAttribute('aria-label')]");
    assert.deepEqual(await bar(), [1280, true, true, 28, true, 'Run a command'], 'at 1280: the words and the key, 28 px tall');
    await page.setViewportSize({ width: 1000, height: 800 });
    try { assert.deepEqual(await bar(), [1000, false, true, 28, true, 'Run a command'], 'at 1000: the key alone, still named'); } finally { await page.setViewportSize({ width: 1280, height: 800 }); }
    await page.focus('#find');
    await palOpen();
    const d = await exec(`const p = document.querySelector('[data-testid=palette]'), q = document.getElementById('pal-q'), on = p.querySelector('.pal-r.on');
      return { is: [p.getAttribute('role'), p.getAttribute('aria-modal'), p.getAttribute('aria-label'), document.activeElement.id, q.getAttribute('role'), document.getElementById(q.getAttribute('aria-controls')).getAttribute('role')],
        chosen: q.getAttribute('aria-activedescendant') === on.id && on.getAttribute('role') === 'option' && on.getAttribute('aria-selected') === 'true', inert: document.querySelector('.frame').inert,
        foot: p.querySelector('.pal-f').textContent.replace(/\\s+/g, ''), hints: [...p.querySelectorAll('.pal-r kbd')].map((k) => k.textContent).slice(0, 3), n: p.querySelectorAll('.pal-r').length };`);
    assert.deepEqual(d.is, ['dialog', 'true', 'Command palette', 'pal-q', 'combobox', 'listbox'], 'a dialog; the focus in its box, which owns the list');
    assert.ok(d.chosen && d.inert, 'the first row is chosen and named to a screen reader; the page behind is inert');
    assert.deepEqual(d.hints, ['↵', 'Ctrl+2', 'Ctrl+3'], '↵ on the chosen row, Ctrl+N on the next');
    assert.ok(/^↑↓move↵runEscclose(\d+of)?\d+results$/.test(d.foot), 'the foot: ' + d.foot);
    // the arrows move the choice, the ↵ with it, around the ends
    await page.keyboard.press('ArrowDown');
    assert.deepEqual([await palChosen(), await exec("return [...document.querySelectorAll('.pal-r kbd')].map((k) => k.textContent).slice(0, 3)"), await exec("return document.getElementById('pal-q').getAttribute('aria-activedescendant')")], [1, ['Ctrl+1', '↵', 'Ctrl+3'], 'pal-r1']);
    await page.keyboard.press('ArrowUp');
    await page.keyboard.press('ArrowUp');
    assert.equal(await palChosen(), d.n - 1, 'up from the first row is the last');
    // a pointer resting where a row is drawn does not take the choice; moving it does
    const at = await exec("const r = document.getElementById('pal-r2').getBoundingClientRect(); return [r.left + 200, r.top + 20]");
    await page.keyboard.press('Escape');
    await palGone();
    assert.equal(await exec('return document.activeElement.id'), 'find', 'Esc returns the focus to where it was');
    await page.mouse.move(at[0], at[1]);
    await palOpen();
    await sleep(300);
    assert.equal(await palChosen(), 0, 'rows drawn under a resting pointer: the first row stays chosen');
    await page.mouse.move(at[0] + 4, at[1] + 2);
    await page.mouse.move(at[0] + 8, at[1] + 3);
    await waitFor(async () => (await palChosen()) === 2, 'the pointer moved: its row is chosen');
    // Ctrl K again closes it; the button opens it, and a click outside closes it with the focus back on the button
    await page.keyboard.press('Control+k');
    await palGone();
    await page.mouse.move(640, 700);
    await click('[data-testid=palette-open]');
    await $('[data-testid=palette]');
    await page.mouse.click(100, 700);
    await palGone();
    assert.equal(await exec('return document.activeElement.dataset.testid'), 'palette-open');
    // a dialog is modal: Ctrl K leaves it as it is
    await click('[data-testid=add-account]');
    await $('[data-testid=add-name]');
    await page.keyboard.press('Control+k');
    await sleep(200);
    assert.ok(await exec("return !!document.querySelector('[data-testid=add-name]') && !document.querySelector('[data-testid=palette]')"), 'no palette over a dialog');
    await page.keyboard.press('Escape');
    // "/" moves to the page's filter when no field has the keys, and is not typed; in a field it is a character
    await exec("document.activeElement.blur(); document.getElementById('sheet').focus()");
    await page.keyboard.press('/');
    assert.deepEqual(await exec("return [document.activeElement.id, document.getElementById('find').value]"), ['find', '']);
    await page.keyboard.type('a/b');
    assert.equal(await exec("return document.getElementById('find').value"), 'a/b', 'a / typed in the filter stays in it');
    await page.fill('[data-testid=find]', '');
    await exec(`document.getElementById('find').dispatchEvent(new Event('input'))`);
    await nav('#settings');
    await $('[data-testid=set-find]');
    await page.focus('[data-testid=set-find]');
    await page.keyboard.type('/');
    assert.deepEqual(await exec("return [document.activeElement.id, document.getElementById('set-find').value]"), ['set-find', '/'], 'and in Search settings');
    await page.fill('[data-testid=set-find]', '');
  });

  await test('command palette: with nothing typed it lists the useful actions and then the pages; typed words find a Settings row of another section, and Enter lands on that section', async () => {
    await nav('#settings?tab=backups');
    await $('[data-testid=sec-backups]');
    await palOpen();
    const e = await exec(`const g = [...document.querySelectorAll('#pal-list [role=group]')].map((x) => [x.querySelector('.pal-g').textContent, [...x.querySelectorAll('.pal-r')].map((r) => r.querySelector('.trunc').textContent)]);
      return { g, icon: !!document.querySelector('#pal-r0 .pal-ic img.ticon'), page: !!document.querySelector('#pal-list [role=group]:last-child .pal-ic svg') };`);
    assert.deepEqual(e.g.map(([n]) => n), ['Actions', 'Go to'], 'two groups: ' + JSON.stringify(e.g));
    assert.ok(/^Start \S+ · recommended for /.test(e.g[0][1][0]) && e.icon, 'first, the recommended account of a tool, with the tool\'s own icon: ' + e.g[0][1][0]);
    assert.ok(e.g[0][1].includes('Start in a folder…') && e.g[0][1].some((r) => r.startsWith('New session in ')), 'then Start in a folder… and a new session in the latest project: ' + e.g[0][1]);
    assert.ok(e.g[1][1][0].startsWith('Overview') && e.g[1][1].length >= 3 && e.page, 'then the pages, with the sidebar\'s icons: ' + e.g[1][1]);
    assert.ok(e.g[0][1].length + e.g[1][1].length <= 9, 'nine rows at most');
    await palType('LAUNCH at');
    await waitFor(async () => (await palRows())[0] === 'Settings › Launch at sign-in', 'the row of Window and terminal, found from Backups');
    assert.deepEqual(await exec("const r = document.querySelector('.pal-r.on'); return [r.closest('[role=group]').querySelector('.pal-g').textContent, [...r.querySelectorAll('b mark')].map((m) => m.textContent).join('|'), getComputedStyle(r.querySelector('mark')).textDecorationLine, r.querySelector('.sub').textContent]"),
      ['Go to', 'Launch|at', 'underline', ' · Starts hidden in the tray.'], 'under Go to, each typed word underlined whatever its case, the row\'s help line beside it');
    await page.keyboard.press('Enter');
    await addressIs('#settings?tab=window');
    await $('[data-testid=sec-window] [data-testid=st-autostart]');
    await palGone();
    // the palette's rows are the ones Search settings looks through: the same labels, section by section
    const both = await exec(`return import('./settings.js').then(async (s) => { const mine = (await s.settingRows()).find((x) => x.id === 'window').rows.map((r) => r.label);
      return [mine, [...document.querySelectorAll('[data-testid=sec-window] .srow .what .row > b')].map((b) => b.textContent)]; })`);
    assert.deepEqual(both[0], both[1]);
  });

  await test('command palette: Enter on "Start <account>" starts it as its Start button does, and Ctrl+3 runs the third row', async () => {
    await nav('#accounts');
    await clearFind();
    await $('.acct-row [data-act=start]');
    const [name, label] =await exec("const b = document.querySelector('.acct-row [data-act=start]'); return [b.dataset.name, b.closest('.acct-row').querySelector('.who b').textContent]");
    const launches = () => readLines(termLog).filter((t) => JSON.stringify(t.args) === JSON.stringify(['run', name])).length;
    const before = launches();
    await palOpen();
    await palType('start ' + label);
    const i = await waitFor(async () => { const k = (await palRows()).indexOf('Start ' + label); return k >= 0 ? k + 1 : 0; }, 'Start ' + label + ' among the rows');
    for (let k = 1; k < i; k++) await page.keyboard.press('ArrowDown');
    assert.equal(await exec("return document.querySelector('.pal-r.on').closest('[role=group]').querySelector('.pal-g').textContent"), 'Actions');
    await page.keyboard.press('Enter');
    await palGone();
    await waitFor(() => launches() === before + 1, 'the terminal log got the launch');
    await hasText('#toast', 'Opened a terminal for ' + label);
    assert.equal(await exec('return location.hash'), '#accounts', 'the page stays');
    // Ctrl+3: the third row, without moving to it. "limits": Refresh limits, the Limits section, then the row of Appearance
    await palOpen();
    await palType('limits');
    await waitFor(async () => (await palRows())[2] === 'Settings › Limits shown as', 'the third row');
    assert.deepEqual((await palRows()).slice(0, 2), ['Refresh limits', 'Settings › Limits'], 'a title that starts with the word comes before one that has it later');
    await page.keyboard.press('Control+3');
    await addressIs('#settings?tab=appearance');
    await palGone();
  });

  await test('command palette: the Sessions group finds a session by title, newest first and five at most, a running one says so; a title with <b> in it is shown as text; nine rows at most; typing asks the backend for sessions only', async () => {
    const t = new Date().toISOString(), f = path.join(home, '.claude', 'projects', 'p', 's-palette-1.jsonl');
    const title = '<b>Quince</b> probe & <img src=x onerror=alert(1)>';
    write(f, jl([
      { type: 'user', sessionId: 's-palette-1', cwd: repo, timestamp: t, message: { role: 'user', content: 'Quince probe' } },
      { type: 'assistant', sessionId: 's-palette-1', cwd: repo, timestamp: t, message: { id: 'q1', model: 'claude-opus-5-5', usage: { input_tokens: 4, output_tokens: 2 }, content: [{ type: 'text', text: 'Done.' }] } },
      { type: 'ai-title', aiTitle: title, sessionId: 's-palette-1' },
    ]));
    try {
      hrm('index', 'refresh');
      await nav('#home');
      await drawn('the Overview');
      // every operation asked of the backend from here on; "zebra" is answered here with seven sessions, one of them running
      await exec(`const t = window.__TAURI__, now = Date.now(); window.__core = t.core; window.__ops = [];
        const zebra = { total: 7, groups: [{ key: 'd', name: 'd', sessions: Array.from({ length: 7 }, (_, i) => ({ profile: 'claude-main', id: 'z' + i, harness: 'claude', title: 'Zebra run ' + i, project_name: 'zoo', last_active_at: now - (6 - i) * 60000, open: i === 6 })) }] };
        const late = (ms) => new Promise((r) => setTimeout(r, ms)); // the projects answer late, as on a busy database
        t.core = { ...t.core, invoke: (cmd, a, o) => { window.__ops.push(a && a.op); return a && a.op === 'sessions' && a.args.filter.query === 'zebra' ? Promise.resolve(zebra) : a && a.op === 'projects' ? late(400).then(() => window.__core.invoke(cmd, a, o)) : window.__core.invoke(cmd, a, o); } };`);
      await palOpen();
      // a row chosen before the projects arrive is still the chosen one after the list is drawn again with them
      const second = (await palRows())[1];
      await page.keyboard.press('ArrowDown');
      await sleep(800);
      assert.deepEqual([await palChosen(), (await palRows())[1]], [1, second], 'rows that arrive do not move the choice');
      const opened = await exec('return window.__ops.splice(0)');
      assert.ok(opened.filter((o) => o === 'projects').length === 1 && !opened.includes('sessions') && !opened.includes('state'), 'opening reads the projects once, and no session: ' + opened);
      await page.keyboard.type('quince probe', { delay: 25 });
      await waitFor(async () => (await palRows()).includes(title), 'the session, by its title');
      await sleep(400);
      const typed = await exec('return window.__ops.splice(0)');
      assert.ok(typed.filter((o) => o === 'sessions').length <= 2 && !typed.some((o) => ['projects', 'state', 'backups', 'archive_status', 'home', 'detail', 'limits_extra'].includes(o)), 'twelve keys: the session search after a pause, nothing else: ' + typed);
      const s = await exec(`const r = [...document.querySelectorAll('[data-testid=pal-row]')].find((x) => x.textContent.includes('Quince'));
        return { g: r.closest('[role=group]').querySelector('.pal-g').textContent, inside: [...r.querySelectorAll('.trunc > b *')].map((x) => x.tagName + ':' + x.textContent), img: document.querySelectorAll('#pal-list img:not(.ticon)').length, sub: r.querySelector('.sub').textContent };`);
      assert.deepEqual([s.g, s.inside, s.img], ['Sessions', ['MARK:Quince', 'MARK:probe'], 0], 'under Sessions; the markup of its title is text, only the typed words are marked');
      assert.ok(s.sub.includes('platform-gitops') && s.sub.includes('Today'), 'its project and when: ' + s.sub);
      // Enter opens the session's page
      await page.keyboard.press('Enter');
      await addressIs('#session?profile=claude-main&id=s-palette-1');
      await palGone();
      // newest first, five at most, and the running one says so
      await palOpen();
      await palType('zebra');
      await waitFor(async () => (await palRows()).includes('Zebra run 6'), 'the sessions found');
      assert.deepEqual(await exec("return [...document.querySelectorAll('#pal-list [role=group]:last-child .pal-r .trunc')].map((r) => r.textContent.split(' · ').slice(0, 2).join(' · '))"),
        ['Zebra run 6 · running now', 'Zebra run 5 · zoo', 'Zebra run 4 · zoo', 'Zebra run 3 · zoo', 'Zebra run 2 · zoo']);
      assert.equal(await text('[data-testid=pal-count]'), '5 of 7 results');
      // nine rows at most, whatever is typed
      for (const q of ['', 's', 'a', 'se', 'start', 'settings']) {
        await palType(q);
        await sleep(450);
        const [n, count] = await exec("return [document.querySelectorAll('[data-testid=pal-row]').length, document.querySelector('[data-testid=pal-count]').textContent]");
        assert.ok(n > 0 && n <= 9, `"${q}": ${n} rows`);
        const m = /^(?:(\d+) of )?(\d+) results?$/.exec(count);
        assert.ok(m && +(m[1] || m[2]) === n && +m[2] >= n, `"${q}": the count says how many of how many: ${count}`);
      }
      assert.equal(await exec("return document.querySelectorAll('[data-testid=pal-row]').length"), 9, 'a word with many matches fills the nine');
      await palType('zzzz-nothing');
      await hasText('[data-testid=pal-none]', 'Nothing matches.');
      await page.keyboard.press('Enter'); // nothing to run: it stays
      assert.ok(await exec("return !!document.querySelector('[data-testid=palette]')"));
      await page.keyboard.press('Escape');
      await palGone();
    } finally { await exec('if (window.__core) window.__TAURI__.core = window.__core'); fs.rmSync(f, { force: true }); hrm('index', 'refresh'); }
  });

  await test('command palette: it measures as its drawing (640 px near the top, 40 px rows, nothing under 12 px), in every theme', async () => {
    await nav('#home');
    await drawn('the Overview');
    const was = await exec("return import('./core.js').then((c) => [c.S.state.settings.brand || 'hm', c.S.state.settings.theme])");
    const look = (brand, theme) => exec("return import('./core.js').then(async (c) => { await c.call('settings_set', { key: 'brand', value: arguments[0] }); await c.call('settings_set', { key: 'theme', value: arguments[1] }); await c.loadState(); })", [brand, theme]);
    try {
      for (const [brand, theme] of [['hm', 'dark'], ['hm', 'light'], ['vg', 'dark'], ['vg', 'light']]) {
        await look(brand, theme);
        await page.mouse.move(640, 760);
        await palOpen();
        await sleep(500);
        await shot(`70-palette-empty-${brand}-${theme}`);
        await palType('start');
        await sleep(600);
        await shot(`70-palette-start-${brand}-${theme}`);
        const d = await exec(`const cs = (e) => getComputedStyle(e), q = (s) => document.querySelector(s), box = (s) => q(s).getBoundingClientRect(), small = [];
          for (const el of document.querySelectorAll('[data-testid=palette] *')) if ([...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim()) && parseFloat(cs(el).fontSize) < 12) small.push(el.className + ' ' + cs(el).fontSize);
          return { card: [box('[data-testid=palette]').left, box('[data-testid=palette]').top, box('[data-testid=palette]').width], input: [box('.pal-in').height, cs(q('#pal-q')).fontSize], row: [box('.pal-r').height, cs(q('.pal-r')).fontSize, cs(q('.pal-r b')).fontWeight],
            group: cs(q('.pal-g')).fontSize, foot: [box('.pal-f').height, cs(q('.pal-f')).fontSize], kbd: cs(q('.pal-r kbd')).fontSize, small, fits: box('[data-testid=palette]').bottom <= innerHeight,
            cut: [...document.querySelectorAll('.pal-r')].filter((r) => r.scrollWidth > r.clientWidth + 1).length, bg: cs(q('[data-testid=palette]')).backgroundColor, canvas: cs(document.body).backgroundColor };`);
        assert.deepEqual(d.card, [320, 8, 640], `${brand} ${theme}: 640 px wide, centred, 8 px from the top`);
        assert.deepEqual([d.input, d.row, d.group, d.foot, d.kbd], [[52, '15px'], [40, '13px', '700'], '12px', [38, '12px'], '12px'], `${brand} ${theme}: the box 52 px at 15, rows 40 px at 13 with a bold title, the foot 38 px`);
        assert.deepEqual([d.small, d.fits, d.cut], [[], true, 0], `${brand} ${theme}: nothing under 12 px, inside the window, no row wider than the card`);
        assert.notEqual(d.bg, d.canvas, `${brand} ${theme}: the card stands off the page`);
        await page.keyboard.press('Escape');
        await palGone();
      }
    } finally { await look(was[0], was[1]); }
  });

  exitCode =results.some((r) => r[1] !== 'ok') ? 1 : 0;
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
