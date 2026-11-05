// The Overview's data as the app has it (home.js: overview, dash, live, forecast, caps), the screen simulator,
// and the feedback form. Classic script (no modules) so the page opens straight from disk.
(function () {
  const H = 3600e3, DAY = 24 * H;
  const NOW = new Date(2026, 10, 5, 15, 42).getTime(); // Thu 5 Nov 2026, 15:42
  const MEASURED = NOW - 38e3; // limits read 38 s ago

  // ---------- the setup ----------
  const computers = [
    { id: 'studio', name: 'Studio', me: true, col: '#6FA8F5', seen: 0 },
    { id: 'lab', name: 'Lab', col: '#C8A45A', seen: 40e3 },
    { id: 'surface', name: 'Surface', col: '#E384B8', seen: 95e3 },
  ];
  const tools = [
    { id: 'claude', label: 'Claude Code', ver: '2.4.1' },
    { id: 'codex', label: 'Codex', ver: '0.98.0' },
    { id: 'opencode', label: 'OpenCode', ver: '1.3.2' },
    { id: 'pi', label: 'Pi', ver: '0.41.0' },
    { id: 'agy', label: 'Antigravity', ver: '1.8.0' },
  ];
  // windows: used %, reset (ms from now), forecast at the reset (%), runs out (ms from now) when before the reset
  const accounts = [
    { id: 'claude-max', label: 'Claude Max', tool: 'claude', kind: 'subscription', plan: 'Max 20x', tps: 1420, tokens: 21.4e6, on: ['studio', 'lab'],
      windows: [{ label: '5-hour', used: 62, reset: 2.3 * H, at: 88 }, { label: 'Weekly', used: 41, reset: 3.4 * DAY, at: 72 }] },
    { id: 'codex-pro', label: 'Codex Pro', tool: 'codex', kind: 'subscription', plan: 'Pro', tps: 540, tokens: 14.9e6, on: ['lab'],
      windows: [{ label: '5-hour', used: 34, reset: 1.2 * H, at: 49 }, { label: 'Weekly', used: 78, reset: 2.31 * DAY, at: 100, out: 1.935 * DAY }] },
    { id: 'claude-team', label: 'Claude Team', tool: 'claude', kind: 'subscription', plan: 'Team', tps: 260, tokens: 6.8e6, on: ['studio'],
      windows: [{ label: '5-hour', used: 18, reset: 4.1 * H, at: 31 }, { label: 'Weekly', used: 33, reset: 5.2 * DAY, at: 58 }] },
    { id: 'opencode-routed', label: 'OpenCode routed', tool: 'opencode', kind: 'routed', route: 'cheap-fast', tps: 90, tokens: 3.9e6, on: ['surface'],
      cap: { cap: 25, spent: 11.2 } },
    { id: 'pi-openrouter', label: 'Pi on OpenRouter', tool: 'pi', kind: 'routed', route: 'openrouter', tps: 30, tokens: 1.2e6, on: ['studio'] },
    { id: 'antigravity', label: 'Antigravity', tool: 'agy', kind: 'subscription', plan: null, needsLogin: true, tps: 0, tokens: 0, on: [] },
  ];
  const providers = [
    { id: 'anthropic', label: 'Anthropic', via: 'direct', tps: 1680, first: 1.1 },
    { id: 'openai', label: 'OpenAI', via: 'direct', tps: 540, first: 0.9 },
    { id: 'google', label: 'Google', via: 'direct', tps: 0, first: null },
    { id: 'deepseek', label: 'DeepSeek', via: 'router', tps: 90, first: 0.6 },
    { id: 'openrouter', label: 'OpenRouter', via: 'router', tps: 30, first: 1.4 },
    { id: 'groq', label: 'Groq', via: 'router', tps: 0, paused: true, retry: 4 * 60e3 },
  ];
  const router = { port: 8787, running: true, routes: 2, errors: 3 };
  // the hops every token takes: account -> tool -> (router ->) provider. kind: c direct login, a through the router, r broken
  const edges = [
    ['claude-max', 'claude', 'c', 1420], ['claude-team', 'claude', 'c', 260], ['codex-pro', 'codex', 'c', 540],
    ['opencode-routed', 'opencode', 'a', 90], ['pi-openrouter', 'pi', 'a', 30], ['antigravity', 'agy', 'r', 0],
    ['claude', 'anthropic', 'c', 1680], ['codex', 'openai', 'c', 540], ['agy', 'google', 'r', 0],
    ['opencode', 'router', 'a', 90], ['pi', 'router', 'a', 30],
    ['router', 'deepseek', 'a', 90], ['router', 'openrouter', 'a', 30], ['router', 'groq', 'r', 0],
  ];
  const running = [
    { project: 'HarnessManager', device: 'studio', account: 'Claude Max', tool: 'Claude Code', since: 72 * 60e3 },
    { project: 'sanduq-api', device: 'lab', account: 'Codex Pro', tool: 'Codex', since: 34 * 60e3 },
    { project: 'resal-wiki', device: 'studio', account: 'Claude Team', tool: 'Claude Code', since: 8 * 60e3 },
    { project: 'dgx-serve', device: 'surface', account: 'OpenCode routed', tool: 'OpenCode', since: 123 * 60e3 },
    { project: 'model-adoption', device: 'lab', account: 'Claude Max', tool: 'Claude Code', since: 19 * 60e3 },
  ];

  // ---------- key numbers: today against yesterday at the same hour ----------
  const kpi = {
    tokens: { v: 48.2e6, prev: 43.0e6, by: { studio: 29.1e6, lab: 15.2e6, surface: 3.9e6 } },
    cost: { v: 184.37, prev: 162.27, billed: 11.2 },
    hit: { v: 0.91, prev: 0.88 },
    requests: { v: 3912, prev: 3530 },
    risk: { n: 1, who: 'Codex Pro', win: 'weekly limit' },
  };
  // hourly series for today (0..15) and the same hours yesterday
  const rng = (seed) => () => { seed = (seed * 16807) % 2147483647; return (seed - 1) / 2147483646; };
  const r1 = rng(42);
  const curve = (h) => (h < 8 ? 0.05 + h * 0.01 : h < 12 ? 0.6 + (h - 8) * 0.12 : h < 13 ? 0.7 : 0.9 + (h - 13) * 0.08);
  const hourly = (scale, n = 16) => Array.from({ length: n }, (_, h) => Math.max(0, curve(h) * scale * (0.75 + r1() * 0.5)));
  const series = { tokens: hourly(5.2e6), cost: hourly(19), hit: Array.from({ length: 16 }, () => 0.86 + r1() * 0.08), tps: Array.from({ length: 60 }, (_, i) => 1500 + 900 * Math.sin(i / 7) + r1() * 500) };

  // ---------- 53 weeks of days ----------
  const r2 = rng(7);
  const days = [];
  for (let i = 370; i >= 0; i--) {
    const t = NOW - i * DAY, dw = new Date(t).getDay(), growth = 0.35 + 0.65 * (1 - i / 370);
    const busy = dw === 0 || dw === 6 ? 0.35 : 1;
    const off = r2() < 0.06;
    const tokens = off ? 0 : Math.round((10 + r2() * 45) * 1e6 * growth * busy);
    days.push({ t, day: new Date(t), tokens, cost: tokens / 1e6 * (3.1 + r2()), requests: Math.round(tokens / 12000), by: { studio: Math.round(tokens * 0.6), lab: Math.round(tokens * 0.3), surface: Math.round(tokens * 0.1) } });
  }

  // ---------- the latest replies ----------
  const models = [['Claude Max', 'claude', 'claude-opus-5-5'], ['Codex Pro', 'codex', 'gpt-6.1-sol'], ['Claude Team', 'claude', 'claude-sonnet-5-5'], ['OpenCode routed', 'opencode', 'deepseek-v4'], ['Claude Max', 'claude', 'claude-haiku-5-5'], ['Pi on OpenRouter', 'pi', 'kimi-k3']];
  const r3 = rng(99);
  const replies = Array.from({ length: 12 }, (_, i) => {
    const m = models[Math.floor(r3() * models.length)];
    const input = Math.round(800 + r3() * 9000), output = Math.round(200 + r3() * 3800), cache = Math.round(20000 + r3() * 140000);
    return { ts: NOW - i * (40e3 + r3() * 90e3), account: m[0], tool: m[1], model: m[2], input, output, cacheRead: cache, cacheWrite: Math.round(r3() * 6000),
      cost: (input * 5 + output * 25 + cache * 0.5) / 1e6, tps: Math.round(60 + r3() * 140), first: +(0.5 + r3() * 4.2).toFixed(1), device: ['studio', 'lab', 'surface'][Math.floor(r3() * 3)] };
  });

  // ---------- helpers ----------
  const fmt = (n) => (n == null ? '—' : n >= 1e9 ? (n / 1e9).toFixed(1) + 'B' : n >= 1e6 ? (n / 1e6).toFixed(1) + 'M' : n >= 1e4 ? Math.round(n / 1e3) + 'k' : n >= 1e3 ? (n / 1e3).toFixed(1) + 'k' : String(Math.round(n)));
  const money = (c, d = 2) => '$' + c.toLocaleString('en-US', { minimumFractionDigits: d, maximumFractionDigits: d });
  const dur = (ms) => { const m = Math.round(ms / 60e3); if (m < 60) return m + ' min'; const h = Math.floor(m / 60); if (h < 48) return h + ' h' + (m % 60 && h < 10 ? ' ' + (m % 60) + ' min' : ''); return Math.round(h / 24) + ' days'; };
  const at = (ms) => new Date(NOW + ms).toLocaleString('en-GB', { weekday: 'short', hour: '2-digit', minute: '2-digit', hour12: false });
  const clock = (t) => new Date(t).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
  const h = (html) => { const t = document.createElement('template'); t.innerHTML = html.trim(); return t.content.firstElementChild; };
  const css = (el, name) => getComputedStyle(el).getPropertyValue(name).trim();
  const rgb = (hex) => { const m = hex.replace('#', ''); const n = parseInt(m.length === 3 ? m.split('').map((c) => c + c).join('') : m, 16); return [(n >> 16) & 255, (n >> 8) & 255, n & 255]; };
  const pct = (cur, prev) => { const d = Math.round(((cur - prev) / prev) * 100); return (d > 0 ? '+' : d < 0 ? '−' : '±') + Math.abs(d) + '%'; };
  const reduced = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const label = (id) => (accounts.find((a) => a.id === id) || tools.find((t) => t.id === id) || providers.find((p) => p.id === id) || { label: id === 'router' ? 'Local router' : id }).label;
  const pcName = (id) => (computers.find((c) => c.id === id) || {}).name || id;
  const pcCol = (id) => (computers.find((c) => c.id === id) || {}).col || '#888';
  const toolLabel = (id) => (tools.find((t) => t.id === id) || {}).label || id;
  // every limit with a reset, tightest first (home.js burnRows)
  const burn = () => {
    const out = [];
    for (const a of accounts) {
      for (const w of a.windows || []) out.push({ acct: a, name: a.label, win: w.label, used: w.used, left: 100 - w.used, reset: w.reset, at: w.at, out: w.out || null });
      if (a.cap) { const mid = new Date(NOW); mid.setHours(24, 0, 0, 0); out.push({ acct: a, name: a.label, win: '$' + a.cap.cap + ' daily cap', used: (a.cap.spent / a.cap.cap) * 100, left: Math.round(100 - (a.cap.spent / a.cap.cap) * 100), reset: mid - NOW, at: null, out: null, cap: a.cap }); }
    }
    return out.sort((x, y) => x.left - y.left);
  };

  // A tiny shell every variation shares: the sidebar's places and the counts beside them.
  const NAV = [['Overview', 'dash', money(kpi.cost.v)], ['Sessions', 'term', String(running.length)], ['Projects', 'folder', '23'], ['Accounts', 'users', String(accounts.length)]];
  const NAV2 = [['MCP servers', '14'], ['Skills', '61'], ['System prompts', ''], ['API keys & routing', ''], ['Apps', '1']];
  const ICON = {
    dash: '<path d="M3 3h7v9H3zM14 3h7v5h-7zM14 12h7v9h-7zM3 16h7v5H3z"/>',
    term: '<path d="M4 17l6-5-6-5M12 19h8"/>',
    folder: '<path d="M3 6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    users: '<path d="M16 19v-1a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v1M9 10a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM22 19v-1a4 4 0 0 0-3-3.9M16 4.1a3 3 0 0 1 0 5.8"/>',
    grid: '<path d="M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z"/>',
    gear: '<path d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>',
    book: '<path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20V3H6.5A2.5 2.5 0 0 0 4 5.5zM4 19.5A2.5 2.5 0 0 0 6.5 22H20v-5"/>',
    search: '<circle cx="11" cy="11" r="7"/><path d="M21 21l-4.3-4.3"/>',
    play: '<path d="M7 4l13 8-13 8z"/>',
    refresh: '<path d="M21 12a9 9 0 1 1-2.6-6.4M21 4v5h-5"/>',
  };
  const icon = (k, s = 16) => `<svg width="${s}" height="${s}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICON[k]}</svg>`;
  const shell = (main, extraTop = '') => `
    <aside class="side"><div class="brand"><svg width="26" height="26" viewBox="0 0 32 32" aria-hidden="true"><rect x="3" y="3" width="10" height="26" rx="3" fill="var(--ac)"/><rect x="19" y="3" width="10" height="26" rx="3" fill="var(--brand2)"/><rect x="8" y="13" width="16" height="6" rx="3" fill="var(--brand3)"/></svg><div><b>Harness Manager</b><small>Studio · 3 computers</small></div></div>
      <nav class="nav">${NAV.map(([t, i, c], n) => `<a class="${n ? '' : 'on'}" href="#" onclick="return false">${icon(i)}<span>${t}</span><em>${c}</em></a>`).join('')}
        <div class="gap"></div><a href="#" onclick="return false">${icon('grid')}<span>Setup</span><em class="upd">1</em></a>
        ${NAV2.map(([t, c]) => `<a class="sub" href="#" onclick="return false"><span>${t}</span><em>${c}</em></a>`).join('')}</nav>
      <nav class="nav bottom"><a href="#" onclick="return false">${icon('book')}<span>CLI reference</span></a><a href="#" onclick="return false">${icon('gear')}<span>Settings</span></a><span class="ver">1.6.1</span></nav></aside>
    <header class="top"><label class="find">${icon('search', 15)}<input placeholder="Find an account, session or setting" aria-label="Find"><kbd>Ctrl K</kbd></label>${extraTop}
      <div class="right"><span class="pcs">${computers.map((c) => `<i style="background:${c.col}" title="${c.name}"></i>`).join('')}<span>3 computers</span></span><span class="fresh">limits read ${Math.round((NOW - MEASURED) / 1e3)} s ago</span><button class="ib" aria-label="Refresh">${icon('refresh', 15)}</button></div></header>
    <main class="sheet">${main}</main>`;

  // ---------- the screen simulator ----------
  // logical sizes: what Windows gives WebView2 at each display scale (physical / scale)
  const SCREENS = [
    ['1920x1080 at 100%', 1920, 1080], ['1920x1080 at 125%', 1536, 864], ['1920x1080 at 150%', 1280, 720],
    ['2560x1440 at 150%', 1707, 960], ['3840x2160 at 200%', 1920, 1080], ['1366x768 at 100%', 1366, 768],
  ];
  const variations = {};
  function mount(sec, key) {
    const v = variations[key];
    const st = { theme: sec.dataset.theme || 'dark', screen: 1, motion: !reduced() };
    const stage = sec.querySelector('.stage');
    const screen = h('<div class="screen"></div>');
    const dims = h('<span class="dims"></span>');
    stage.append(screen, dims);
    let inst = null;
    const draw = () => {
      if (inst && inst.destroy) inst.destroy();
      const [name, W, Hh] = SCREENS[st.screen];
      screen.style.width = W + 'px'; screen.style.height = Hh + 'px';
      stage.style.aspectRatio = W + ' / ' + Hh;
      dims.textContent = `${name} → laid out at ${W} × ${Hh}`;
      screen.innerHTML = '';
      const app = h(`<div class="app ${v.cls}" data-theme="${st.theme}"></div>`);
      screen.append(app);
      fit();
      inst = v.render(app, { motion: st.motion, theme: st.theme, W, H: Hh });
    };
    const fit = () => { const W = SCREENS[st.screen][1], k = stage.clientWidth / W; screen.style.transform = `scale(${k})`; screen.dataset.k = k; };
    new ResizeObserver(fit).observe(stage);
    sec.querySelectorAll('[data-theme-btn]').forEach((b) => b.addEventListener('click', () => { st.theme = b.dataset.themeBtn; sec.querySelectorAll('[data-theme-btn]').forEach((x) => x.classList.toggle('on', x === b)); draw(); }));
    sec.querySelectorAll('[data-motion-btn]').forEach((b) => b.addEventListener('click', () => { st.motion = b.dataset.motionBtn === 'on'; sec.querySelectorAll('[data-motion-btn]').forEach((x) => x.classList.toggle('on', x === b)); draw(); }));
    const sel = sec.querySelector('select.scr');
    sel.innerHTML = SCREENS.map(([n], i) => `<option value="${i}">${n}</option>`).join('');
    sel.value = st.screen;
    sel.addEventListener('change', () => { st.screen = +sel.value; draw(); });
    sec.querySelector('[data-full]').addEventListener('click', () => (stage.requestFullscreen ? stage.requestFullscreen() : null));
    document.addEventListener('fullscreenchange', fit);
    if (!st.motion) sec.querySelectorAll('[data-motion-btn]').forEach((x) => x.classList.toggle('on', x.dataset.motionBtn === 'off'));
    draw();
  }
  // a click inside the scaled screen, in the app's own (unscaled) pixels
  const local = (el, e) => { const r = el.getBoundingClientRect(), k = r.width / el.offsetWidth || 1; return [(e.clientX - r.left) / k, (e.clientY - r.top) / k]; };

  // ---------- feedback, kept in this browser ----------
  const KEY = 'hm-overview-redesign-2026-11';
  function feedback() {
    let draft = {};
    try { draft = JSON.parse(localStorage.getItem(KEY) || '{}'); } catch (e) { draft = {}; }
    const saved = document.getElementById('saved');
    const save = () => { try { localStorage.setItem(KEY, JSON.stringify(draft)); } catch (e) {} if (saved) saved.textContent = 'Saved in this browser ' + new Date().toLocaleTimeString(); };
    document.querySelectorAll('[data-k]').forEach((el) => {
      const k = el.dataset.k;
      if (el.type === 'radio') { el.checked = draft[k] === el.value; el.addEventListener('change', () => { draft[k] = el.value; save(); }); }
      else if (el.type === 'checkbox') { el.checked = !!draft[k + ':' + el.value]; el.addEventListener('change', () => { draft[k + ':' + el.value] = el.checked; save(); }); }
      else { el.value = draft[k] || ''; el.addEventListener('input', () => { draft[k] = el.value; save(); }); }
    });
    const text = () => {
      const out = ['# Overview redesign: my answers', ''];
      for (const [key, name] of [['A', 'A, Blueprint'], ['B', 'B, Orbit'], ['C', 'C, Runway']]) {
        out.push(`## ${name}`, `Verdict: ${draft['v' + key] || '(none)'}`);
        const parts = [...document.querySelectorAll(`[data-k="p${key}"]`)].filter((x) => x.checked).map((x) => x.value);
        out.push('Keep: ' + (parts.join('; ') || '(none)'));
        if (draft['c' + key]) out.push('Comments: ' + draft['c' + key]);
        out.push('');
      }
      out.push('## Direction for every page', `Pick: ${draft.pick || '(none)'}`, `Theme to lead with: ${draft.lead || '(none)'}`);
      if (draft.foundation) out.push('On the foundation: ' + draft.foundation);
      if (draft.general) out.push('Anything else: ' + draft.general);
      return out.join('\n');
    };
    const box = document.getElementById('out');
    document.getElementById('copy').addEventListener('click', async () => {
      box.style.display = 'block'; box.value = text(); box.select();
      try { await navigator.clipboard.writeText(box.value); saved.textContent = 'Copied. Paste it into the chat with Claude.'; } catch (e) { document.execCommand('copy'); saved.textContent = 'Copied (or select the text below and copy it).'; }
    });
    document.getElementById('dl').addEventListener('click', () => {
      const a = document.createElement('a');
      a.href = URL.createObjectURL(new Blob([text()], { type: 'text/markdown' }));
      a.download = 'overview-redesign-answers.md'; a.click();
    });
    document.getElementById('clear').addEventListener('click', () => { if (!window.confirm('Clear every answer on this page?')) return; draft = {}; save(); location.reload(); });
  }

  window.HM = { NOW, MEASURED, H, DAY, computers, tools, accounts, providers, router, edges, running, kpi, series, days, replies,
    fmt, money, dur, at, clock, esc, h, css, rgb, pct, reduced, label, pcName, pcCol, toolLabel, burn, icon, shell, local, rng, variations, mount, feedback };
})();
