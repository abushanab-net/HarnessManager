// Designer A · round 2 · Sessions, Session (live), Projects. Registers three pages on window.HM.pages and
// extends window.HM.data with its own fixtures (aProjects, aSessions, aTags, aTranscripts). Touches no lead file.
(function () {
  'use strict';
  const H = window.HM, D = H.data, esc = D.esc;
  document.head.appendChild(Object.assign(document.createElement('link'), { rel: 'stylesheet', href: 'pages-a.css' }));

  const MIN = D.MIN, HR = D.H, DAY = D.DAY, NOW = D.NOW, HERE = 'developmentpc', PER = 50;
  const TOOL = Object.fromEntries(D.TOOLS.map((t) => [t.id, t.name]));
  const PC = (id) => D.COMPUTERS.find((c) => c.id === id);
  const pcName = (id) => PC(id)?.name || id;
  const rng = (seed) => () => { seed = (seed * 16807) % 2147483647; return (seed - 1) / 2147483646; };
  const hash = (s) => [...String(s)].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 2147483647, 7) || 1;
  const plural = (n, one, many) => `${n} ${n === 1 ? one : many || one + 's'}`;
  const ICON = {
    pin: '<svg class="a-ico" viewBox="0 0 24 24"><path d="M9 4h6l-1 6 3 3v2H7v-2l3-3zM12 15v5"/></svg>',
    edit: '<svg class="a-ico" viewBox="0 0 24 24"><path d="M4 20h4l10-10-4-4L4 16zM13 7l4 4"/></svg>',
    play: '<svg class="a-ico" viewBox="0 0 24 24"><path d="M7 5v14l11-7z"/></svg>',
    tag: '<svg class="a-ico" viewBox="0 0 24 24"><path d="M3 12V4h8l9 9-8 8zM7 8h.01"/></svg>'
  };

  // ---------------------------------------------------------------- fixtures: the owner's projects, 3 computers
  const P = (name, root, remote, devices, tags, here = true) => ({ key: root || remote, name, root: here ? root : '', remote, devices, tags: tags || [], here, pinned: D.PINNED_PROJECTS.some((p) => p.name === name) });
  const PROJECTS = [
    P('HarnessManager', 'D:\\Projects\\abushanab-net\\HarnessManager', 'github.com/abushanab-net/HarnessManager', ['developmentpc', 'samymsi', 'macbook'], ['design']),
    P('Bunyan', 'D:\\Projects\\abushanab-net\\Bunyan', 'github.com/abushanab-net/bunyan', ['developmentpc', 'samymsi'], ['release']),
    P('AdminPortal', 'D:\\Projects\\Teksa\\AdminPortal', 'github.com/teksagit/adminportal', ['samymsi'], ['client-x'], false),
    P('sanduq-api', 'D:\\Projects\\resal\\sanduq-api', 'github.com/resal/sanduq-api', ['developmentpc', 'samymsi'], ['sanduq']),
    P('PropertyManagment', 'D:\\Projects\\Teksa\\PropertyManagment', 'github.com/teksagit/propertymanagment', ['samymsi'], ['client-x'], false),
    P('resal-wiki', 'D:\\Projects\\resal\\resal-wiki', '', ['developmentpc']),
    P('spark-serve', 'D:\\Projects\\dgx\\spark-serve', 'github.com/abushanab-net/spark-serve', ['developmentpc', 'macbook'], ['infra']),
    P('checkout-service', 'D:\\Projects\\resal\\checkout-service', 'github.com/resal/checkout-service', ['developmentpc', 'samymsi']),
    P('core-ledger', 'D:\\Projects\\resal\\core-ledger', '', ['developmentpc']),
    P('merchant-portal', 'D:\\Projects\\resal\\merchant-portal', 'github.com/resal/merchant-portal', ['developmentpc']),
    P('edge-gateway', 'D:\\Work\\teksa\\edge-gateway', 'github.com/teksagit/edge-gateway', ['samymsi'], [], false),
    P('telemetry-ui', 'D:\\Work\\teksa\\telemetry-ui', '', ['samymsi'], ['ui'], false),
    P('model-adoption', 'D:\\Projects\\model-adoption', '', ['developmentpc', 'samymsi']),
    P('synth-webgl', '~/Projects/personal/synth-webgl', '', ['macbook'], [], false),
    P('blog', 'D:\\Projects\\abushanab-net\\blog', 'github.com/abushanab-net/blog', ['developmentpc', 'macbook']),
    P('sanduq', 'D:\\Personal\\sanduq', '', ['developmentpc'], ['sanduq']),
    P('platform-gitops', 'D:\\Projects\\platform-gitops', 'github.com/abushanab-net/platform-gitops', ['developmentpc', 'samymsi'], ['infra']),
    P('hub-client', 'D:\\Projects\\abushanab-net\\hub-client', '', ['developmentpc']),
    P('security-audit', 'D:\\Projects\\resal\\security-audit', '', ['samymsi'], [], false),
    P('flutter-canvas', 'D:\\Projects\\experiments\\flutter-canvas', '', ['developmentpc']),
    P('backup', 'D:\\Projects\\backup', '', ['developmentpc']),
    P('index', 'D:\\Projects\\index', '', ['developmentpc']),
    P('ai', 'D:\\Projects\\ai', '', ['developmentpc'])
  ];
  const PROJECT_ACCOUNTS = {
    HarnessManager: ['claude-main', 'codex-main', 'opencode-main', 'service-account-resal'], Bunyan: ['claude-main', 'codex-main'], AdminPortal: ['claude-main', 'samykabu-teksa'],
    'sanduq-api': ['codex-main', 'service-account-resal', 'claude-main'], PropertyManagment: ['samykabu-teksa', 'claude-main'], 'resal-wiki': ['service-account-resal', 'claude-main'],
    'spark-serve': ['opencode-main'], 'checkout-service': ['sabushanab-resal'], 'core-ledger': ['claude-main'], 'merchant-portal': ['sabushanab-resal', 'claude-main'],
    'edge-gateway': ['samykabu-teksa'], 'telemetry-ui': ['samykabu-teksa'], 'model-adoption': ['codex-main', 'claude-main'], 'synth-webgl': ['samykabu-personal'], blog: ['samykabu-personal', 'opencode-main'],
    sanduq: ['claude-main'], 'platform-gitops': ['claude-main', 'codex-main'], 'hub-client': ['claude-main'], 'security-audit': ['claude-main'], 'flutter-canvas': ['agy-main'], backup: ['claude-main'], index: ['claude-main'], ai: ['claude-main']
  };
  const WEIGHTS = [['HarnessManager', 22], ['Bunyan', 18], ['sanduq-api', 9], ['AdminPortal', 8], ['PropertyManagment', 5], ['resal-wiki', 4], ['spark-serve', 4], ['checkout-service', 4], ['core-ledger', 3], ['merchant-portal', 3], ['edge-gateway', 2], ['telemetry-ui', 2], ['model-adoption', 2], ['synth-webgl', 2], ['blog', 2], ['sanduq', 3], ['platform-gitops', 2], ['hub-client', 1], ['security-audit', 1], ['flutter-canvas', 1], ['backup', 1], ['index', 1], ['ai', 1], ['', 2]];
  const MODELS = { claude: ['claude-opus-4-6', 'claude-sonnet-4-5', 'claude-sonnet-4-5', 'claude-haiku-4-5'], codex: ['gpt-5.3-codex', 'gpt-5.3-codex', 'o4-mini-high'], opencode: ['deepseek-v4', 'qwen-3-coder'], agy: ['gemini-3-pro'], pi: ['kimi-k2.5'], omp: ['kimi-k2.5'] };
  const LIVE_TITLES = { s1: 'Multi-agent UI redesign committee', s2: 'Settlement race in the payout worker', s3: 'Sync the architecture diagrams into the wiki', s4: 'Quantised vLLM serving on Spark', s5: 'Stripe v2 webhook idempotency', s6: 'Tray and sync in Tauri', s7: 'Partition the audit log by month', s8: 'OpenAPI spec for 14 endpoints', s9: 'Visual regression suite in Playwright', s10: 'RTL polish on the merchant tables', s11: 'CUDA graphs for prefill', s12: 'Index the runbooks', s13: 'Rust proxy filters for the gateway', s14: 'WebGL grid shader uniforms', s15: 'Qwen3 throughput on the 4090', s16: 'k6 stress profile for checkout', s17: 'Audio-reactive particles', s18: 'Draft: AI harnesses post' };
  const TITLES = ['Sub-agent Socrates · sanduq-collector', 'Sanduq manual update for consumer fidelity P2', 'Consumer payment success screen 2050', 'Github-wiki-publish documentation', 'Fix the flaky e2e on the Windows runner', 'Release notes 1.7.0', 'Migrate settings to SQLite', 'Review PR #412', 'Arabic RTL on invoices', 'Tenant onboarding wizard', 'Lease renewal reminders', 'Owner statements as PDF', 'Rate limiter for the hub', 'Profile import from the PowerShell launcher', 'Year clock redraw', 'Dockerfile for the Caddy sidecar', 'Backfill token prices', 'Explain the settlement ledger', 'User manual, chapter 4', 'Tag colours by position', 'Repair the tray popup at 150%', 'Port the limits reader to macOS', 'Benchmark Haiku vs Sonnet on the tests', 'Clean up dead CSS', 'Add --json to hrm sessions', 'Pin projects from Sessions', 'Merge duplicate projects', 'Archive sessions older than 30 days', '', 'Sanduq qa_document stage for 2050 payment success', 'Sanduq review for the 2050 payment success screen', 'Teksa consumer app Figma gap analysis', 'HarnessManager theme design variations', 'Multi-agent QA direction implementation', 'Hub client reconnect loop', '', 'Rewrite the README in my voice', 'Skills store: not-managed folders', 'MCP matrix left-alone entries'];
  const TAGS_POOL = ['sanduq', 'release', 'client-x', 'design', 'infra', 'ui'];
  const costRate = (profile) => { const a = D.BASE_ACCOUNTS.concat(D.EXTRA_4_ACCOUNTS).find((x) => x.id === profile); return a && a.tokens7d ? a.cost7d / a.tokens7d : 0; };
  const priced = (s) => s.model !== 'qwen-3-coder' && !['pi', 'omp'].includes(s.harness);
  const costOf = (s) => (priced(s) ? s.tokens * costRate(s.profile) : null);
  const projectOf = (s) => PROJECTS.find((p) => p.key === s.project_key);

  function buildSessions() {
    const r = rng(4242), rows = [], pick = (a) => a[Math.floor(r() * a.length)];
    const acctOf = (id) => D.BASE_ACCOUNTS.find((a) => a.id === id);
    const mk = (p, profile, device, open, started, last, i) => {
      const acct = acctOf(profile), harness = acct.tool, model = pick(MODELS[harness]);
      const messages = open ? 10 + Math.floor(r() * 120) : 1 + Math.floor(r() * 220);
      const cwd = p ? (p.here ? p.root : p.key.replace('github.com/', 'D:\\Work\\')) : '';
      const t = TITLES[Math.floor(r() * TITLES.length)];
      const tags = r() < 0.2 ? [pick(TAGS_POOL)].concat(r() < 0.25 ? [pick(TAGS_POOL)] : []) : [];
      const failed = r() < 0.12 ? 1 + Math.floor(r() * 3) : 0;
      const archived = !open && NOW - last > 30 * DAY && r() < 0.6;
      const remote = device !== HERE;
      return {
        id: `a${i}`, profile, harness, title: t, cwd, messages, prompts: Math.max(1, Math.round(messages / 4)), tool_calls: Math.round(messages * 0.6), failed,
        tokens: Math.round(10 ** (3.7 + r() * 3.6)), model, open, archived, started_at: started, last_active_at: last, tags: [...new Set(tags)],
        source_path: `~\\.${harness}${profile.endsWith('-main') ? '' : '-' + profile}\\projects\\${(p ? p.name : 'no-folder').toLowerCase()}\\${i}.jsonl`,
        project_key: p ? p.key : '', project_name: p ? p.name : '', device, device_name: pcName(device), open_elsewhere: open && remote,
        resumable: remote && !open ? (p && p.here ? true : `not cloned here: ${p ? p.remote || p.name : 'no folder'} is only on ${pcName(device)}`) : true,
        continued_from: !open && r() < 0.04 ? pick(['SAMYMSI', 'Samys-MacBook-Pro']) : ''
      };
    };
    D.LIVE_SESSIONS.forEach((s, i) => {
      const p = PROJECTS.find((x) => x.name === s.project), row = mk(p, s.accountId, s.computer, true, NOW - s.sinceMin * MIN, NOW, i);
      Object.assign(row, { id: s.id, title: LIVE_TITLES[s.id] || s.lastAction, tokens: s.tokensSession, model: s.model, archived: false, failed: s.id === 's2' ? 1 : 0, cwd: p ? p.key.startsWith('github') ? 'D:\\Work\\' + p.name : p.root : '', tags: s.id === 's1' ? ['design'] : s.id === 's2' ? ['sanduq'] : s.id === 's15' ? ['infra', 'release'] : [] });
      rows.push(row);
    });
    const total = WEIGHTS.reduce((a, w) => a + w[1], 0);
    for (let i = 100; i < 420; i++) {
      let x = r() * total, name = '';
      for (const [n, w] of WEIGHTS) { x -= w; if (x <= 0) { name = n; break; } }
      const p = PROJECTS.find((q) => q.name === name) || null;
      const profile = pick(p ? PROJECT_ACCOUNTS[p.name] : ['claude-main', 'codex-main']);
      const acct = acctOf(profile), both = p ? p.devices.filter((d) => acct.computers.includes(d)) : acct.computers;
      const device = both.length ? pick(both) : p ? p.devices[0] : HERE;
      const started = NOW - Math.pow(r(), 1.7) * 60 * DAY - 20 * MIN, last = Math.min(NOW - 8 * MIN, started + (5 + r() * 300) * MIN);
      rows.push(mk(p, profile, device, false, started, last, i));
    }
    return rows;
  }
  const SESSIONS = buildSessions();
  const TAGS = (() => { const m = {}; const add = (t, k) => { m[t] ||= { tag: t, sessions: 0, projects: 0 }; m[t][k]++; }; SESSIONS.forEach((s) => s.tags.forEach((t) => add(t, 'sessions'))); PROJECTS.forEach((p) => p.tags.forEach((t) => add(t, 'projects'))); return TAGS_POOL.filter((t) => m[t]).map((t) => m[t]); })();
  Object.assign(D, { aProjects: PROJECTS, aSessions: SESSIONS, aTags: TAGS, aTranscripts: {} });

  // ---------------------------------------------------------------- transcripts (generated once per session)
  const PROMPTS = ['Read the sessions renderer and tell me why the group fold loses its state on a redraw.', 'Add a --json flag to `hrm sessions` and keep the table output as it is.', 'The settlement worker double-pays when two webhooks land in the same second. Find the race.', 'Run the e2e suite and fix whatever fails on Windows.', 'Write the release notes for 1.7.0 from the commits since 1.6.0.', 'Make the tray popup fit at 150% scaling; it clips the second column.', 'Profile prefill latency and try CUDA graphs.', 'Index the runbooks under docs/ and link them from the wiki.'];
  const ANSWERS = ['The fold set is rebuilt on every draw, so the key changes when the grouping changes. I keep it per grouping now:\n\n- `sFold` keyed by `group:key`\n- the Running-now group keeps its own key\n\nThe test `sessions fold survives redraw` passes.', 'Done. `hrm sessions --json` prints the same rows the page reads, one object per session. The table path is untouched.', 'Found it: `settle()` reads the balance, then writes, with no lock. Two webhooks interleave. I moved the read into the same `UPDATE … RETURNING` and added a test that fires both at once.', 'Three tests failed on the Windows runner; all three used `/` paths. Fixed with `path.join`. Running the full suite again now.', 'Here is the draft:\n\n**1.7.0**\n\n- Pins from Sessions\n- Live limits for Claude\n- Tray popup follows the theme', 'The grid had fixed 170px columns; at 150% that is wider than the popup. Columns are now `minmax(0, 1fr)` and names wrap.'];
  const FILES = ['app/ui/views.js', 'app/ui/core.js', 'app/ui/styles.css', 'src/settle.rs', 'src/worker.rs', 'e2e/app.e2e.mjs', 'docs/release-notes.md', 'app/src-tauri/src/main.rs', 'app/ui/limits.html', 'serve/prefill.py'];
  function transcriptOf(s) {
    if (D.aTranscripts[s.id]) return D.aTranscripts[s.id];
    const r = rng(hash(s.id)), pick = (a) => a[Math.floor(r() * a.length)];
    const p = projectOf(s), root = s.cwd || '';
    const nTurns = s.messages ? Math.max(1, Math.min(12, Math.round(s.messages / 9))) : 0;
    const step = (i, running) => {
      const tool = pick(['Read', 'Edit', 'Bash', 'Grep', 'Read', 'Glob', 'Write']);
      const f = (root ? root.replace(/[\\/]$/, '') + (root.includes('/') ? '/' : '\\') : '') + pick(FILES).replace(/\//g, root.includes('/') || !root ? '/' : '\\');
      const input = tool === 'Bash' ? pick(['npm test -- --grep sessions', 'cargo test --workspace', 'git diff --stat', 'node e2e/app.e2e.mjs', 'python -m pytest -q']) : tool === 'Grep' ? pick(['renderRows', 'sFold', 'settle(']) : tool === 'Glob' ? '**/*.css' : f;
      const st = { kind: 'tool', tool, input, status: running ? 'running' : 'done', duration_ms: running ? null : Math.round(80 + r() * 2400), result: tool === 'Bash' ? '> 42 passing (3.1s)\n> 0 failing' : tool === 'Read' ? `${Math.round(40 + r() * 400)} lines read` : tool === 'Grep' ? '3 matches in 2 files' : tool === 'Glob' ? '12 files' : '' };
      if (!running && ['Edit', 'Write'].includes(tool)) { const a = 1 + Math.floor(r() * 30), d = Math.floor(r() * 12); st.change = { path: f, added: a, removed: d, diff: `@@ -${12 + i},${3 + d} +${12 + i},${3 + a} @@\n   const key = group + ':' + g.key;\n-  const open = !sFold.has(g.key);\n+  const open = !sFold.has(key);\n+  // folds are remembered per grouping\n   return open;` }; }
      return st;
    };
    const turns = []; let t0 = s.started_at, left = s.failed;
    for (let i = 0; i < nTurns; i++) {
      const live = s.open && i === nTurns - 1;
      const n = s.id === 's9' && i === 1 ? 2000 : 2 + Math.floor(r() * 8);
      const steps = Array.from({ length: n }, (_, j) => step(j, live && j === n - 1));
      if (s.id === 's9' && i === 1) steps.forEach((st, j) => { st.tool = 'Bash'; st.input = `npx playwright test shots/${String(j + 1).padStart(4, '0')}.spec.ts`; st.result = '✓ 1 passed (1.2s)'; delete st.change; });
      for (let j = 0; left > 0 && j < steps.length; j++) if (steps[j].tool === 'Bash' && steps[j].status === 'done') { steps[j].status = 'error'; steps[j].error = 'Error: ENOENT: no such file or directory, open \'e2e/fixtures/missing.json\'\n    at Object.openSync (node:fs:581:3)'; delete steps[j].change; left--; }
      const dur = steps.reduce((a, st) => a + (st.duration_ms || 0), 0) + 20e3;
      const answers = live ? [] : [ANSWERS[(hash(s.id) + i) % ANSWERS.length]];
      if (!live && r() < 0.35) answers.unshift('Looking at it now.');
      turns.push({ prompt: PROMPTS[(hash(s.id) + i) % PROMPTS.length], prompt_at: t0, answer: answers, answer_pos: answers.length === 2 ? [1, n] : [n], answer_at: t0 + dur, model: s.model, steps });
      t0 += dur + (2 + r() * 20) * MIN;
    }
    return (D.aTranscripts[s.id] = { meta: s, turns });
  }
  const statsOf = (s) => ({ cost: costOf(s), requests: Math.round(s.messages * 0.8) + 1, hit: 0.55 + (hash(s.id) % 40) / 100, p50: (D.BASE_ACCOUNTS.find((a) => a.id === s.profile)?.firstReplySec || 1.0) * 1000 });
  const LIVE_STEPS = [['Edit', 'app/ui/views.js'], ['Bash', 'npm test -- --grep sessions'], ['Read', 'app/ui/core.js'], ['Grep', 'drawPins']];
  function appendLive(s, tick) {
    const t = transcriptOf(s), turn = t.turns[t.turns.length - 1]; if (!turn) return;
    const running = turn.steps.find((st) => st.status === 'running');
    if (running) { running.status = 'done'; running.duration_ms = 900 + (tick % 5) * 420; if (running.tool === 'Edit') running.change = { path: running.input, added: 4, removed: 1, diff: '@@ -210,4 +210,7 @@\n   const rows = [];\n-  rows.push(row);\n+  rows.push(row);\n+  if (s.open) running.push(row);\n+  // live rows stay at the top' }; }
    if (tick % 4 === 3) { turn.answer.push(['Tests pass; moving to the next file.', 'The lane renderer is patched; running the suite again.', 'Two rows changed; nothing else moved.'][tick % 3]); turn.answer_pos.push(turn.steps.length); turn.answer_at = NOW; }
    const [tool, input] = LIVE_STEPS[tick % LIVE_STEPS.length];
    turn.steps.push({ kind: 'tool', tool, input: (s.cwd ? s.cwd + '\\' : '') + input, status: 'running', duration_ms: null, result: tool === 'Bash' ? '> 42 passing (3.1s)' : '' });
    s.messages += 1; s.tokens += 11800 + (tick % 7) * 900; s.tool_calls += 1;
  }

  // ---------------------------------------------------------------- shared helpers
  const A = (state) => (state.a ||= { sessions: { group: 'project', days: '7', live: false, project: '', tag: '', pc: '', profile: '', page: 0, folds: {}, more: {}, railMore: false }, session: { id: null, mode: 'all', open: {}, more: {}, follow: false, ticks: 0 }, projects: { days: '7', sort: 'active', metric: 'tokens', per: 10, page: 0, tiles: {} }, dialog: null, menu: null, rename: null, terminal: 'wt' });
  const sameDay = (a, b) => new Date(a).toDateString() === new Date(b).toDateString();
  const hm = (ts) => { const d = new Date(ts); return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`; };
  const when = (ts) => sameDay(ts, NOW) ? `Today ${hm(ts)}` : sameDay(ts, NOW - DAY) ? `Yesterday ${hm(ts)}` : new Date(ts).toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
  const dayLabel = (ts) => sameDay(ts, NOW) ? 'Today' : sameDay(ts, NOW - DAY) ? 'Yesterday' : new Date(ts).toLocaleDateString('en-US', { weekday: 'short', month: 'short', day: 'numeric' });
  const money = (c) => (c == null ? '—' : D.fmtMoney(c));
  const secs = (ms) => (ms >= 1000 ? (ms / 1000).toFixed(1) + ' s' : ms + ' ms');
  const tagColor = (t) => 'c' + (Math.max(0, TAGS.findIndex((x) => x.tag === t)) % 6);
  const btn = (act, id, cls, label, extra = '') => `<button class="${cls}" data-a="${act}" data-id="${esc(id ?? '')}" data-key="${esc(act + ':' + (id ?? ''))}" ${extra}>${label}</button>`;
  const toolPill = (h) => `<span class="a-toolpill" title="${esc(TOOL[h] || h)}">${D.LOGOS[h] || ''}<span>${esc(TOOL[h] || h)}</span></span>`;
  const tagsHtml = (tags, kind, key, name) => `<span class="a-tags">${btn('tag-edit', key, 'a-tagbtn', `${ICON.tag}${tags.length ? `<small>${tags.length}</small>` : ''}`, `data-kind="${kind}" aria-label="Tags of ${esc(name)}" title="${tags.length ? esc(tags.join(', ')) : 'Add a tag'}"`)}${tags.slice(0, 2).map((t) => btn('s-tag', t, `a-tag ${tagColor(t)}`, esc(t), `title="Sessions tagged ${esc(t)}"`)).join('')}${tags.length > 2 ? `<span class="a-tag" title="${esc(tags.slice(2).join(', '))}">+${tags.length - 2}</span>` : ''}</span>`;
  const sessionsOf = (state) => (state.metrics.isFresh ? [] : SESSIONS.filter((s) => state.accounts.some((a) => a.id === s.profile)));
  const inDays = (days) => (s) => days === 'all' || s.last_active_at >= NOW - Number(days) * DAY;
  const syncOn = (state) => state.metrics.computersList.length > 1;
  const area = (series) => {
    const w = 300, h = 44, m = Math.max(1, ...series), n = Math.max(2, series.length);
    const pts = series.map((v, i) => [i * (w / (n - 1)), h - 4 - (v / m) * (h - 8)]);
    const d = pts.map((q, i) => (i ? 'L' : 'M') + q[0].toFixed(1) + ' ' + q[1].toFixed(1)).join(' ');
    return `<svg class="a-area" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" aria-hidden="true"><path d="${d} L${pts[pts.length - 1][0].toFixed(1)} ${h} L0 ${h}Z" fill="var(--teal-dim)"/><path d="${d}" fill="none" stroke="var(--teal)" stroke-width="1.5" vector-effect="non-scaling-stroke"/><circle cx="${pts[pts.length - 1][0].toFixed(1)}" cy="${pts[pts.length - 1][1].toFixed(1)}" r="2.5" fill="var(--teal)"/></svg>`;
  };
  function projectRows(state, days) {
    const list = sessionsOf(state).filter(inDays(days)), n = days === 'all' ? 60 : Number(days);
    return PROJECTS.map((p) => {
      const ss = list.filter((s) => s.project_key === p.key); if (!ss.length) return null;
      const tokens = ss.reduce((a, s) => a + s.tokens, 0), pricedTok = ss.filter(priced).reduce((a, s) => a + s.tokens, 0);
      const cost = pricedTok ? ss.reduce((a, s) => a + (costOf(s) || 0), 0) : null;
      const series = Array.from({ length: n }, (_, i) => ss.filter((s) => Math.floor((NOW - s.last_active_at) / DAY) === n - 1 - i).reduce((a, s) => a + s.tokens, 0));
      const devices = [...new Set(ss.map((s) => s.device))].map((id) => ({ id, name: pcName(id), sessions: ss.filter((s) => s.device === id).length }));
      return { ...p, sessions: ss.length, tokens, cost, live: ss.filter((s) => s.open).length, accounts: [...new Set(ss.map((s) => s.profile))], series, last_active_at: Math.max(...ss.map((s) => s.last_active_at)), priced_share: tokens ? pricedTok / tokens : 1, devices };
    }).filter(Boolean);
  }
  const SH = ['l1', 'l2', 'l3', 'l4', 'l5'];
  const startable = (state) => state.accounts.filter((a) => !a.needsLogin && D.TOOLS.find((t) => t.id === a.tool)?.installed && (a.kind !== 'subscription' || a.measured || a.tool !== 'agy'));
  const flagOf = (tool) => ({ claude: '--dangerously-skip-permissions', codex: '--dangerously-bypass-approvals-and-sandbox' })[tool] || '';

  // ---------------------------------------------------------------- Sessions (the list)
  function filterSessions(state) {
    const f = A(state).sessions, q = (state.searchQuery || '').trim().toLowerCase();
    return sessionsOf(state).filter((s) => (f.days === 'all' || s.last_active_at >= NOW - Number(f.days) * DAY || s.open) && (!f.live || s.open) && (!f.project || s.project_key === f.project) && (!f.tag || s.tags.includes(f.tag) || (projectOf(s)?.tags || []).includes(f.tag)) && (!f.pc || (f.pc === 'here' ? s.device === HERE : s.device === f.pc)) && (!f.profile || s.profile === f.profile) && (!q || `${s.title} ${s.cwd} ${s.profile} ${TOOL[s.harness]} ${s.device_name} ${s.project_name}`.toLowerCase().includes(q)))
      .sort((a, b) => (b.open - a.open) || (b.last_active_at - a.last_active_at));
  }
  function groupKey(s, group) { return group === 'project' ? s.project_key : group === 'date' ? new Date(s.last_active_at).toDateString() : group === 'account' ? s.profile : group === 'tool' ? s.harness : (s.tags[0] || ''); }
  function groupName(key, group, sample) { return group === 'project' ? (sample.project_name || 'No folder') : group === 'date' ? dayLabel(sample.last_active_at) : group === 'account' ? key : group === 'tool' ? TOOL[key] : (key || 'untagged'); }
  function sessionRow(s, state) {
    const sync = syncOn(state), remote = s.device !== HERE;
    const where = [s.continued_from ? `continued from ${s.continued_from}` : '', plural(s.messages, 'msg'), s.cwd ? s.cwd.split(/[\\/]/).slice(-2).join('\\') : ''].filter(Boolean).join(' · ');
    const whenTxt = s.open ? (s.open_elsewhere ? `running on ${s.device_name}` : 'live') : when(s.last_active_at);
    const action = s.archived ? '' : s.open ? `<button class="btn-follow" data-action="follow-session" data-session="${esc(s.id)}">Follow</button>` : !remote ? btn('resume', s.id, 'btn-lane-start', 'Resume', 'style="opacity:1;pointer-events:auto"') : s.resumable === true ? btn('resume-here', s.id, 'btn-lane-start', 'Resume here', 'style="opacity:1;pointer-events:auto"') : `<span class="a-reason" title="${esc(s.resumable)}">${esc(String(s.resumable).split(':')[0])}</span>`;
    return `<div class="a-srow" data-key="row:${esc(s.id)}"><i class="${s.open ? 'live-pulse-dot' : 'a-dot ' + (s.failed ? 'bad' : '')}" ${s.failed ? `title="${plural(s.failed, 'failed command')}"` : 'aria-hidden="true"'}></i>
      <div class="a-tcell">${btn('open-session', s.id, 'a-title', `<b>${esc(s.title || 'Untitled session')}${s.archived ? '<span class="a-pill arch">archived</span>' : ''}</b>`, `title="${esc(s.cwd || 'No folder')}"`)}<div class="a-tline"><span class="a-where mono" title="${esc(s.cwd)}">${esc(where)}</span>${tagsHtml(s.tags, 'session', s.id, s.title || 'this session')}</div></div>
      <span class="a-acct" title="${esc(s.profile)}">${esc(s.profile)}</span>${toolPill(s.harness)}${sync ? `<span class="a-pc" title="Ran on ${esc(s.device_name)}"><i class="pc-dot" style="background:${PC(s.device)?.col || 'var(--line-strong)'}"></i><span>${esc(s.device_name)}</span></span>` : '<span></span>'}
      <span class="num">${D.fmtTokens(s.tokens)}</span><span class="a-when ${s.open ? 'live' : ''}">${esc(whenTxt)}</span><span class="a-act">${action}</span></div>`;
  }
  function renderSessions(state) {
    const S = A(state), f = S.sessions, all = sessionsOf(state), sync = syncOn(state);
    if (state.projectId) { f.project = PROJECTS.some((p) => p.key === state.projectId) ? state.projectId : (PROJECTS.find((p) => p.name === state.projectId)?.key || ''); f.page = 0; state.projectId = null; }
    if (state.pageTab === 'live') { f.live = true; state.pageTab = null; }
    if (!all.length) return `<div class="page-body" data-tour="sessions"><section class="panel-card empty-state"><h1>No sessions yet.</h1><span class="muted">Sessions appear here as your tools write their logs.</span>${state.accounts.length ? btn('new-session', '', 'btn-primary', 'New session') : '<button class="btn-primary" data-action="open-add-account">Add account</button>'}</section>${renderDialog(state)}</div>`;
    const rows = filterSessions(state), total = rows.length, running = rows.filter((s) => s.open), q = (state.searchQuery || '').trim();
    const group = f.project && f.group === 'project' ? 'date' : f.group;
    const plist = projectRows(state, f.days), cur = f.project ? (plist.find((p) => p.key === f.project) || PROJECTS.find((p) => p.key === f.project)) : null;
    const pageRows = rows.slice(f.page * PER, f.page * PER + PER);
    const groups = [];
    for (const s of rows) { const key = groupKey(s, group); let g = groups.find((x) => x.key === key); if (!g) groups.push((g = { key, name: groupName(key, group, s), sessions: [], total: 0, tokens: 0, root: s.cwd && group === 'project' ? (projectOf(s)?.root || '') : '', remote: group === 'project' ? projectOf(s)?.remote || '' : '' })); g.total++; g.tokens += s.tokens; if (f.more[group + ':' + key] || pageRows.includes(s)) g.sessions.push(s); }
    const shown = f.live ? groups : groups.map((g) => ({ ...g, sessions: g.sessions.filter((s) => !s.open), total: g.total - g.sessions.filter((s) => s.open).length })).filter((g) => g.total);
    const grp = (g, k, cls) => { const open = !f.folds[k], p = g.key && group === 'project' ? plist.find((x) => x.key === g.key) : null; return `<section class="a-grp ${cls || ''} ${open ? '' : 'shut'}" data-key="grp:${esc(k)}"><div class="a-gh"><h2 class="a-gname">${group === 'tag' && g.key && !cls ? `<span class="a-tag ${tagColor(g.key)}">${esc(g.name)}</span>` : esc(g.name)}</h2>${g.root ? `<span class="a-path mono" title="${esc(g.root)}">${esc(g.root)}</span>` : g.remote ? `<span class="a-path">${esc(g.remote)}</span>` : ''}${group === 'project' && g.key && !cls ? menuWrap(state, 'project', g.key, btn('project-menu', g.key, 'a-ib', '⋯', `aria-label="Open, pin, rename or merge ${esc(g.name)}" aria-haspopup="menu"`)) : ''}<span class="a-tot">${plural(g.total, 'session')} · ${D.fmtTokens(g.tokens)} tokens${p && p.cost != null ? ' · ' + money(p.cost) : ''}</span>${group === 'project' && p && p.root && !cls ? btn('new-session', p.key, 'btn-lane-start', 'New session', 'style="opacity:1;pointer-events:auto"') : ''}${btn('s-fold', k, 'a-fold', open ? '▾' : '▸', `aria-expanded="${open}" aria-label="${open ? 'Fold' : 'Open'} ${esc(g.name)}"`)}</div>${open ? g.sessions.map((s) => sessionRow(s, state)).join('') + (g.sessions.length < g.total ? `<div class="a-more-row">${btn('s-more', group + ':' + g.key, 'filter-chip', `Show all ${plural(g.total, 'session')}`)}</div>` : '') : ''}</section>`; };
    const keep = { group: f.group, days: f.days };
    const item = (p) => `<div class="a-rail-row">${btn('rail-project', p.key, `a-rail-item ${f.project === p.key ? 'on' : ''}`, `<i class="a-dot ${p.live ? 'live' : ''}"></i><span>${esc(p.name)}</span><span class="a-cnt ${p.live ? 'live' : ''}">${p.live ? p.live + ' live' : p.sessions}</span>`)}${btn('pin', p.key, `a-ib ${p.pinned ? 'on' : ''}`, ICON.pin, `aria-label="${p.pinned ? 'Unpin' : 'Pin'} ${esc(p.name)}" title="${p.pinned ? 'Pinned to the sidebar: click to unpin' : 'Pin to the sidebar'}"`)}</div>`;
    const pinned = plist.filter((p) => p.pinned), rest = plist.filter((p) => !p.pinned);
    const rail = `<aside class="a-rail" aria-label="Projects, pinned and recent" data-tour="rail">
      ${btn('rail-live', '', `a-rail-item ${f.live ? 'on' : ''}`, `<i class="a-dot live"></i><span>Live now</span><span class="a-cnt live">${running.length || ''}</span>`)}
      ${btn('rail-all', '', `a-rail-item ${!f.live && !f.project ? 'on' : ''}`, `<i class="a-dot"></i><span>All projects</span><span class="a-cnt">${plist.reduce((a, p) => a + p.sessions, 0)}</span>`)}
      ${pinned.length ? `<div class="a-rail-sec">Pinned</div>${pinned.map(item).join('')}` : ''}
      <div class="a-rail-sec">${pinned.length ? 'Recent' : 'Projects'}</div>${(f.railMore ? rest : rest.slice(0, 8)).map(item).join('')}${rest.length > 8 && !f.railMore ? btn('rail-more', '', 'a-rail-link muted', `${rest.length - 8} more…`) : ''}
      ${btn('go-projects', '', 'a-rail-link', 'Every project, with costs →')}
      ${TAGS.length ? `<div class="a-rail-sec" data-tour="tags">Tags</div><div class="a-rail-tags">${TAGS.map((t) => btn('rail-tag', t.tag, `a-rail-item ${f.tag === t.tag ? 'on' : ''}`, `<i class="a-dot" style="visibility:hidden"></i><span><span class="a-tag ${tagColor(t.tag)}">${esc(t.tag)}</span></span><span class="a-cnt">${t.sessions + t.projects}</span>`)).join('')}</div>` : ''}</aside>`;
    const nameH1 = (p) => S.rename && S.rename.key === p.key ? `<input id="a-rename-input" class="a-rename" value="${esc(S.rename.draft)}" aria-label="Project name" maxlength="80" oninput="window.HM.aInput('rename',this.value)"/>` : `<span class="a-pname" data-a-name="${esc(p.key)}" title="Double-click to rename">${esc(p.name)}</span>`;
    const head = cur
      ? `${btn('back-projects', '', 'filter-chip', '← Projects', 'style="align-self:flex-start"')}<header class="page-heading"><div><h1 class="a-h1row">${nameH1(cur)}${btn('rename', cur.key, 'a-ib', ICON.edit, `aria-label="Rename ${esc(cur.name)}" title="Rename (or double-click the name)"`)}${btn('pin', cur.key, `a-ib ${cur.pinned ? 'on' : ''}`, ICON.pin, `aria-label="${cur.pinned ? 'Unpin' : 'Pin'} ${esc(cur.name)}" title="${cur.pinned ? 'Pinned to the sidebar: click to unpin' : 'Pin to the sidebar'}"`)}${tagsHtml(cur.tags, 'project', cur.key, cur.name)}</h1><div class="heading-facts mono a-path">${esc(cur.remote || cur.root || '')}</div><div class="a-pstats"><span><b>${money(cur.cost)}</b> ≈ list prices</span><span><b>${D.fmtTokens(cur.tokens || 0)}</b> tokens</span><span><b>${cur.sessions || 0}</b> ${cur.sessions === 1 ? 'session' : 'sessions'}</span><span><b>${(cur.accounts || []).length}</b> ${(cur.accounts || []).length === 1 ? 'account' : 'accounts'}</span>${cur.live ? `<span class="a-live"><b>${cur.live}</b> running</span>` : ''}</div></div><div class="page-actions">${menuWrap(state, 'project', cur.key, btn('project-menu', cur.key, 'icon-btn', '⋯', `aria-label="Pin, rename or merge" aria-haspopup="menu"`))}${cur.root ? btn('new-session', cur.key, 'btn-primary', 'New session') : ''}</div></header>`
      : `<header class="page-heading"><div><h1>${running.length ? `<span class="a-warn">${plural(running.length, 'session is', 'sessions are')}</span> running now; open one to follow it live.` : `Every tool's sessions in one place${[...new Set(rows.map((s) => s.harness))].length ? ', from ' + esc([...new Set(rows.map((s) => TOOL[s.harness]))].join(', ').replace(/, ([^,]*)$/, ' and $1')) : ''}.`}</h1><div class="heading-facts">${plural(total, 'session')}${f.days !== 'all' ? ' · ' + f.days + ' days' : ''} · ${plural(groups.length, group === 'project' ? 'project' : 'group')}${sync ? ' · ' + plural(state.metrics.computersList.length, 'computer') : ''} · tokens from the logs · cost ≈ list prices</div></div><div class="page-actions">${btn('new-session', '', 'btn-primary', 'New session')}</div></header>`;
    const folders = cur ? [...new Set(rows.map((s) => s.cwd).filter(Boolean))] : [], accts = cur ? (cur.accounts || []) : [], ptags = cur ? [...new Set([...(cur.tags || []), ...rows.flatMap((s) => s.tags)])] : [];
    const about = cur && (folders.length > 1 || accts.length > 1) ? `<aside class="a-rail a-about" aria-label="This project"><div class="a-rail-sec">This project</div><div class="a-kv"><b>${money(cur.cost)}</b><span>≈ list prices</span><b>${D.fmtTokens(cur.tokens)}</b><span>tokens</span><b>${cur.sessions}</b><span>${cur.sessions === 1 ? 'session' : 'sessions'}</span></div>${cur.series && cur.series.some((v) => v) ? `<div class="a-rail-sec">${f.days === 'all' ? 'Activity' : f.days + ' days'}</div>${area(cur.series)}` : ''}${running.length ? `<div class="a-rail-sec">Running</div>${running.map((s) => btn('open-session', s.id, 'a-rail-item', `<i class="a-dot live"></i><span>${esc(s.title || 'Untitled session')}</span><span></span>`)).join('')}` : ''}${accts.length > 1 ? `<div class="a-rail-sec">Accounts</div>${accts.map((a) => { const n = rows.filter((s) => s.profile === a).length; return `<div class="a-arow"><span>${esc(a)}</span><span>${n}${rows.length ? ' · ' + Math.round((n / rows.length) * 100) + '%' : ''}</span></div>`; }).join('')}` : ''}${folders.length > 1 ? `<div class="a-rail-sec">Folders</div>${folders.slice(0, 6).map((x) => `<div class="a-afold mono" title="${esc(x)}">${esc(x)}</div>`).join('')}${folders.length > 6 ? `<div class="a-afold">${folders.length - 6} more</div>` : ''}` : ''}</aside>` : '';
    const seg = (k, opts, cur, label) => `${label ? `<span class="a-lbl">${label}</span>` : ''}<div class="chip-group" role="group" aria-label="${label || k}">${opts.map(([v, l]) => btn('s-set', `${k}=${v}`, `filter-chip ${cur === v ? 'active' : ''}`, l, `data-k="${k}" data-v="${esc(v)}" aria-pressed="${cur === v}"`)).join('')}</div>`;
    const fbar = `<div class="horizon-filterbar a-fbar" data-tour="filters">${f.live ? btn('s-set', 'live=', 'a-chipx', 'Running now ✕', 'data-k="live" data-v=""') : ''}${f.tag && !cur ? btn('s-set', 'tag=', 'a-chipx', `<span class="a-tag ${tagColor(f.tag)}">${esc(f.tag)}</span> ✕`, 'data-k="tag" data-v=""') : ''}${q ? btn('s-set', 'q=', 'a-chipx', `“${esc(q)}” ✕`, 'data-k="q" data-v=""') : ''}
      ${seg('group', [['project', 'Project'], ['date', 'Date'], ['account', 'Account'], ['tool', 'Tool'], ['tag', 'Tag']].filter(([k]) => !(cur && k === 'project')), group, 'Group by')}${seg('days', [['7', '7d'], ['30', '30d'], ['all', 'All']], f.days)}
      ${sync ? seg('pc', [['', 'All computers'], ['here', 'This computer'], ...state.metrics.computersList.filter((c) => c.id !== HERE).map((c) => [c.id, c.name])], f.pc, '') : ''}
      ${cur && accts.length > 1 ? seg('profile', [['', 'All'], ...accts.map((a) => [a, a])], f.profile, 'Accounts') : ''}${cur && ptags.length ? seg('tag', [['', 'All'], ...ptags.map((t) => [t, t])], f.tag, 'Tags') : ''}<span class="a-hint">Search above searches every session</span></div>`;
    const body = `${running.length && !f.live ? grp({ key: '', name: 'Running now', sessions: running.filter((s) => pageRows.includes(s) || f.more['live']), total: running.length, tokens: running.reduce((a, s) => a + s.tokens, 0) }, 'live', 'livegrp') : ''}
      ${shown.length ? shown.map((g) => grp(g, group + ':' + g.key)).join('') : running.length && !f.live ? '' : `<section class="a-grp"><div class="a-empty">No sessions match.${btn('s-clear', '', 'filter-chip', 'Clear filters')}</div></section>`}
      ${total > PER ? `<div class="a-pager">${btn('s-set', 'page=' + (f.page - 1), 'filter-chip', '‹', `data-k="page" data-v="${f.page - 1}" ${f.page === 0 ? 'disabled' : ''} aria-label="Previous page"`)}<span>${f.page * PER + 1}–${Math.min(total, (f.page + 1) * PER)} of ${total}</span>${btn('s-set', 'page=' + (f.page + 1), 'filter-chip', '›', `data-k="page" data-v="${f.page + 1}" ${(f.page + 1) * PER >= total ? 'disabled' : ''} aria-label="Next page"`)}</div>` : ''}`;
    return `<div class="page-body" data-tour="sessions">${head}<div class="a-wrap" ${cur && !about ? 'style="grid-template-columns:minmax(0,1fr)"' : ''}>${cur ? about : rail}<div class="a-smain" data-tour="groups">${fbar}${body}</div></div>${renderDialog(state)}</div>`;
  }

  // ---------------------------------------------------------------- Session (one session, followed live)
  const inline = (s) => esc(s).replace(/`([^`]+)`/g, '<code class="mono">$1</code>').replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>');
  const md = (src) => String(src).split(/\n{2,}/).map((b) => { const lines = b.split('\n'); if (lines.every((l) => /^[-*] /.test(l))) return `<ul>${lines.map((l) => `<li>${inline(l.slice(2))}</li>`).join('')}</ul>`; if (/^```/.test(b)) return `<pre class="mono">${esc(b.replace(/^```\w*\n?|```$/g, ''))}</pre>`; return `<p>${inline(b)}</p>`; }).join('');
  const diffHtml = (d) => `<div class="a-diff mono">${d.split('\n').map((l) => `<span class="${l[0] === '+' ? 'a-add' : l[0] === '-' ? 'a-del' : l.startsWith('@@') ? 'a-hunk' : ''}">${esc(l)}</span>`).join('')}</div>`;
  function renderSession(state) {
    const S = A(state), rv = S.session, s = sessionsOf(state).find((x) => x.id === state.sessionId);
    if (!s) return `<div class="page-body"><section class="panel-card empty-state"><h1>Session unavailable.</h1><span class="muted">It is not in this preset, or its log was not read.</span>${btn('back-sessions', '', 'btn-secondary', '← Sessions')}</section></div>`;
    if (rv.id !== s.id) Object.assign(rv, { id: s.id, mode: 'all', open: {}, more: {}, follow: s.open, ticks: 0, needScroll: true });
    const t = transcriptOf(s), ss = statsOf(s), remote = s.device !== HERE;
    const steps = t.turns.flatMap((x) => x.steps).filter((st) => st.kind !== 'thinking');
    const changes = steps.filter((st) => st.change), failed = steps.filter((st) => st.status === 'error'), files = new Set(changes.map((st) => st.change.path)).size;
    const project = s.project_name || (s.cwd || '').split(/[\\/]/).filter(Boolean).pop() || 'no folder';
    const row = (st, k) => { const open = rv.open[k], bad = st.status === 'error', busy = st.status === 'running' || st.status === 'pending'; const meta = st.change ? `+${st.change.added} −${st.change.removed}` : bad ? `failed${st.duration_ms ? ' · ' + secs(st.duration_ms) : ''}` : st.duration_ms ? secs(st.duration_ms) : busy ? 'running now' : ''; return `<div class="a-step ${open ? 'open' : ''}" data-key="st:${k}">${btn('step-open', k, 'a-sb', `<span class="${bad ? 'a-bad' : busy ? 'a-busy' : 'a-ok'}">${bad ? '✗' : busy ? '…' : '✓'}</span><span class="a-k">${esc(st.tool)}</span><span class="a-what mono" title="${esc(st.input)}">${esc(st.input)}</span><span class="a-smeta ${bad ? 'a-bad' : ''}">${esc(meta)}</span>`, `aria-expanded="${!!open}"`)}${open ? `<div class="a-sdet">${st.change ? diffHtml(st.change.diff) : `<pre class="a-sout mono">${esc(st.error || st.result || 'No output.')}</pre>`}</div>` : ''}</div>`; };
    const run = (list, id, head, tail) => !list.length ? '' : `<div class="a-steps" data-key="run:${id}">${rv.more[id] || list.length <= head + tail + 2 ? list.map(([st, k]) => row(st, k)).join('') : list.slice(0, head).map(([st, k]) => row(st, k)).join('') + `<div class="a-more" data-key="more:${id}">${btn('more-steps', id, 'filter-chip active', `Show ${(list.length - head - tail).toLocaleString('en-US')} more steps`)}</div>` + list.slice(-tail).map(([st, k]) => row(st, k)).join('')}</div>`;
    const who = (x) => `<span class="a-who">${esc(TOOL[s.harness].toUpperCase())}${x.answer_at ? ' · ' + hm(x.answer_at) : ''}${x.model ? ' · ' + esc(x.model) : ''}</span>`;
    let k = 0;
    const chatAll = t.turns.map((x, ti) => {
      let out = x.prompt ? `<div class="a-msg you" data-key="you:${ti}"><span class="a-who">YOU${x.prompt_at ? ' · ' + hm(x.prompt_at) : ''}</span><div class="a-tx">${esc(x.prompt)}</div></div>` : '';
      const at = x.steps.map((st) => (st.kind === 'thinking' ? -1 : k++)), pos = x.answer.map((_, i) => (x.answer_pos && x.answer_pos[i] != null ? x.answer_pos[i] : x.steps.length));
      let from = 0;
      const flush = (to) => { const l = x.steps.slice(from, to).map((st, i) => [st, at[from + i]]).filter(([, j]) => j >= 0); out += run(l, ti + ':' + from, 5, 5); from = to; };
      const groups = []; x.answer.forEach((a, i) => { if (!groups.length || groups[groups.length - 1].p !== pos[i]) groups.push({ p: pos[i], a: [] }); groups[groups.length - 1].a.push(a); });
      groups.forEach((g, gi) => { flush(Math.max(from, g.p)); out += `<div class="a-msg ai" data-key="ai:${ti}:${gi}">${who(x)}<div class="a-tx">${g.a.map(md).join('')}</div></div>`; });
      flush(x.steps.length);
      return out;
    }).join('');
    const pickedList = steps.map((st, j) => [st, j]).filter(([st]) => (rv.mode === 'changes' ? st.change : st.status === 'error'));
    const body = rv.mode === 'all' ? chatAll : run(pickedList, 'flat', 30, 10) || `<div class="a-empty">${rv.mode === 'changes' ? 'This session changed no files.' : 'Nothing failed.'}</div>`;
    const term = S.terminal === 'console' ? 'a console window' : 'Windows Terminal';
    const stat = (title, value, note, cls = '') => `<div class="account-stat"><span>${title}</span><b class="${cls}">${value}</b><span>${note || ''}</span></div>`;
    const actions = remote ? (s.resumable === true && !s.open ? btn('resume-here', s.id, 'btn-primary', 'Resume here') : s.open ? btn('resume-here', s.id, 'btn-primary', 'Resume here') : `<span class="a-reason muted" title="${esc(s.resumable)}">${esc(String(s.resumable).split(':')[0])}</span>`)
      : s.archived ? '<span class="muted">archived: it can be read, not resumed</span>'
      : `<span class="a-split">${btn('resume', s.id, 'btn-secondary', `Resume in ${term}`)}${menuWrap(state, 'terminal', s.id, btn('terminal-menu', s.id, 'btn-secondary', '▾', 'aria-label="Choose terminal" aria-haspopup="menu"'))}</span>`;
    return `<div class="page-body a-sbody" data-tour="session">${btn('back-sessions', '', 'filter-chip', `← Sessions · ${esc(project)}`, 'style="align-self:flex-start"')}
      <header class="page-heading"><div><h1>${esc(s.title || 'Untitled session')}</h1><div class="a-meta">${s.open ? `<span class="a-pill live"><i class="live-pulse-dot"></i>LIVE${rv.follow ? ' · following' : ''}</span>` : ''}${s.archived ? '<span class="a-pill arch">archived</span>' : ''}<span>${esc(s.profile)} · ${esc(TOOL[s.harness])}</span>${tagsHtml(s.tags, 'session', s.id, s.title || 'this session')}${s.model ? `<span class="mono">${esc(s.model)}</span>` : ''}${remote ? `<span><i class="pc-dot" style="background:${PC(s.device)?.col}"></i> ran on ${esc(s.device_name)}</span>` : s.continued_from ? `<span>continued from ${esc(s.continued_from)}</span>` : ''}<span>started ${esc(when(s.started_at))} · ${plural(t.turns.length, 'turn')}</span><span>${D.fmtTokens(s.tokens)} tokens${ss.cost != null ? ' · ' + money(ss.cost) + ' ≈' : ''}</span></div></div>
      <div class="page-actions" data-tour="resume">${remote ? '' : btn('export-md', s.id, 'btn-secondary', 'Export as Markdown')}${actions}${menuWrap(state, 'session', s.id, btn('session-menu', s.id, 'icon-btn', '⋯', 'aria-label="More" aria-haspopup="menu"'))}</div></header>
      <section class="account-stats" aria-label="Session readouts">${stat('Tokens', D.fmtTokens(s.tokens))}${stat('Read from cache', ss.requests ? Math.round(ss.hit * 100) + '%' : '—')}${stat('First reply p50', ss.p50 != null ? (ss.p50 / 1000).toFixed(1) + ' s' : '—')}${stat('Failed commands', String(failed.length), '', failed.length ? 'a-neg' : '')}${stat('Files changed', String(files))}</section>
      <section class="panel-card a-conv" data-tour="conversation"><div class="panel-head"><h2 class="panel-title">Conversation<span class="muted">${plural(t.turns.length, 'turn')} · ${steps.length.toLocaleString('en-US')} steps · ${files} changed ${files === 1 ? 'file' : 'files'} · ${failed.length} failed</span></h2><div class="chip-group" role="group" aria-label="Show" data-tour="modes">${[['all', 'All'], ['changes', 'Changes'], ['failed', 'Failed']].map(([v, l]) => btn('rv-mode', v, `filter-chip ${rv.mode === v ? 'active' : ''}`, l, `aria-pressed="${rv.mode === v}"`)).join('')}</div></div>
      <div class="a-chat" id="a-chat" tabindex="0" aria-label="Conversation; scroll to read">${body || '<div class="a-empty">This session has no messages yet.</div>'}<div class="a-toend" id="a-toend" ${rv.follow || !s.open && rv.atEnd !== false ? 'hidden' : ''}>${btn('follow-end', '', 'btn-primary', `↓ Latest${s.open ? ' · follow live' : ''}`)}</div></div></section>${renderDialog(state)}</div>`;
  }

  // ---------------------------------------------------------------- Projects
  function renderProjects(state) {
    const S = A(state), pj = S.projects;
    if (state.projectId) return `<div class="page-body"><h1 class="muted" style="font-size:18px">Opening ${esc(state.projectId)}…</h1></div>`;
    const list = projectRows(state, pj.days);
    const rangeTxt = pj.days === 'all' ? 'all time' : 'the last ' + pj.days + ' days';
    const liveN = list.filter((p) => p.live).length;
    const head = `<header class="page-heading"><div><h1>${list.length ? `${plural(list.length, 'project')} ran in ${rangeTxt}${liveN ? `; <span class="a-warn">${liveN}</span> ${liveN === 1 ? 'has' : 'have'} a session open now` : ''}.` : `No sessions in this range yet.`}</h1><div class="heading-facts">${pj.days === 'all' ? 'All time' : pj.days + ' days'}${syncOn(state) ? ' · ' + plural(state.metrics.computersList.length, 'computer') : ''} · cost ≈ at list prices</div></div><div class="page-actions"><div class="chip-group" role="group" aria-label="Range">${[['7', '7d'], ['30', '30d'], ['all', 'All']].map(([v, l]) => btn('p-set', 'days=' + v, `filter-chip ${pj.days === v ? 'active' : ''}`, l, `data-k="days" data-v="${v}" aria-pressed="${pj.days === v}"`)).join('')}</div><div class="chip-group" role="group" aria-label="Sort">${[['active', 'Active'], ['cost', 'Cost'], ['tokens', 'Tokens'], ['name', 'Name']].map(([v, l]) => btn('p-set', 'sort=' + v, `filter-chip ${pj.sort === v ? 'active' : ''}`, l, `data-k="sort" data-v="${v}" aria-pressed="${pj.sort === v}"`)).join('')}</div></div></header>`;
    if (!list.length) return `<div class="page-body" data-tour="projects">${head}<section class="panel-card empty-state"><span class="muted">A project is a folder your tools ran in.</span>${sessionsOf(state).length ? btn('p-set', 'days=all', 'btn-secondary', 'Show all time', 'data-k="days" data-v="all"') : state.accounts.length ? btn('new-session', '', 'btn-primary', 'New session') : '<button class="btn-primary" data-action="open-add-account">Add account</button>'}</section>${renderDialog(state)}</div>`;
    const tokens = list.reduce((a, p) => a + p.tokens, 0), sessions = list.reduce((a, p) => a + p.sessions, 0), live = list.reduce((a, p) => a + p.live, 0);
    const pricedList = list.filter((p) => p.cost != null), cost = pricedList.length ? pricedList.reduce((a, p) => a + p.cost, 0) : null;
    const nDays = pj.days === 'all' ? 60 : Number(pj.days), busiest = [...list].sort((a, b) => b.tokens - a.tokens)[0];
    const tile = (title, value, note, extra = '') => `<div class="account-stat ${extra}" ${extra ? `data-a="go-live" data-key="go-live" tabindex="0" role="link" title="Open the live sessions"` : ''}><span>${title}</span><b data-a-tile="${title}">${value}</b><span title="${esc(note)}">${note}</span></div>`;
    const tiles = `<section class="account-stats a-tiles" aria-label="Project readouts" data-tour="tiles">${tile('Tokens · ' + (pj.days === 'all' ? 'all time' : pj.days + ' days'), D.fmtTokens(tokens), plural(sessions, 'session'))}${tile('At list prices', money(cost), cost != null ? money(cost / nDays) + ' a day ≈' : 'no priced model yet')}${tile('Projects', String(list.length), plural([...new Set(list.flatMap((p) => p.accounts))].length, 'account'))}${tile('Running now', live ? `<span class="a-live">${live}</span>` : '0', live ? esc(list.filter((p) => p.live).map((p) => p.name).join(', ')) : 'nothing running', live ? 'a-tile-btn' : '')}${tile('Busiest', busiest ? esc(busiest.name) : '—', busiest && tokens ? Math.round((busiest.tokens / tokens) * 100) + '% of tokens' + (busiest.cost != null && cost ? ' · ' + Math.round((busiest.cost / cost) * 100) + '% of cost' : '') : '')}</section>`;
    const metric = pj.metric, val = (p) => (metric === 'cost' ? p.cost || 0 : metric === 'sessions' ? p.sessions : p.tokens), total = list.reduce((a, p) => a + val(p), 0) || 1;
    const top = [...list].sort((a, b) => val(b) - val(a)), head5 = top.slice(0, 5), others = top.slice(5), oVal = others.reduce((a, p) => a + val(p), 0), on = (k) => (metric === k ? 'on' : '');
    const prow = (name, key, v, c, tok, n, sw) => `<div class="a-prow"><span class="a-sw ${sw}"></span>${key ? btn('open-project', key, 'a-nm', esc(name)) : `<span class="a-nm muted">${esc(name)}</span>`}<div class="a-track"><i class="${sw}" style="width:${((v / total) * 100).toFixed(1)}%"></i></div><span class="num ${on('cost')}">${money(c)}</span><span class="num ${on('tokens')}">${D.fmtTokens(tok)}</span><span class="num ${on('sessions')}">${n}</span><span class="num">${Math.round((v / total) * 100)}%</span></div>`;
    const dist = `<section class="panel-card" aria-label="Where the tokens and the cost went"><div class="panel-head"><h2 class="panel-title">Where the tokens and the cost went</h2><div class="chip-group" role="group" aria-label="Metric">${[['tokens', 'Tokens'], ['cost', 'Cost'], ['sessions', 'Sessions']].map(([v, l]) => btn('p-set', 'metric=' + v, `filter-chip ${metric === v ? 'active' : ''}`, l, `data-k="metric" data-v="${v}" aria-pressed="${metric === v}"`)).join('')}</div></div>
      <div class="a-stackbar" aria-hidden="true">${head5.map((p, i) => `<i class="${SH[i]}" style="width:${((val(p) / total) * 100).toFixed(1)}%" title="${esc(p.name)}"></i>`).join('')}${oVal ? `<i class="l0" style="width:${((oVal / total) * 100).toFixed(1)}%"></i>` : ''}</div>
      <div class="a-prow head"><span></span><span>Project</span><span>Share of ${metric}</span><span class="num ${on('cost')}">Cost ≈</span><span class="num ${on('tokens')}">Tokens</span><span class="num ${on('sessions')}">Sessions</span><span class="num">Share</span></div>
      ${head5.map((p, i) => prow(p.name, p.key, val(p), p.cost, p.tokens, p.sessions, SH[i])).join('')}${others.length ? prow(`${others.length} other ${others.length === 1 ? 'project' : 'projects'}`, '', oVal, others.some((p) => p.cost != null) ? others.reduce((a, p) => a + (p.cost || 0), 0) : null, others.reduce((a, p) => a + p.tokens, 0), others.reduce((a, p) => a + p.sessions, 0), 'l0') : ''}</section>`;
    const sorted = [...list].sort((a, b) => pj.sort === 'cost' ? (b.cost || 0) - (a.cost || 0) : pj.sort === 'tokens' ? b.tokens - a.tokens : pj.sort === 'name' ? a.name.localeCompare(b.name) : b.last_active_at - a.last_active_at);
    const pages = Math.max(1, Math.ceil(sorted.length / pj.per)); if (pj.page >= pages) pj.page = pages - 1;
    const shown = sorted.slice(pj.page * pj.per, pj.page * pj.per + pj.per), sync = syncOn(state);
    const card = (p) => `<div class="panel-card a-pcard ${p.live ? 'running' : ''}" data-a="open-project" data-id="${esc(p.key)}" data-key="card:${esc(p.key)}" role="link" tabindex="0" aria-label="Sessions of ${esc(p.name)}">
      <div class="a-pwho"><div class="a-phd">${S.rename && S.rename.key === p.key ? `<input id="a-rename-input" class="a-rename" value="${esc(S.rename.draft)}" aria-label="Project name" maxlength="80" oninput="window.HM.aInput('rename',this.value)"/>` : `<span class="a-pname" data-a-name="${esc(p.key)}" title="Double-click to rename">${esc(p.name)}</span>`}${btn('rename', p.key, 'a-ib', ICON.edit, `aria-label="Rename ${esc(p.name)}" title="Rename (or double-click the name)"`)}</div><span class="a-path mono" title="${esc(p.remote || p.root)}">${esc(p.remote || p.root)}</span><div>${tagsHtml(p.tags, 'project', p.key, p.name)}</div></div>
      ${area(p.series)}
      <div class="a-pnum"><b>${money(p.cost)}</b><small>${D.fmtTokens(p.tokens)} tokens${p.cost != null && p.priced_share < 0.95 ? ' · ' + Math.round(p.priced_share * 100) + '% priced' : ''}</small></div>
      <div class="a-pmeta"><span>${plural(p.sessions, 'session')} · ${plural(p.accounts.length, 'account')}</span><span class="${p.live ? 'a-live' : ''}">${p.live ? p.live + ' live' : esc(when(p.last_active_at))}</span>${sync && p.devices.length ? `<span class="pc-dots" title="${esc(p.devices.map((d) => `${d.name}: ${plural(d.sessions, 'session')}`).join(', '))}">${p.devices.map((d) => `<i class="pc-dot" style="background:${PC(d.id)?.col}"></i>`).join('')}${p.here === false ? '<span class="muted" style="margin-left:4px">not cloned here</span>' : ''}</span>` : ''}</div>
      <div class="a-pacts">${btn('pin', p.key, `a-ib ${p.pinned ? 'on' : ''}`, ICON.pin, `aria-label="${p.pinned ? 'Unpin' : 'Pin'} ${esc(p.name)}" title="${p.pinned ? 'Pinned to the sidebar: click to unpin' : 'Pin to the sidebar'}"`)}${p.root ? btn('new-session', p.key, 'a-ib', ICON.play, `aria-label="New session in ${esc(p.name)}" title="New session here"`) : ''}${menuWrap(state, 'project', p.key, btn('project-menu', p.key, 'a-ib', '⋯', `aria-label="More for ${esc(p.name)}" aria-haspopup="menu"`))}</div></div>`;
    const pager = sorted.length > 10 ? `<div class="a-pager">${btn('p-set', 'page=' + (pj.page - 1), 'filter-chip', '‹', `data-k="page" data-v="${Math.max(0, pj.page - 1)}" ${pj.page === 0 ? 'disabled' : ''} aria-label="Previous page"`)}${Array.from({ length: pages }, (_, i) => btn('p-set', 'page=' + i, `filter-chip ${i === pj.page ? 'active' : ''}`, String(i + 1), `data-k="page" data-v="${i}"`)).join('')}${btn('p-set', 'page=' + (pj.page + 1), 'filter-chip', '›', `data-k="page" data-v="${Math.min(pages - 1, pj.page + 1)}" ${pj.page === pages - 1 ? 'disabled' : ''} aria-label="Next page"`)}<span>· ${pj.page * pj.per + 1}–${Math.min(sorted.length, (pj.page + 1) * pj.per)} of ${sorted.length} ·</span>${[10, 25, 50].map((n) => btn('p-set', 'per=' + n, `filter-chip ${pj.per === n ? 'active' : ''}`, String(n), `data-k="per" data-v="${n}"`)).join('')}<span>a page</span></div>` : '';
    return `<div class="page-body" data-tour="projects">${head}${tiles}${dist}<div class="a-pcards" data-tour="cards">${shown.map(card).join('')}</div>${pager}${renderDialog(state)}</div>`;
  }

  // ---------------------------------------------------------------- menus and dialogs (rendered inside the page)
  function menuWrap(state, kind, id, button) {
    const m = A(state).menu, open = m && m.kind === kind && m.id === id;
    const item = (k, label, cls = '') => btn('menu-item', `${kind}:${id}:${k}`, cls, label, `data-k="${k}" role="menuitem"`);
    let items = '';
    if (open && kind === 'project') { const p = PROJECTS.find((x) => x.key === id), onIt = state.page === 'sessions' && A(state).sessions.project === id; items = `${onIt ? '' : item('open', 'Open its sessions')}${item('pin', p?.pinned ? 'Unpin from the sidebar' : 'Pin to the sidebar')}${item('rename', 'Rename project')}${item('merge', 'Merge into another project')}<div class="a-sep"></div>${item('reset', 'Reset name', 'danger')}`; }
    if (open && kind === 'session') items = item('folder', 'Open the log file folder');
    if (open && kind === 'terminal') items = `${item('wt', 'Open sessions in Windows Terminal')}${item('console', 'Open sessions in a console window')}`;
    return `<span class="a-menuwrap">${button}${open ? `<div class="a-menu ${m.up ? 'up' : ''}" role="menu">${items}</div>` : ''}</span>`;
  }
  function renderDialog(state) {
    const S = A(state), d = S.dialog; if (!d) return '';
    const wrap = (label, body) => `<div class="modal-backdrop a-dialog" data-a="dialog-close" data-key="a-dialog"><div class="modal-card" role="dialog" aria-modal="true" aria-label="${esc(label)}"><div class="modal-head"><h2>${esc(label)}</h2>${btn('dialog-close', 'x', 'icon-btn', '✕', 'aria-label="Close"')}</div>${body}</div></div>`;
    const foot = (ok, okAct = 'dialog-ok', left = '', disabled = false) => `<div class="a-foot">${left}${btn('dialog-close', 'cancel', 'btn-secondary', 'Cancel')}${btn(okAct, '', 'btn-primary', ok, disabled ? 'disabled' : '')}</div>`;
    if (d.kind === 'tags') {
      const known = TAGS.map((t) => t.tag).concat(d.extra.filter((t) => !TAGS.some((k) => k.tag === t)));
      return wrap(`Tags of ${d.name}`, `<div class="form-field"><label for="a-tag-new">New tag</label><input id="a-tag-new" value="${esc(d.draft)}" placeholder="Type a tag and press Enter to add it" oninput="window.HM.aInput('tag',this.value)"/><span class="a-hint">Free text, shared across tools and accounts, never written into the tools' files.${d.target === 'project' ? ' A project\'s tag reaches every session of it in filters.' : ''}</span></div>
        <div class="a-tagrows" id="a-tag-list">${known.length ? known.map((t) => { const c = TAGS.find((k) => k.tag === t); return `<label class="a-tagrow a-check"><input type="checkbox" value="${esc(t)}" ${d.selected[t] ? 'checked' : ''} onchange="window.HM.aToggleTag(this.value,this.checked)"/><span class="a-tag ${tagColor(t)}">${esc(t)}</span><span class="a-cnt">${c ? c.sessions + c.projects : ''}</span></label>`; }).join('') : '<span class="muted">No tags yet.</span>'}</div>${foot('Save', 'tags-save')}`);
    }
    if (d.kind === 'rename' || d.kind === 'merge') {
      const p = PROJECTS.find((x) => x.key === d.key);
      return wrap(d.kind === 'rename' ? 'Rename project' : 'Merge project', `<dl class="facts-grid"><dt>Project</dt><dd>${esc(p.name)} <span class="mono muted">${esc(p.key)}</span></dd></dl><div class="form-field"><label for="a-dlg-input">${d.kind === 'rename' ? 'New name' : 'Merge into key'}</label><input id="a-dlg-input" value="${esc(d.draft)}" oninput="window.HM.aInput('dlg',this.value)"/>${d.kind === 'merge' ? '<span class="a-hint">The key of the project to merge into, as shown under its name.</span>' : ''}</div>${foot('Save', 'dialog-ok')}`);
    }
    if (d.kind === 'new') {
      const accts = startable(state), acct = accts.find((a) => a.id === d.accountId), flag = acct ? flagOf(acct.tool) : '';
      const folders = [...new Set([...PROJECTS.filter((p) => p.root && p.here).map((p) => p.root), ...(acct?.recentFolders || [])])];
      return wrap(d.projectName ? `New session in ${d.projectName}` : 'New session', `<div class="form-field"><label for="a-new-folder">Folder</label>${d.fixed ? `<input id="a-new-folder" class="mono" value="${esc(d.folder)}" readonly/>` : `<select id="a-new-folder" class="mono" onchange="window.HM.aInput('folder',this.value)"><option value="">No folder chosen</option>${folders.map((x) => `<option value="${esc(x)}" ${d.folder === x ? 'selected' : ''}>${esc(x)}</option>`).join('')}</select><span class="a-hint">Preview: Choose folder… opens the system picker in the app; recent folders are listed here.</span>`}</div>
        <div class="form-field"><label>Account</label>${accts.length ? `<div class="a-chips">${accts.map((a) => btn('new-pick-acct', a.id, `filter-chip ${d.accountId === a.id ? 'active' : ''}`, `${D.LOGOS[a.tool]}<span>${esc(a.name)}</span>`, `aria-pressed="${d.accountId === a.id}"`)).join('')}</div>` : '<span class="muted">No account can start yet: add one, or log one in, on Accounts.</span>'}</div>
        ${flag ? `<label class="a-check"><input type="checkbox" ${d.skip ? 'checked' : ''} onchange="window.HM.aInput('skip',this.checked)"/><span>Skip permission prompts <code class="mono">${esc(flag)}</code></span></label><span class="a-hint a-warn">The tool runs every command and edit without asking.</span>` : ''}
        ${foot('Start session', 'new-start', '', !(d.folder && acct))}`);
    }
    if (d.kind === 'resumehere') {
      const s = SESSIONS.find((x) => x.id === d.id), p = projectOf(s);
      return wrap(s.title || 'Untitled session', `<div class="muted">${esc(s.project_name || 'no folder')} · ${esc(s.profile)} · ${esc(TOOL[s.harness])}</div><dl class="facts-grid"><dt>Last ran on</dt><dd>${esc(s.device_name)} · ${esc(when(s.last_active_at))}</dd><dt>Opens in</dt><dd class="mono">${p && p.here ? esc(p.root) : `this computer's clone of ${esc(s.project_name || 'the folder')}`}</dd></dl><span>Opens as a new session that continues from the ${esc(s.device_name)} transcript; the ${esc(s.device_name)} copy stays as it is.</span><span class="a-hint">Only the conversation comes across: files changed since, attachments and running shells do not. Pull the branch first.</span>${foot('Resume here', 'resume-here-go')}`);
    }
    return '';
  }

  // ---------------------------------------------------------------- actions
  let openerKey = null, pendingOpen = null;
  function toast(state, text) { state.toastText = text; }
  function setPinned(state, key, on) { const p = PROJECTS.find((x) => x.key === key); if (!p) return; p.pinned = on; const i = D.PINNED_PROJECTS.findIndex((x) => x.name === p.name); if (on && i < 0) D.PINNED_PROJECTS.push({ name: p.name, path: p.root || p.remote, live: SESSIONS.filter((s) => s.open && s.project_key === key).length }); if (!on && i >= 0) D.PINNED_PROJECTS.splice(i, 1); toast(state, on ? 'Pinned to the sidebar' : 'Unpinned'); }
  function openRename(state, key) { const p = PROJECTS.find((x) => x.key === key); if (!p) return; if (document.querySelector(`[data-a-name="${CSS.escape(key)}"]`)) A(state).rename = { key, draft: p.name }; else A(state).dialog = { kind: 'rename', key, draft: p.name }; }
  function finishRename(state, save) { const r = A(state).rename; if (!r) return; A(state).rename = null; const p = PROJECTS.find((x) => x.key === r.key), name = (r.draft || '').trim(); if (save && p && name && name !== p.name) { const pin = D.PINNED_PROJECTS.find((x) => x.name === p.name); if (pin) pin.name = name; p.name = name; SESSIONS.forEach((s) => { if (s.project_key === p.key) s.project_name = name; }); toast(state, `Renamed to ${name}`); } }
  function openNew(state, key) { const p = PROJECTS.find((x) => x.key === key), accts = startable(state); const latest = p ? sessionsOf(state).filter((s) => s.project_key === key).sort((a, b) => b.last_active_at - a.last_active_at)[0] : null; A(state).dialog = { kind: 'new', projectName: p?.name || '', folder: p?.root || '', fixed: !!(p && p.root), accountId: (latest && accts.some((a) => a.id === latest.profile) ? latest.profile : accts[0]?.id) || '', skip: true }; }
  function exportMarkdown(s) {
    const t = transcriptOf(s);
    const text = `# ${s.title || 'Untitled session'}\n\n` + t.turns.map((x) => `## You\n\n${x.prompt}\n\n${x.steps.map((st) => `\`${st.tool}\` ${st.input}${st.status === 'error' ? ' (failed)' : ''}`).join('\n')}\n\n${x.answer.join('\n\n')}\n`).join('\n');
    const blob = new Blob([text], { type: 'text/markdown;charset=utf-8' }), url = URL.createObjectURL(blob), a = document.createElement('a');
    a.href = url; a.download = `${(s.title || 'session').replace(/[^\w -]+/g, '').slice(0, 60) || 'session'}.md`; document.body.appendChild(a); a.click(); a.remove(); setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  function act(state, el, e, root) {
    const S = A(state), a = el.dataset.a, id = el.dataset.id || '', f = S.sessions, pj = S.projects;
    const go = (page, fields) => { S.menu = null; S.dialog = null; S.rename = null; H.navigate(page, fields || {}); };
    switch (a) {
      case 's-set': { const k = el.dataset.k, v = el.dataset.v; if (k === 'q') { state.searchQuery = ''; const inp = document.getElementById('global-search'); if (inp) inp.value = ''; } else if (k === 'live') f.live = v === '1'; else if (k === 'page') f.page = Number(v); else f[k] = v; if (k !== 'page') f.page = 0; return; }
      case 's-clear': Object.assign(f, { live: false, tag: '', pc: '', profile: '', page: 0 }); state.searchQuery = ''; return;
      case 's-fold': f.folds[id] = !f.folds[id]; return;
      case 's-more': f.more[id] = true; return;
      case 's-tag': f.tag = id; f.live = false; f.page = 0; if (state.page !== 'sessions') go('sessions'); return;
      case 'rail-more': f.railMore = true; return;
      case 'rail-live': f.live = !f.live; f.project = ''; f.page = 0; return;
      case 'rail-all': Object.assign(f, { live: false, project: '', tag: '', profile: '', page: 0 }); return;
      case 'rail-project': f.project = f.project === id ? '' : id; f.profile = ''; f.live = false; f.page = 0; return;
      case 'rail-tag': f.tag = f.tag === id ? '' : id; f.page = 0; return;
      case 'go-projects': go('projects'); return;
      case 'go-live': f.live = true; f.project = ''; f.page = 0; go('sessions'); return;
      case 'back-projects': f.project = ''; f.profile = ''; go('projects'); return;
      case 'back-sessions': go('sessions'); return;
      case 'open-session': go('session', { sessionId: id }); return;
      case 'open-project': { if (e.target.closest('[data-a-name]')) { clearTimeout(pendingOpen); pendingOpen = setTimeout(() => { f.project = id; f.profile = ''; f.page = 0; go('sessions'); }, 280); return; } f.project = id; f.profile = ''; f.page = 0; go('sessions'); return; }
      case 'pin': { const p = PROJECTS.find((x) => x.key === id); setPinned(state, id, !p?.pinned); return; }
      case 'rename': openRename(state, id); return;
      case 'project-menu': case 'session-menu': case 'terminal-menu': { const kind = a.split('-')[0]; if (S.menu && S.menu.kind === kind && S.menu.id === id) { S.menu = null; return; } const r = el.getBoundingClientRect(), m = root.getBoundingClientRect(); S.menu = { kind, id, up: r.bottom + 200 > m.bottom }; return; }
      case 'menu-item': { const [kind, key, k] = [el.dataset.id.split(':')[0], el.dataset.id.slice(el.dataset.id.indexOf(':') + 1, el.dataset.id.lastIndexOf(':')), el.dataset.k]; S.menu = null;
        if (kind === 'project') { const p = PROJECTS.find((x) => x.key === key); if (k === 'open') { f.project = key; f.profile = ''; f.page = 0; go('sessions'); } else if (k === 'pin') setPinned(state, key, !p.pinned); else if (k === 'rename') openRename(state, key); else if (k === 'merge') S.dialog = { kind: 'merge', key, draft: '' }; else if (k === 'reset') { const base = (p.root || p.remote).split(/[\\/]/).filter(Boolean).pop(); p.name = base; SESSIONS.forEach((s) => { if (s.project_key === key) s.project_name = base; }); toast(state, 'Reset'); } }
        if (kind === 'session') toast(state, `Preview: would open the log folder ${SESSIONS.find((s) => s.id === key)?.source_path.replace(/[\\/][^\\/]*$/, '')}`);
        if (kind === 'terminal') { S.terminal = k; toast(state, 'Saved'); }
        return; }
      case 'tag-edit': { const kind = el.dataset.kind, target = kind === 'project' ? PROJECTS.find((x) => x.key === id) : SESSIONS.find((x) => x.id === id); S.dialog = { kind: 'tags', target: kind, key: id, name: kind === 'project' ? target.name : target.title || 'this session', selected: Object.fromEntries(target.tags.map((t) => [t, true])), extra: [], draft: '' }; return; }
      case 'tags-save': { const d = S.dialog; if (!d) return; if (d.draft.trim()) addTag(state, d.draft); const tags = Object.keys(d.selected).filter((t) => d.selected[t]); const target = d.target === 'project' ? PROJECTS.find((x) => x.key === d.key) : SESSIONS.find((x) => x.id === d.key); target.tags = tags; tags.forEach((t) => { if (!TAGS.some((k) => k.tag === t)) TAGS.push({ tag: t, sessions: 0, projects: 0 }); }); TAGS.forEach((k) => { k.sessions = SESSIONS.filter((s) => s.tags.includes(k.tag)).length; k.projects = PROJECTS.filter((p) => p.tags.includes(k.tag)).length; }); S.dialog = null; toast(state, tags.length ? `Tagged ${tags.join(', ')}` : 'Tags cleared'); return; }
      case 'new-session': openNew(state, id); return;
      case 'new-pick-acct': S.dialog.accountId = id; return;
      case 'new-start': { const d = S.dialog, acct = state.accounts.find((x) => x.id === d.accountId); if (!d.folder || !acct) return; S.dialog = null; toast(state, `Preview: would start ${acct.name} in ${d.folder}${d.skip && flagOf(acct.tool) ? ' with ' + flagOf(acct.tool) : ''}; no terminal was opened.`); return; }
      case 'dialog-close': S.dialog = null; return;
      case 'dialog-ok': { const d = S.dialog, p = PROJECTS.find((x) => x.key === d.key), v = (d.draft || '').trim(); if (!v) { toast(state, d.kind === 'rename' ? 'Enter a name.' : 'Enter the key of the project to merge into.'); return; } if (d.kind === 'rename') { p.name = v; SESSIONS.forEach((s) => { if (s.project_key === p.key) s.project_name = v; }); } else { const into = PROJECTS.find((x) => x.key === v); if (!into) { toast(state, 'No project has that key.'); return; } SESSIONS.forEach((s) => { if (s.project_key === p.key) { s.project_key = into.key; s.project_name = into.name; } }); } S.dialog = null; toast(state, 'Saved'); return; }
      case 'rv-mode': S.session.mode = id; return;
      case 'step-open': S.session.open[id] = !S.session.open[id]; return;
      case 'more-steps': S.session.more[id] = true; return;
      case 'follow-end': { S.session.follow = true; const chat = document.getElementById('a-chat'); if (chat) chat.scrollTop = chat.scrollHeight; return; }
      case 'export-md': { const s = SESSIONS.find((x) => x.id === id); exportMarkdown(s); toast(state, 'Exported'); return; }
      case 'resume': { const s = SESSIONS.find((x) => x.id === id); toast(state, `Preview: would resume ${s.title || 'the session'} in ${S.terminal === 'console' ? 'a console window' : 'Windows Terminal'}; no terminal was opened.`); return; }
      case 'resume-here': S.dialog = { kind: 'resumehere', id }; return;
      case 'resume-here-go': { const s = SESSIONS.find((x) => x.id === S.dialog.id); S.dialog = null; toast(state, `Preview: would resume here as a new session continued from ${s.device_name}; no terminal was opened.`); return; }
      case 'p-set': { const k = el.dataset.k, v = el.dataset.v; if (k === 'per') { pj.per = Number(v); pj.page = 0; } else if (k === 'page') pj.page = Number(v); else { pj[k] = v; if (k === 'days') pj.page = 0; } return; }
      default: return;
    }
  }
  function addTag(state, raw) { const d = A(state).dialog, t = raw.trim().replace(/\s+/g, '-').toLowerCase(); if (!t) return; if (!TAGS.some((k) => k.tag === t) && !d.extra.includes(t)) d.extra.push(t); d.selected[t] = true; d.draft = ''; }
  H.aInput = (field, value) => { const S = A(H.state); if (field === 'rename' && S.rename) S.rename.draft = value; if (field === 'tag' && S.dialog) S.dialog.draft = value; if (field === 'dlg' && S.dialog) S.dialog.draft = value; if (field === 'folder' && S.dialog) { S.dialog.folder = value; H.redraw(); } if (field === 'skip' && S.dialog) S.dialog.skip = value; };
  H.aToggleTag = (tag, on) => { const d = A(H.state).dialog; if (d) d.selected[tag] = on; };

  // ---------------------------------------------------------------- mount: delegated listeners, focus, live follow
  function mountShared(root, state) {
    const S = A(state);
    const focusFirst = () => { const card = root.querySelector('.a-dialog .modal-card'); if (card) (card.querySelector('input:not([readonly]), select, .filter-chip.active, button:not(.icon-btn)') || card).focus(); };
    const after = (hadDialog, hadMenu, opener) => {
      H.redraw();
      const dialogNow = !!S.dialog, menuNow = !!S.menu;
      if (!hadDialog && dialogNow) { openerKey = opener; queueMicrotask(focusFirst); }
      else if (hadDialog && !dialogNow && openerKey) { const o = root.querySelector(`[data-key="${CSS.escape(openerKey)}"]`); if (o) o.focus(); openerKey = null; }
      else if (!hadMenu && menuNow) { openerKey = opener; const m = root.querySelector('.a-menu button'); if (m) m.focus(); }
      else if (hadMenu && !menuNow && openerKey) { const o = root.querySelector(`[data-key="${CSS.escape(openerKey)}"]`); if (o && !document.activeElement?.closest('.a-dialog')) o.focus(); openerKey = null; }
      const ri = document.getElementById('a-rename-input'); if (ri && document.activeElement !== ri) { ri.focus(); ri.select(); }
      syncChat(state);
    };
    const onClick = (e) => {
      const el = e.target.closest('[data-a]'); if (!el || !root.contains(el)) return;
      if (el.classList.contains('modal-backdrop') && e.target !== el) return;
      if (el.disabled) return;
      if (S.menu && !el.closest('.a-menu') && el.dataset.a !== 'project-menu' && el.dataset.a !== 'session-menu' && el.dataset.a !== 'terminal-menu') S.menu = null;
      const hadDialog = !!S.dialog, hadMenu = !!S.menu, opener = el.closest('.a-menu') ? openerKey : el.dataset.key || null;
      act(state, el, e, root);
      after(hadDialog, hadMenu, opener);
    };
    const onDbl = (e) => { const n = e.target.closest('[data-a-name]'); if (!n || !root.contains(n)) return; clearTimeout(pendingOpen); e.preventDefault(); openRename(state, n.dataset.aName); H.redraw(); const ri = document.getElementById('a-rename-input'); if (ri) { ri.focus(); ri.select(); } };
    const onKey = (e) => {
      const t = e.target;
      if (t.id === 'a-rename-input') { if (e.key === 'Enter') { e.preventDefault(); finishRename(state, true); H.redraw(); } if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); finishRename(state, false); H.redraw(); } return; }
      if (t.id === 'a-tag-new' && e.key === 'Enter') { e.preventDefault(); addTag(state, t.value); H.redraw(); const i = document.getElementById('a-tag-new'); if (i) { i.value = ''; i.focus(); } return; }
      if (t.id === 'a-dlg-input' && e.key === 'Enter') { e.preventDefault(); root.querySelector('[data-a="dialog-ok"]')?.click(); return; }
      if (e.key === 'Escape') { if (S.menu || S.dialog) { e.stopPropagation(); const opener = openerKey; S.menu = null; S.dialog = null; H.redraw(); if (opener) root.querySelector(`[data-key="${CSS.escape(opener)}"]`)?.focus(); openerKey = null; } return; }
      if (e.key === 'Tab' && S.dialog) { const card = root.querySelector('.a-dialog .modal-card'); if (!card) return; const f = [...card.querySelectorAll('button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex="0"]')]; if (!f.length) return; if (e.shiftKey && document.activeElement === f[0]) { e.preventDefault(); f[f.length - 1].focus(); } else if (!e.shiftKey && document.activeElement === f[f.length - 1]) { e.preventDefault(); f[0].focus(); } return; }
      if ((e.key === 'Enter' || e.key === ' ') && t.tagName !== 'BUTTON' && t.tagName !== 'INPUT' && t.tagName !== 'SELECT' && t.hasAttribute('data-a')) { e.preventDefault(); t.click(); }
    };
    const onBlur = (e) => { if (e.target.id === 'a-rename-input' && S.rename) { setTimeout(() => { if (S.rename && !document.getElementById('a-rename-input')?.matches(':focus')) { finishRename(state, true); H.redraw(); } }, 0); } };
    const onScroll = (e) => { if (e.target.id !== 'a-chat') return; const c = e.target, atEnd = c.scrollHeight - c.scrollTop - c.clientHeight < 40; S.session.follow = atEnd; S.session.atEnd = atEnd; const b = document.getElementById('a-toend'); if (b) b.hidden = atEnd; };
    root.addEventListener('click', onClick); root.addEventListener('dblclick', onDbl); root.addEventListener('keydown', onKey); root.addEventListener('focusout', onBlur); root.addEventListener('scroll', onScroll, true);
    return () => { root.removeEventListener('click', onClick); root.removeEventListener('dblclick', onDbl); root.removeEventListener('keydown', onKey); root.removeEventListener('focusout', onBlur); root.removeEventListener('scroll', onScroll, true); S.menu = null; S.dialog = null; S.rename = null; };
  }
  function syncChat(state) { const rv = A(state).session, c = document.getElementById('a-chat'); if (!c) return; if (rv.needScroll) { rv.needScroll = false; if (rv.follow) c.scrollTop = c.scrollHeight; } const atEnd = c.scrollHeight - c.scrollTop - c.clientHeight < 40; rv.atEnd = atEnd; const b = document.getElementById('a-toend'); if (b) b.hidden = atEnd || (rv.follow && c.scrollTop === c.scrollHeight - c.clientHeight); }

  H.pages.sessions = { title: 'Sessions', render: renderSessions, mount(root, state) {
    const off = mountShared(root, state);
    // the live tick: open rows grow as their tools write; the page keeps scroll, focus and folds (prototype simulation, off when Still)
    const timer = setInterval(() => { if (state.motion !== 'live' || state.modal || A(state).dialog || A(state).menu || A(state).rename) return; sessionsOf(state).filter((s) => s.open).forEach((s, i) => { s.tokens += 2400 + i * 300; if (i % 3 === 0) s.messages += 1; }); H.redraw(); }, 3000);
    return () => { clearInterval(timer); off(); };
  } };
  H.pages.session = { title: 'Session', render: renderSession, mount(root, state) {
    const off = mountShared(root, state);
    queueMicrotask(() => syncChat(state));
    const timer = setInterval(() => { const rv = A(state).session, s = sessionsOf(state).find((x) => x.id === state.sessionId); if (!s || !s.open || state.motion !== 'live' || state.modal || A(state).dialog || A(state).menu) return; rv.ticks++; appendLive(s, rv.ticks); H.redraw(); const c = document.getElementById('a-chat'); if (c && rv.follow) c.scrollTop = c.scrollHeight; syncChat(state); }, 2000);
    return () => { clearInterval(timer); off(); };
  } };
  H.pages.projects = { title: 'Projects', render: renderProjects, mount(root, state) {
    const off = mountShared(root, state);
    // a rail or account click with data-project means "this project's sessions": hand it to Sessions
    if (state.projectId) setTimeout(() => { const id = state.projectId; if (!id || state.page !== 'projects') return; const f = A(state).sessions; f.project = PROJECTS.find((p) => p.key === id)?.key || PROJECTS.find((p) => p.name === id)?.key || ''; f.profile = ''; f.page = 0; state.projectId = null; H.navigate('sessions'); }, 0);
    // a tile number that changed since the last draw flashes once
    const mo = new MutationObserver((muts) => { for (const m of muts) { const b = (m.target.nodeType === 3 ? m.target.parentElement : m.target)?.closest?.('[data-a-tile]'); if (b && !b.classList.contains('a-flash')) { b.classList.add('a-flash'); b.addEventListener('animationend', () => b.classList.remove('a-flash'), { once: true }); } } });
    mo.observe(root, { subtree: true, characterData: true, childList: true });
    return () => { mo.disconnect(); off(); };
  } };
})();
