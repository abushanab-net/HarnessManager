// Design A mock data. Shapes follow the app's own (docs/overview-redesign-2026-11/data.js, home.js, accounts.js).
// Classic script so the page opens from disk. Every value is "measured" unless it carries an `est` flag or ≈.
(function () {
  const MIN = 60e3, H = 3600e3, DAY = 24 * H;
  const NOW = new Date(2026, 10, 6, 21, 10).getTime(); // Fri 6 Nov 2026, 21:10
  const LIMITS_READ = NOW - 14 * MIN; // limits read 14 m ago
  const USAGE_READ = NOW - 20e3;      // usage read just now

  const computers = [
    { id: 'devpc', name: 'DEVELOPMENTPC', me: true, col: '#6FA8F5', seen: 0 },
    { id: 'msi', name: 'SAMYMSI', col: '#C8A45A', seen: 50e3 },
    { id: 'mbp', name: 'Samys-MacBook-Pro', col: '#C792EA', seen: 26 * MIN },
  ];
  // kinds follow the app's Caps (crates/core/src/harness/*.rs): subscription_login, api_key, routable; quota = the tool has limits to read
  const tools = [
    { id: 'claude', label: 'Claude Code', ver: '2.4.1', installed: true, kinds: ['subscription', 'api', 'routed'], quota: true, main: '~/.claude' },
    { id: 'codex', label: 'Codex', ver: '0.98.0', installed: true, update: '0.99.0', kinds: ['subscription', 'api'], quota: true, main: '~/.codex' },
    { id: 'opencode', label: 'OpenCode', ver: '1.3.2', installed: true, kinds: ['subscription'], quota: false, main: '~/.config/opencode' },
    { id: 'agy', label: 'Antigravity', ver: '1.8.0', installed: true, kinds: ['subscription'], quota: false, main: '~/.antigravity' },
    { id: 'pi', label: 'Pi', ver: null, installed: false, kinds: ['subscription', 'api', 'routed'], quota: false, main: '~/.pi' },
    { id: 'omp', label: 'Oh My Pi', ver: null, installed: false, kinds: ['subscription'], quota: false, main: '~/.omp' },
  ];

  // windows: used % (measured), reset = ms from now, out = ms from now when the pace runs it out before the reset (≈)
  const accounts = [
    { id: 'service-account-resal', tool: 'claude', kind: 'subscription', plan: 'Max 5x', email: 'akusumah@resal.me', added: 'Nov 4', label: '',
      windows: [{ label: '5-hour', used: 2, reset: 4 * H + 8 * MIN }, { label: 'Week', used: 0, reset: 4 * DAY + 7 * H }, { label: 'Week · Opus', used: 0, reset: 4 * DAY + 7 * H }],
      tokens7: 2.75e6, cost7: 1.61, spark: [0, 0, 0, 0, 0, 0.1, 1], open: 1, read: LIMITS_READ, advice: 'Most room of any account: start here next.' },
    { id: 'samykabu-personal', tool: 'claude', kind: 'subscription', plan: 'Max 20x', email: 'samykabu@gmail.com', added: 'Nov 4', label: '',
      windows: [{ label: '5-hour', used: 4, reset: 2 * H + 30 * MIN }, { label: 'Week', used: 4, reset: 5 * DAY + 11 * H }, { label: 'Week · Opus', used: 6, reset: 5 * DAY + 11 * H }],
      tokens7: 56.85e6, cost7: 41.41, spark: [0.2, 0.1, 0, 0.4, 0.3, 0.6, 0.5], open: 1, read: LIMITS_READ },
    { id: 'claude-main', tool: 'claude', kind: 'subscription', main: true, plan: 'Max 20x', email: 'samykabu@gmail.com', added: 'Nov 4', label: 'main',
      windows: [{ label: '5-hour', used: 38, reset: 2 * H + 12 * MIN }, { label: 'Week', used: 28, reset: 3 * DAY + 18 * H + 36 * MIN }, { label: 'Week · Opus', used: 56, reset: 3 * DAY + 18 * H + 36 * MIN, out: 1 * DAY + 11 * H, est: true }],
      tokens7: 7.51e9, cost7: 2795.31, spark: [0.9, 0.7, 1, 0.55, 0.8, 0.85, 0.6], open: 4, read: LIMITS_READ, advice: 'Opus week runs out ≈ Sun 08:04 at this pace.' },
    { id: 'samykabu-teksa', tool: 'claude', kind: 'subscription', plan: 'Pro', email: 'samy@teksa.sa', added: 'Nov 5', label: '',
      windows: [{ label: '5-hour', used: 46, reset: 1 * H + 40 * MIN }, { label: 'Week', used: 23, reset: 2 * DAY + 14 * H + 10 * MIN }],
      tokens7: 410e6, cost7: 162.8, spark: [0.3, 0.5, 0.4, 0.6, 0.2, 0.7, 0.5], open: 1, read: LIMITS_READ },
    { id: 'codex-main', tool: 'codex', kind: 'subscription', main: true, plan: 'Pro', email: 'samykabu@gmail.com', added: 'Nov 4', label: 'main',
      windows: [{ label: '5-hour', used: 41, reset: 3 * H + 2 * MIN }, { label: 'Week', used: 73, reset: 3 * DAY + 18 * H + 36 * MIN, out: 1 * DAY + 3 * H + 46 * MIN, est: true }],
      tokens7: 1.98e9, cost7: 611.2, spark: [0.6, 0.8, 0.7, 0.9, 1, 0.8, 0.7], open: 2, read: LIMITS_READ, advice: 'Week runs out ≈ Sun 00:56, 2 d 14 h before it resets.' },
    { id: 'sabushanab-resal', tool: 'claude', kind: 'subscription', plan: 'Max 5x', email: 'sabushanab@resal.me', added: 'Nov 4', label: '',
      windows: [{ label: '5-hour', used: 71, reset: 22 * MIN }, { label: 'Week', used: 13, reset: 1 * DAY + 12 * H + 2 * MIN }, { label: 'Week · Opus', used: 20, reset: 1 * DAY + 12 * H + 2 * MIN }],
      tokens7: 1.12e9, cost7: 380.4, spark: [0.4, 0.6, 0.5, 0.8, 0.7, 0.9, 0.8], open: 1, read: LIMITS_READ, advice: '5-hour resets in 22 m: plenty of room after that.' },
    { id: 'opencode-main', tool: 'opencode', kind: 'subscription', main: true, plan: null, email: 'samykabu@gmail.com', added: 'Nov 4', label: 'main',
      windows: null, tokens7: 92e6, cost7: 12.9, spark: [0.1, 0.2, 0, 0.3, 0.2, 0.4, 0.3], open: 1, read: null },
    { id: 'agy-main', tool: 'agy', kind: 'subscription', main: true, plan: null, email: 'samykabu@gmail.com', added: 'Nov 4', label: 'main',
      windows: null, tokens7: 0, cost7: 0, spark: [0, 0, 0, 0, 0, 0, 0], open: 0, read: null, needsLogin: true },
  ];
  // four more for the 12-account state: one exhausted now, one never read, one on a tool without limits
  const extra = [
    { id: 'samykabu-work', tool: 'claude', kind: 'subscription', plan: 'Max 5x', email: 'samy@work.sa', added: 'Nov 6', label: '',
      windows: [{ label: '5-hour', used: 100, reset: 1 * H + 12 * MIN }, { label: 'Week', used: 61, reset: 2 * DAY + 3 * H }, { label: 'Week · Opus', used: 70, reset: 2 * DAY + 3 * H }],
      tokens7: 620e6, cost7: 240.1, spark: [0.2, 0.4, 0.6, 0.5, 0.9, 1, 0.8], open: 0, read: LIMITS_READ, advice: '5-hour exhausted: back in 1 h 12 m.' },
    { id: 'codex-resal', tool: 'codex', kind: 'subscription', plan: 'Plus', email: 'codex@resal.me', added: 'Nov 5', label: '',
      windows: [{ label: '5-hour', used: 12, reset: 4 * H + 20 * MIN }, { label: 'Week', used: 35, reset: 5 * DAY + 2 * H }],
      tokens7: 310e6, cost7: 96.4, spark: [0.3, 0.2, 0.5, 0.4, 0.6, 0.3, 0.4], open: 0, read: LIMITS_READ },
    { id: 'opencode-resal', tool: 'opencode', kind: 'subscription', plan: null, email: 'oc@resal.me', added: 'Nov 6', label: '',
      windows: null, tokens7: 14e6, cost7: 2.1, spark: [0, 0, 0.2, 0.1, 0.3, 0.2, 0.2], open: 0, read: null },
    { id: 'pi-main', tool: 'pi', kind: 'api', main: true, plan: null, email: 'samykabu@gmail.com', added: 'Nov 6', label: 'main',
      windows: null, tokens7: 48e6, cost7: 9.8, spark: [0.1, 0.3, 0.2, 0.4, 0.3, 0.5, 0.4], open: 0, read: null },
  ];

  // sessions running now. since/wrote = ms before NOW. tokens/cost measured for this session.
  const sessions = [
    { id: 's1', project: 'HarnessManager', computer: 'devpc', account: 'claude-main', tool: 'claude', model: 'claude-opus-4-7', since: 72 * MIN, wrote: 4e3, tokens: 2.1e6, cost: 3.4, first: 1.1 },
    { id: 's2', project: 'resal-wiki', computer: 'devpc', account: 'sabushanab-resal', tool: 'claude', model: 'claude-sonnet-4-6', since: 34 * MIN, wrote: 18e3, tokens: 640e3, cost: 0.92, first: 0.8 },
    { id: 's3', project: 'sanduq-api', computer: 'devpc', account: 'codex-main', tool: 'codex', model: 'gpt-5.1-codex', since: 123 * MIN, wrote: 2 * MIN, tokens: 4.8e6, cost: 5.1, first: 0.9 },
    { id: 's4', project: 'dgx-serve', computer: 'devpc', account: 'opencode-main', tool: 'opencode', model: 'deepseek-v3.2', since: 19 * MIN, wrote: 9e3, tokens: 210e3, cost: 0.11, first: 0.6 },
    { id: 's5', project: 'model-adoption', computer: 'devpc', account: 'claude-main', tool: 'claude', model: 'claude-opus-4-7', since: 8 * MIN, wrote: 1e3, tokens: 380e3, cost: 0.71, first: 1.3 },
    { id: 's6', project: 'teksa-app', computer: 'devpc', account: 'samykabu-teksa', tool: 'claude', model: 'claude-sonnet-4-6', since: 51 * MIN, wrote: 6 * MIN, tokens: 1.3e6, cost: 1.5, first: 0.9 },
    { id: 's7', project: 'sanduq-ci', computer: 'msi', account: 'codex-main', tool: 'codex', model: 'gpt-5.1-codex', since: 44 * MIN, wrote: 11e3, tokens: 1.9e6, cost: 2.2, first: 1.0 },
    { id: 's8', project: 'vektragrid', computer: 'msi', account: 'claude-main', tool: 'claude', model: 'claude-opus-4-7', since: 100 * MIN, wrote: 3e3, tokens: 3.6e6, cost: 6.0, first: 1.2 },
    { id: 's9', project: 'athar-mcp', computer: 'msi', account: 'service-account-resal', tool: 'claude', model: 'claude-sonnet-4-6', since: 12 * MIN, wrote: 40e3, tokens: 150e3, cost: 0.2, first: 0.7 },
    { id: 's10', project: 'resal-mobile', computer: 'mbp', account: 'samykabu-personal', tool: 'claude', model: 'claude-sonnet-4-6', since: 190 * MIN, wrote: 27 * MIN, tokens: 5.2e6, cost: 4.9, first: 1.0 },
  ];

  const projects = { count: 23, pinned: [['HarnessManager', 2], ['sanduq-api', 1], ['resal-wiki', 1], ['vektragrid', 1]] };
  const counts = { mcp: 11, skills: 255, apps: 1, sessions: sessions.length, newerHarness: '1.8.4', version: '1.8.3' };

  // readouts: Tokens, Cost (API list prices), Cached %, each with a dot-grid trend (28 values, 0..1) and a delta
  const rng = (seed) => () => { seed = (seed * 16807) % 2147483647; return (seed - 1) / 2147483646; };
  const grid = (seed, bias) => { const r = rng(seed); return Array.from({ length: 28 }, (_, i) => Math.min(1, Math.max(0.05, bias + (i / 28) * 0.4 + (r() - 0.5) * 0.5))); };
  const readouts = {
    today: { tokens: { v: 2.41e9, d: '+18%' }, cost: { v: 571.37, d: '−$256' }, cached: { v: 98, d: '+0 pt' }, grid: [grid(3, 0.3), grid(5, 0.5), grid(7, 0.8)] },
    '7d': { tokens: { v: 10.9e9, d: '+9%' }, cost: { v: 2840.12, d: '+$310' }, cached: { v: 97, d: '+1 pt' }, grid: [grid(11, 0.4), grid(13, 0.4), grid(17, 0.8)] },
    '30d': { tokens: { v: 38.2e9, d: '+31%' }, cost: { v: 9120.4, d: '+$1,980' }, cached: { v: 96, d: '+2 pt' }, grid: [grid(19, 0.3), grid(23, 0.3), grid(29, 0.7)] },
    all: { tokens: { v: 212e9, d: '' }, cost: { v: 48900, d: '' }, cached: { v: 95, d: '' }, grid: [grid(31, 0.2), grid(37, 0.2), grid(41, 0.6)] },
  };

  // 53 weeks of days, with a per-computer split
  const r2 = rng(7);
  const days = [];
  for (let i = 370; i >= 0; i--) {
    const t = NOW - i * DAY, dw = new Date(t).getDay(), growth = 0.35 + 0.65 * (1 - i / 370);
    const busy = dw === 0 || dw === 6 ? 0.35 : 1, off = r2() < 0.06;
    const tokens = off ? 0 : Math.round((10 + r2() * 45) * 1e6 * growth * busy * (i === 37 ? 3.6 : 1));
    days.push({ t, tokens, by: { devpc: Math.round(tokens * 0.6), msi: Math.round(tokens * 0.3), mbp: Math.round(tokens * 0.1) } });
  }

  const attention = [
    { t: NOW - 14 * MIN, who: 'codex-main · week', when: '≈ Sun 00:56', why: 'Runs out 2 d 14 h before its reset at today\'s pace.', kind: 'warn' },
    { t: NOW - 14 * MIN, who: 'claude-main · Opus week', when: '≈ Sun 08:04', why: 'Runs out 3 d 4 h before its reset at today\'s pace.', kind: 'warn' },
    { t: NOW - 2 * H, who: 'Codex 0.99.0', when: 'update', why: 'A newer Codex is available; Apps installs it.', kind: 'info' },
    { t: NOW - 9 * H, who: 'agy-main', when: 'login', why: 'The login has expired; sign in again from Accounts.', kind: 'neg' },
    { t: NOW - 26 * MIN, who: 'Samys-MacBook-Pro', when: '26 m ago', why: 'Not heard from for 26 m; its session may be asleep.', kind: 'info' },
  ];

  const r3 = rng(99);
  const pool = [['claude-main', 'claude', 'claude-opus-4-7', 'devpc'], ['codex-main', 'codex', 'gpt-5.1-codex', 'devpc'], ['sabushanab-resal', 'claude', 'claude-sonnet-4-6', 'devpc'],
    ['claude-main', 'claude', 'claude-opus-4-7', 'msi'], ['opencode-main', 'opencode', 'deepseek-v3.2', 'devpc'], ['codex-main', 'codex', 'gpt-5.1-codex', 'msi'], ['service-account-resal', 'claude', 'claude-sonnet-4-6', 'msi']];
  const replies = Array.from({ length: 14 }, (_, i) => {
    const m = pool[Math.floor(r3() * pool.length)];
    const input = Math.round(800 + r3() * 9000), output = Math.round(200 + r3() * 3800), cache = Math.round(20000 + r3() * 140000);
    return { ts: NOW - i * (30e3 + r3() * 80e3), account: m[0], tool: m[1], model: m[2], computer: m[3], tokens: input + output + cache, speed: Math.round(60 + r3() * 140), first: +(0.5 + r3() * 3.2).toFixed(1) };
  });
  const repliesToday = 10472;

  // By model (Accounts page)
  const byModel = [['claude-opus-4-7', 6.9e9, 2610.4], ['claude-sonnet-4-6', 1.2e9, 198.2], ['gpt-5.1-codex', 1.98e9, 611.2], ['deepseek-v3.2', 92e6, 12.9], ['claude-haiku-4-5', 310e6, 24.6]];
  // Tokens per day by account, 7 days (Accounts page chart), in tokens
  const r4 = rng(5);
  const perDay = Array.from({ length: 7 }, (_, d) => { const o = {}; for (const a of accounts.concat(extra)) o[a.id] = Math.round(a.tokens7 / 7 * (0.6 + r4() * 0.8)); return { t: NOW - (6 - d) * DAY, by: o }; });
  const routes = ['openrouter', 'deepseek', 'cheap-fast'];

  // first-run variants
  const firstRun = {
    found: { tools: ['claude', 'codex'], message: 'Two tools found on DEVELOPMENTPC' },
    none: { tools: [], message: 'No AI coding tool found on DEVELOPMENTPC' },
  };

  window.A = { NOW, MIN, H, DAY, LIMITS_READ, USAGE_READ, computers, tools, accounts, extra, sessions, projects, counts, readouts, days, attention, replies, repliesToday, byModel, perDay, routes, firstRun, rng };
})();
