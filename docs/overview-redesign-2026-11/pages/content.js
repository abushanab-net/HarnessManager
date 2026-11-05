// Every page but the Overview, drawn in its direction: the finding leads each page, the body is quiet until something
// needs attention, numbers say whether they are measured (plain) or estimated (≈), every harness has its own icon.
// The data is invented in the formats the app uses (see the inventory); nothing is read from the app.
(function () {
  const { NOW, H, DAY, computers, accounts, tools, running, kpi, fmt, money, dur, esc, pcName, pcCol, rng } = HM;
  const toolId = (label) => (tools.find((t) => t.label === label) || { id: 'claude' }).id;
  const toolPill = (id, label) => `<span class="tool">${ticon(id, 13)}${esc(label || (tools.find((t) => t.id === id) || {}).label || id)}</span>`;
  const at = (ms) => { const d = new Date(ms), t = d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' }), days = Math.floor((new Date(NOW).setHours(0, 0, 0, 0) - new Date(ms).setHours(0, 0, 0, 0)) / DAY); return days === 0 ? 'Today ' + t : days === 1 ? 'Yesterday ' + t : d.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' }) + ', ' + t; };
  const pcDots = (ids) => `<span class="pc">${ids.map((id) => `<i style="background:${pcCol(id)}" title="${pcName(id)}"></i>`).join('')}</span>`;
  const seg = (items, on) => `<span class="seg">${items.map((t) => `<a class="${t === on ? 'on' : ''}">${t}</a>`).join('')}</span>`;
  const pill = (t, k = '') => `<span class="pill ${k}">${t}</span>`;
  const btn = (t, k = '') => `<button class="btn ${k}">${t}</button>`;
  const head = (crumb, h1, basis, act = '') => `<div class="head"><div><div class="crumb">${crumb}</div><h1>${h1}</h1><div class="basis">${basis}</div></div><div class="act">${act}</div></div>`;
  const tile = (label, v, note, k = '') => `<div class="tile"><label>${label}</label><strong class="${k}">${v}</strong><div class="note">${note}</div></div>`;
  const ph = (t, sub = '', right = '') => `<div class="ph"><h2>${t}</h2>${sub ? `<span>${sub}</span>` : ''}${right ? `<span class="r">${right}</span>` : ''}</div>`;
  const arc = (cx, cy, r, f0, f1) => { const a = (f) => f * 2 * Math.PI - Math.PI / 2, x = (f) => cx + r * Math.cos(a(f)), y = (f) => cy + r * Math.sin(a(f)); if (f1 - f0 >= 0.9999) return `M${x(0)} ${y(0)} A${r} ${r} 0 1 1 ${x(0.5)} ${y(0.5)} A${r} ${r} 0 1 1 ${x(0)} ${y(0)}`; return `M${x(f0)} ${y(f0)} A${r} ${r} 0 ${f1 - f0 > 0.5 ? 1 : 0} 1 ${x(f1)} ${y(f1)}`; };
  const tone = (u) => (u >= 80 ? 'var(--neg)' : u >= 55 ? 'var(--warn)' : 'var(--ac)');
  function dots(vals, color, cols = 14, rows = 3) {
    const max = Math.max(...vals, 1), w = cols * 6, hh = rows * 6;
    let s = `<svg width="${w}" height="${hh}" viewBox="0 0 ${w} ${hh}" aria-hidden="true">`;
    vals.slice(-cols).forEach((v, c) => { const lit = v ? Math.max(1, Math.round((v / max) * rows)) : 0; for (let r = 0; r < rows; r++) s += `<circle cx="${c * 6 + 3}" cy="${hh - r * 6 - 3}" r="1.7" fill="${r < lit ? color : 'var(--line)'}"/>`; });
    return s + '</svg>';
  }
  /** An account's limits as the Overview draws them: inner 5-hour, outer weekly, dashed the estimate at the reset. */
  function ring(a, S = 72) {
    const c = S / 2, r1 = c - 8, r0 = c - 17; let g = '', mid = '';
    if (a.needsLogin) { g = `<circle cx="${c}" cy="${c}" r="${r1}" fill="none" stroke="var(--neg)" stroke-dasharray="3 5" opacity=".7"/>`; mid = '—'; }
    else if (a.cap) { const u = a.cap.spent / a.cap.cap; g = `<circle cx="${c}" cy="${c}" r="${r1}" fill="none" stroke="var(--line)" stroke-width="6"/><path d="${arc(c, c, r1, 0, u)}" fill="none" stroke="var(--r2)" stroke-width="6" stroke-linecap="round"/>`; mid = Math.round((1 - u) * 100) + '%'; }
    else if (!a.windows) { g = `<circle cx="${c}" cy="${c}" r="${r1}" fill="none" stroke="var(--line)" stroke-dasharray="2 4"/>`; mid = '∞'; }
    else { a.windows.forEach((w, i) => { const r = i ? r1 : r0, u = w.used / 100, f = Math.min(1, (w.at || w.used) / 100); g += `<circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="var(--line)" stroke-width="6"/><path d="${arc(c, c, r, u, f)}" fill="none" stroke="${tone(w.at)}" stroke-width="6" stroke-dasharray="2 3" opacity=".55"/><path d="${arc(c, c, r, 0, u)}" fill="none" stroke="${tone(w.used)}" stroke-width="6" stroke-linecap="round"/>`; }); mid = 100 - a.windows[1].used + '%'; }
    return `<svg width="${S}" height="${S}" viewBox="0 0 ${S} ${S}" aria-hidden="true">${g}<text x="${c}" y="${c + 5}" text-anchor="middle" font-size="13" font-weight="600" fill="var(--ink)" font-family="var(--sans)">${mid}</text></svg>`;
  }
  const r = rng(11);
  const series = (n, base) => Array.from({ length: n }, () => Math.round(base * (0.3 + r() * 1.4)));

  // ---------- the data ----------
  const projects = [
    { name: 'HarnessManager', remote: 'github.com/abushanab-net/HarnessManager', cost: 84.2, tokens: 12.4e6, sessions: 31, accounts: 3, live: 1, last: NOW - 3 * 60e3, on: ['studio', 'lab'], tags: ['ui'], pinned: true },
    { name: 'sanduq-api', remote: 'github.com/abushanab-net/sanduq-api', cost: 41.6, tokens: 7.1e6, sessions: 14, accounts: 2, live: 1, last: NOW - 34 * 60e3, on: ['lab'], tags: ['api'], pinned: true },
    { name: 'resal-wiki', path: 'D:\\Projects\\resal-wiki', cost: 22.15, tokens: 3.8e6, sessions: 9, accounts: 1, live: 1, last: NOW - 8 * 60e3, on: ['studio'], priced: 72 },
    { name: 'dgx-serve', remote: 'github.com/abushanab-net/dgx-serve', cost: 19.4, tokens: 3.1e6, sessions: 11, accounts: 1, live: 1, last: NOW - 123 * 60e3, on: ['surface'], tags: ['infra'] },
    { name: 'model-adoption', path: 'D:\\Projects\\model-adoption', cost: 12.7, tokens: 2.2e6, sessions: 6, accounts: 2, live: 1, last: NOW - 19 * 60e3, on: ['lab', 'studio'] },
    { name: 'skills-hub', path: 'D:\\Projects\\skills-hub', cost: 9.3, tokens: 1.6e6, sessions: 7, accounts: 1, live: 0, last: NOW - DAY + 4 * H, on: ['studio'] },
    { name: 'router-bench', remote: 'github.com/abushanab-net/router-bench', cost: null, tokens: 640e3, sessions: 4, accounts: 1, live: 0, last: NOW - 2 * DAY, on: ['lab'], notHere: true },
    { name: 'scratch', path: 'C:\\Users\\sabus\\tmp\\scratch', cost: 0.4, tokens: 48e3, sessions: 3, accounts: 1, live: 0, last: NOW - 6 * DAY, on: ['studio'] },
  ].map((p) => ({ ...p, daily: series(14, p.tokens / 9) }));
  const sessions = [
    { title: 'Sessions page rail redesign', project: 'HarnessManager', msgs: 42, account: 'Claude Max', tool: 'claude', device: 'studio', tokens: 3.12e6, when: NOW - 72 * 60e3, open: true, tags: ['ui'] },
    { title: 'Breaker retry after 3 errors', project: 'sanduq-api', msgs: 18, account: 'Codex Pro', tool: 'codex', device: 'lab', tokens: 864e3, when: NOW - 34 * 60e3, open: true, failed: 2 },
    { title: 'Wiki search index', project: 'resal-wiki', msgs: 9, account: 'Claude Team', tool: 'claude', device: 'studio', tokens: 212e3, when: NOW - 8 * 60e3, open: true },
    { title: 'Serve two models on one GPU', project: 'dgx-serve', msgs: 61, account: 'OpenCode routed', tool: 'opencode', device: 'surface', tokens: 1.9e6, when: NOW - 123 * 60e3, open: true },
    { title: 'Adoption report for Qwen 4', project: 'model-adoption', msgs: 14, account: 'Claude Max', tool: 'claude', device: 'lab', tokens: 640e3, when: NOW - 19 * 60e3, open: true },
    { title: 'Release 1.6.2 notes', project: 'HarnessManager', msgs: 12, account: 'Claude Max', tool: 'claude', device: 'studio', tokens: 412e3, when: NOW - 5 * H },
    { title: 'Journal rows without a computer', project: 'HarnessManager', msgs: 27, account: 'Codex Pro', tool: 'codex', device: 'lab', tokens: 1.1e6, when: NOW - 7 * H, cont: 'studio' },
    { title: 'Backups test retries', project: 'HarnessManager', msgs: 8, account: 'Claude Team', tool: 'claude', device: 'studio', tokens: 180e3, when: NOW - 9 * H, failed: 3 },
    { title: 'Rate limit headers', project: 'sanduq-api', msgs: 22, account: 'Codex Pro', tool: 'codex', device: 'lab', tokens: 720e3, when: NOW - DAY + 2 * H, notHere: true },
    { title: 'Pi on OpenRouter smoke test', project: 'skills-hub', msgs: 5, account: 'Pi on OpenRouter', tool: 'pi', device: 'studio', tokens: 36e3, when: NOW - DAY + H },
    { title: 'Untitled session', project: 'scratch', msgs: 3, account: 'OpenCode routed', tool: 'opencode', device: 'surface', tokens: 4.1e3, when: NOW - 6 * DAY, archived: true },
  ];
  const acctX = {
    'claude-max': { email: 'sami@abushanab.net', main: true, added: 'Added 3 Nov', tokens7: 18.4e6, cost7: 142.6, read: '2 m ago', open: 2, note: ['ok', 'Most room of any account: start here next.'] },
    'codex-pro': { email: 'sami@abushanab.net', added: 'Added 3 Nov', tokens7: 9.3e6, cost7: 61.4, read: '38 s ago', open: 1, note: ['warn', 'Week runs out Sat 14:08 at this pace.'] },
    'claude-team': { email: 'sami@team.example', added: 'Added 4 Nov', tokens7: 6.12e6, cost7: 48.9, read: '9 m ago', open: 1 },
    'opencode-routed': { email: 'route cheap-fast', added: 'Added 4 Nov', tokens7: 980e3, cost7: 1.8, read: '—' },
    'pi-openrouter': { email: 'openrouter.ai/api/v1', added: 'Added 5 Nov', tokens7: 120e3, cost7: 0.31, read: '—' },
    antigravity: { email: 'sami@abushanab.net', added: 'Added 3 Nov', tokens7: 0, cost7: 0, read: '2 d ago' },
  };
  const mcp = [
    { name: 'github', tr: 'http', line: 'https://api.githubcopilot.com/mcp/', on: ['claude', 'codex', 'opencode'], desc: 'Issues, pull requests and code search' },
    { name: 'playwright', tr: 'stdio', line: 'npx @playwright/mcp@latest', on: ['claude', 'opencode'] },
    { name: 'codegraph', tr: 'stdio', line: 'codegraph mcp', on: ['claude', 'codex', 'opencode', 'agy'], desc: 'The code graph of the open project' },
    { name: 'context7', tr: 'http', line: 'https://mcp.context7.com/mcp', on: [], off: true },
    { name: 'figma', tr: 'http', line: 'https://mcp.figma.com/mcp', on: ['claude'] },
    { name: 'blender', tr: 'stdio', line: 'uvx blender-mcp', on: ['claude', 'codex'], alone: 'codex' },
    { name: 'comfyui', tr: 'stdio', line: 'python -m comfyui_mcp', on: ['claude'] },
    { name: 'voicestudio', tr: 'sse', line: 'http://127.0.0.1:7860/sse', on: ['claude', 'opencode'], alone: 'opencode' },
  ];
  const skills = [
    { name: 'pdf', src: 'anthropics/skills', on: ['claude', 'codex'], upd: true, desc: 'Read, fill and make PDF files' },
    { name: 'frontend-design', src: 'anthropics/skills', on: ['claude', 'codex', 'opencode', 'pi'], desc: 'Distinctive, intentional visual design for new UI' },
    { name: 'graphify', src: 'Local', on: ['claude'], desc: 'Any input to a knowledge graph' },
    { name: 'humanizer', src: 'Imported', on: ['claude'], desc: 'Rewrite AI-sounding text so it reads like the writer' },
    { name: 'docx', src: 'anthropics/skills', on: ['claude', 'codex'], upd: true, desc: 'Word documents: create, read, edit' },
    { name: 'illustrate', src: 'Store', on: ['claude', 'opencode'], desc: 'Diagrams and charts as self-contained HTML' },
    { name: 'ponytail', src: 'Local', on: ['claude', 'codex', 'pi'], desc: 'The smallest change that fully solves the task' },
    { name: 'user-manual', src: 'Store', on: ['claude'], desc: 'A manual from the app as it is' },
  ];
  const prompts = [
    { title: 'Default house rules', first: 'Write like a person. Short sentences, plain words, no filler.', on: true },
    { title: 'Terse reviewer', first: 'Review only, never edit. Name the line and the risk.' },
    { title: 'Release week', first: 'Freeze: no schema changes, no new dependencies.' },
    { title: 'Arabic first', first: 'Answer in Arabic unless the code comments are English.' },
  ];
  const providers = [
    { id: 'openrouter', name: 'OpenRouter', fmt: 'openai', url: 'https://openrouter.ai/api/v1', key: true, breaker: 'closed', req: 612, p95: 1.4 },
    { id: 'deepseek', name: 'DeepSeek', fmt: 'openai', url: 'https://api.deepseek.com/v1', key: true, breaker: 'closed', req: 540, p95: 0.9 },
    { id: 'groq', name: 'Groq', fmt: 'openai', url: 'https://api.groq.com/openai/v1', key: true, breaker: 'open · 3 failures', req: 118, p95: 0.4, paused: true },
    { id: 'google', name: 'Google', fmt: 'openai', url: 'https://generativelanguage.googleapis.com/v1beta/openai', key: false, breaker: 'closed', req: 14, p95: 2.1 },
  ];
  const reqs = [
    { t: '15:41:52', acct: 'OpenCode routed', route: 'cheap-fast → DeepSeek', model: 'deepseek-v4', st: 200, first: 0.9, time: 6.2, tok: 18.4e3 },
    { t: '15:41:10', acct: 'Pi on OpenRouter', route: 'openrouter → OpenRouter', model: 'kimi-k3', st: 200, first: 1.2, time: 9.8, tok: 42.1e3 },
    { t: '15:38:03', acct: 'OpenCode routed', route: 'cheap-fast → Groq', model: 'llama-4-maverick', st: 429, first: null, time: 0.3, tok: null },
    { t: '15:37:58', acct: 'OpenCode routed', route: 'cheap-fast → Groq', model: 'llama-4-maverick', st: 429, first: null, time: 0.3, tok: null },
    { t: '15:30:21', acct: 'OpenCode routed', route: 'cheap-fast → DeepSeek', model: 'deepseek-v4', st: 200, first: 0.7, time: 4.1, tok: 9.6e3 },
    { t: '15:21:40', acct: 'Pi on OpenRouter', route: 'openrouter → OpenRouter', model: 'kimi-k3', st: 'failed', first: null, time: 30.0, tok: null },
  ];
  const apps = [
    { id: 'claude', label: 'Claude Code', path: 'C:\\Users\\sabus\\.local\\bin\\claude.exe', ver: '2.4.1', newest: '2.4.1', by: 'native · updates itself' },
    { id: 'codex', label: 'Codex', path: 'C:\\Users\\sabus\\AppData\\Roaming\\npm\\codex.cmd', ver: '0.98.0', newest: '0.99.1', by: 'npm', dup: { path: 'C:\\Users\\sabus\\AppData\\Local\\Microsoft\\WinGet\\Links\\codex.exe', ver: '0.95.0', by: 'winget' } },
    { id: 'opencode', label: 'OpenCode', path: 'C:\\Users\\sabus\\scoop\\shims\\opencode.exe', ver: '1.3.2', newest: null, by: 'Scoop' },
    { id: 'pi', label: 'Pi', path: 'C:\\Users\\sabus\\AppData\\Roaming\\npm\\pi.cmd', ver: '0.41.0', newest: '0.41.0', by: 'npm' },
    { id: 'omp', label: 'Oh My Pi', ver: null, newest: '0.9.2', by: null },
    { id: 'agy', label: 'Antigravity', path: 'C:\\Users\\sabus\\AppData\\Local\\Programs\\Antigravity\\Antigravity.exe', ver: '1.8.0', newest: '1.8.0', by: 'native · updates itself' },
  ];

  // ---------- the pages ----------
  const P = window.P;
  const sessionRow = (s, pc = true) => `<tr class="${s.open ? 'live' : ''}"><td><span class="dot ${s.open ? 'on' : s.failed ? 'neg' : ''}" title="${s.failed ? s.failed + ' failed commands' : ''}"></span></td>
      <td><b>${esc(s.title)}</b>${s.archived ? ' ' + pill('archived') : ''}<small>${s.cont ? 'continued from ' + pcName(s.cont) + ' · ' : ''}${s.msgs} msgs · ${esc(s.project)}${(s.tags || []).map((t) => ` <span class="tag a">${t}</span>`).join('')}</small></td>
      <td>${toolPill(s.tool, s.account)}</td>${pc ? `<td><span class="chip"><i style="background:${pcCol(s.device)}"></i>${pcName(s.device)}</span></td>` : ''}<td class="n">${fmt(s.tokens)}</td>
      <td class="tm">${s.open ? (s.device === 'studio' ? '<span style="color:var(--ac)">live</span>' : 'running on ' + pcName(s.device)) : at(s.when)}</td>
      <td class="act">${s.archived ? '' : s.open ? btn('Follow', 'sm') : s.notHere ? '<span class="m xs">not cloned here</span>' : s.device !== 'studio' ? btn('Resume here', 'sm') : btn('Resume', 'sm')}</td></tr>`;
  const sessionTable = (rows, pc = true) => `<table class="t"><thead><tr><th></th><th>Session</th><th>Account</th>${pc ? '<th>Computer</th>' : ''}<th class="n">Tokens</th><th>When</th><th></th></tr></thead><tbody>${rows.map((s) => sessionRow(s, pc)).join('')}</tbody></table>`;

  /** Today as a ribbon: a lane per computer, a session a bar, the open ones reach the now line. */
  function ribbon(W) {
    const w = Math.max(400, W - 420), lanes = computers.length, lh = 14, x = (t) => ((t - (NOW - 16 * H)) / (16 * H)) * w;
    let s = `<svg viewBox="0 0 ${w} ${lanes * lh + 12}" preserveAspectRatio="none">`;
    computers.forEach((c, i) => { s += `<line x1="0" y1="${i * lh + 7}" x2="${w}" y2="${i * lh + 7}" stroke="var(--line)" stroke-dasharray="1 4"/>`; });
    for (const ss of sessions.filter((q) => q.when > NOW - 16 * H)) {
      const i = computers.findIndex((c) => c.id === ss.device), x0 = x(ss.when - ss.msgs * 2.5 * 60e3), x1 = ss.open ? x(NOW) : x(ss.when);
      s += `<rect x="${Math.max(0, x0).toFixed(1)}" y="${i * lh + 3}" width="${Math.max(3, x1 - Math.max(0, x0)).toFixed(1)}" height="8" rx="4" fill="${pcCol(ss.device)}" opacity="${ss.open ? 1 : 0.45}"><title>${esc(ss.title)}</title></rect>`;
      if (ss.open) s += `<circle cx="${x(NOW).toFixed(1)}" cy="${i * lh + 7}" r="4" fill="${pcCol(ss.device)}" stroke="var(--s)" stroke-width="1.5"/>`;
    }
    for (let hh = 0; hh <= 16; hh += 4) s += `<text x="${x(NOW - (16 - hh) * H).toFixed(1)}" y="${lanes * lh + 11}" font-size="9" fill="var(--m)" font-family="var(--mono)" text-anchor="middle">${new Date(NOW - (16 - hh) * H).getHours()}:00</text>`;
    s += `<line x1="${x(NOW).toFixed(1)}" y1="0" x2="${x(NOW).toFixed(1)}" y2="${lanes * lh}" stroke="var(--ac)" stroke-width="1"/>`;
    return `<div class="ribbon" style="max-width:${w}px">${s}</svg></div>`;
  }

  P.pages.sessions = { find: 'Find a session', html: (W) => {
    const live = sessions.filter((s) => s.open), rest = sessions.filter((s) => !s.open);
    const groups = {}; for (const s of rest) (groups[s.project] = groups[s.project] || []).push(s);
    return head('Sessions · 7 days · 3 computers',
      `<span class="w">${live.length} sessions</span> are running on 3 computers; the one on Lab has had ${live[1].failed} failed commands.`,
      `${sessions.length} sessions in 7 days across ${Object.keys(groups).length + 1} projects. Tokens are counted from the logs; costs are at list prices.`,
      btn('▶ New session', 'pri'))
    + ribbon(W)
    + `<div class="bar">${seg(['Project', 'Date', 'Account', 'Tool', 'Tag'], 'Project')}${seg(['7 d', '30 d', 'All'], '7 d')}${seg(['All computers', 'This computer', 'Studio', 'Lab', 'Surface'], 'All computers')}<span class="r">Search above narrows this list</span></div>`
    + `<div class="cols rail"><div class="railc pn" style="padding:var(--s2)"><a class="on"><span class="dot on"></span>Live now<em>${live.length}</em></a><a>All projects<em>${sessions.length}</em></a><h3>Pinned</h3>${projects.filter((p) => p.pinned).map((p) => `<a><span class="dot ${p.live ? 'on' : ''}"></span>${p.name}<em>${p.live ? p.live + ' live' : p.sessions}</em></a>`).join('')}<h3>Recent</h3>${projects.filter((p) => !p.pinned).slice(0, 5).map((p) => `<a><span class="dot ${p.live ? 'on' : ''}"></span>${p.name}<em>${p.live ? p.live + ' live' : p.sessions}</em></a>`).join('')}<h3>Tags</h3><a><span class="tag a">ui</span><em>3</em></a><a><span class="tag b">api</span><em>2</em></a><a><span class="tag c">infra</span><em>1</em></a></div>
      <div><div class="grp live"><div class="gh"><b>Running now</b><span>${live.length} sessions · ${fmt(live.reduce((s, q) => s + q.tokens, 0))} tokens</span></div>${sessionTable(live)}</div>
      ${Object.entries(groups).map(([p, rows]) => { const pr = projects.find((q) => q.name === p) || {}; return `<div class="grp"><div class="gh"><b>${esc(p)}</b><span>${esc(pr.remote || pr.path || '')}</span><span class="r">${rows.length} sessions · ${fmt(rows.reduce((s, q) => s + q.tokens, 0))} tokens · ≈ ${money(rows.reduce((s, q) => s + q.tokens, 0) / 1e6 * 6.8)} ${btn('▶ New session', 'sm')}</span></div>${sessionTable(rows)}</div>`; }).join('')}</div></div>`;
  } };

  P.pages.session = { nav: 'sessions', find: 'Find a session', html: () => {
    const s = sessions[1];
    const steps = [['ok', 'Read', 'crates/router/src/lib.rs', '120 ms'], ['ok', 'Edit', 'crates/router/src/breaker.rs', '+18 −6'], ['bad', 'Bash', 'cargo test -p router breaker', 'failed · 31.0 s'], ['ok', 'Edit', 'crates/router/src/breaker.rs', '+4 −1'], ['run', 'Bash', 'cargo test -p router breaker', 'running now']];
    return head('<a>← Sessions</a> · sanduq-api', esc(s.title),
      `${pill('● LIVE · following', 'ok mono')} ${toolPill(s.tool, s.account)} · <span class="mono">gpt-6.1-sol</span> · running on ${pcName(s.device)} · started ${at(NOW - 34 * 60e3).toLowerCase()} · 12 turns · <span class="mono">864.2k tokens · ≈ ${money(5.9)}</span>`,
      btn('Export as Markdown') + btn('Resume here', 'pri') + btn('⋯'))
    + `<div class="tiles">${tile('Tokens', fmt(s.tokens), 'measured from the log')}${tile('Read from cache', '71<small>%</small>', 'measured')}${tile('First reply p50', '≈ 1.8 s', 'from the log\'s times')}${tile('Failed commands', '2', 'cargo test, twice', 'neg')}${tile('Files changed', '3', '+22 −7')}</div>
      <section class="pn">${ph('Conversation', '12 turns · 48 steps · 3 changed files · 2 failed', seg(['All', 'Changes', 'Failed'], 'All'))}
      <div class="chat">
        <div class="msg me"><span class="who">YOU · 15:08</span>The breaker opens after 3 errors but never retries. Make it half-open after the retry delay and close on the first 200.</div>
        <div class="msg ai"><span class="who">CODEX · 15:09 · gpt-6.1-sol</span>The breaker in <code>breaker.rs</code> has no half-open state; it stays open until the process restarts. I'll add one with a retry deadline, let one probe request through, and close on success.</div>
        ${steps.map(([k, t, i, m]) => `<div class="step"><i class="${k}">${k === 'ok' ? '✓' : k === 'bad' ? '✗' : '…'}</i><span>${t}</span><span class="in">${i}</span><span class="meta ${k === 'bad' ? 'bad' : ''}">${m}</span></div>`).join('')}
        <div class="msg ai"><span class="who">CODEX · 15:40 · gpt-6.1-sol</span>The first run failed on the retry deadline being in seconds while the test expects milliseconds. Fixed the unit; running the test again.</div>
      </div></section>`;
  } };

  P.pages.projects = { find: 'Find a project', html: () => {
    const total = projects.reduce((s, p) => s + p.tokens, 0), cols = ['var(--ac)', 'var(--r2)', 'var(--warn)', '#6FA8F5', '#E384B8'];
    const top = projects.slice(0, 5);
    return head('Projects · 7 days',
      `<span class="w">HarnessManager</span> took ${Math.round(projects[0].tokens / total * 100)}% of this week's tokens; ${projects.length} projects ran, ${projects.filter((p) => p.live).length} have a session open now.`,
      'A project is a folder your tools ran in. Click one for its sessions; pin, tag or rename it from its icons. Cost is at API list prices, so ≈.',
      seg(['7 d', '30 d', 'All'], '7 d') + seg(['Active', 'Cost', 'Tokens', 'Name'], 'Active'))
    + `<div class="tiles">${tile('Tokens · 7 days', fmt(total), `${projects.reduce((s, p) => s + p.sessions, 0)} sessions`)}${tile('At list prices', '≈ ' + money(projects.reduce((s, p) => s + (p.cost || 0), 0), 0), '≈ $27 a day')}${tile('Projects', projects.length, '5 accounts')}${tile('Running now', projects.filter((p) => p.live).length, 'HarnessManager, sanduq-api, +3', 'ac')}${tile('Busiest', 'HarnessManager', '41% of tokens · 44% of cost')}</div>
      <section class="pn">${ph('Where the tokens and the cost went', '', seg(['Tokens', 'Cost', 'Sessions'], 'Tokens'))}
        <div style="display:flex;height:10px;border-radius:5px;overflow:hidden;gap:2px;margin-bottom:var(--s3)">${top.map((p, i) => `<i style="flex:${p.tokens};background:${cols[i]}"></i>`).join('')}<i style="flex:${total - top.reduce((s, p) => s + p.tokens, 0)};background:var(--line)"></i></div>
        <table class="t"><thead><tr><th></th><th>Project</th><th style="width:30%">Share of tokens</th><th class="n">≈ Cost</th><th class="n">Tokens</th><th class="n">Sessions</th><th class="n">Share</th></tr></thead><tbody>
        ${top.map((p, i) => `<tr><td><i class="dot" style="background:${cols[i]};opacity:1"></i></td><td><a class="lnk">${p.name}</a></td><td><span class="track"><i style="width:${p.tokens / total * 100}%;background:${cols[i]}"></i></span></td><td class="n">${p.cost == null ? '—' : money(p.cost)}</td><td class="n">${fmt(p.tokens)}</td><td class="n">${p.sessions}</td><td class="n">${Math.round(p.tokens / total * 100)}%</td></tr>`).join('')}
        <tr><td></td><td class="m">${projects.length - 5} other projects</td><td><span class="track"><i style="width:${(total - top.reduce((s, p) => s + p.tokens, 0)) / total * 100}%;background:var(--m)"></i></span></td><td class="n">${money(projects.slice(5).reduce((s, p) => s + (p.cost || 0), 0))}</td><td class="n">${fmt(total - top.reduce((s, p) => s + p.tokens, 0))}</td><td class="n">${projects.slice(5).reduce((s, p) => s + p.sessions, 0)}</td><td class="n">${Math.round((total - top.reduce((s, p) => s + p.tokens, 0)) / total * 100)}%</td></tr></tbody></table></section>
      <div class="stack" style="margin-top:var(--s3)">${projects.map((p) => `<div class="grp ${p.live ? 'live' : ''}" style="display:grid;grid-template-columns:minmax(12rem,1.2fr) minmax(0,2fr) 9rem 11rem auto;gap:var(--s4);align-items:center">
        <div><b>${p.name}</b>${p.pinned ? ' <span class="m xs">pinned</span>' : ''}<br><span class="mono xs m">${esc(p.remote || p.path)}</span><br>${(p.tags || []).map((t) => `<span class="tag a">${t}</span>`).join('')}<a class="lnk xs">+ tag</a></div>
        <div>${dots(p.daily, 'var(--ac)', 14, 3)}</div>
        <div><b>${p.cost == null ? '—' : '≈ ' + money(p.cost)}</b><br><span class="mono xs m">${fmt(p.tokens)} tokens${p.priced ? ' · ' + p.priced + '% priced' : ''}</span></div>
        <div class="xs m">${p.sessions} sessions · ${p.accounts} account${p.accounts > 1 ? 's' : ''}<br>${p.live ? `<span style="color:var(--ac)">${p.live} live</span>` : at(p.last)}<br>${pcDots(p.on)}${p.notHere ? ' <span class="xs">not cloned here</span>' : ''}</div>
        <div class="act" style="display:flex;gap:4px">${btn('▶', 'sm')}${btn('⋯', 'sm')}</div></div>`).join('')}</div>
      <div class="bar" style="margin-top:var(--s3)">${btn('‹', 'sm ghost')} ${btn('1', 'sm pri')} ${btn('2', 'sm ghost')} ${btn('›', 'sm ghost')}<span>1–10 of 18</span><span class="r">${seg(['10', '25', '50'], '10')} a page</span></div>`;
  } };

  P.pages.accounts = { html: () => {
    const byTool = {}; for (const a of accounts) (byTool[a.tool] = byTool[a.tool] || []).push(a);
    const planPill = (a) => a.needsLogin ? pill('Login expired', 'neg') : a.cap ? pill('Routed · ' + a.route, 'r2') : !a.windows ? pill('API key', 'warn') : pill(a.plan, 'ok');
    const lim = (a) => {
      if (a.needsLogin) return '<span class="xs m">Log in to see its plan and limits.</span>';
      if (a.cap) return `<div class="lim"><span>$${a.cap.cap} cap</span><span class="track"><i class="r2" style="width:${a.cap.spent / a.cap.cap * 100}%"></i></span><b>${Math.round((1 - a.cap.spent / a.cap.cap) * 100)}% left</b><span>↻ ${dur(8.3 * H)}</span></div>`;
      if (!a.windows) return '<span class="xs m">Pay per use · no plan limits</span>';
      return `<div class="lim">${a.windows.map((w) => `<span>${w.label}</span><span class="track"><i class="${w.used >= 90 ? 'neg' : w.used >= 75 ? 'warn' : ''}" style="width:${100 - w.used}%"></i></span><b class="${w.used >= 90 ? 'neg' : w.used >= 75 ? 'warn' : ''}">${100 - w.used}% left</b><span>↻ ${dur(w.reset)}${w.out ? ' · ≈ out' : ''}</span>`).join('')}</div>`;
    };
    return head('Accounts · 7 days · limits read 38 s ago',
      `<span class="w">Codex Pro</span> will likely run out of its weekly limit on Sat at about 14:08; Claude Max has the most room, start there next.`,
      `${accounts.length} accounts across ${Object.keys(byTool).length} tools, each in its own folder with its own login. Sorted by room left. Usage at API list prices, so ≈.`,
      seg(['7 d', '14 d', '30 d'], '7 d') + btn('Export CSV') + btn('Start in a folder…') + btn('Add account', 'pri'))
    + `<section class="pn" style="margin-bottom:var(--s3)">${ph('Tokens per day, by account', '7 days · measured', accounts.filter((a) => a.tokens).map((a) => `<span class="chip">${ticon(a.tool, 11)}${a.label}</span>`).join(' '))}
        <svg viewBox="0 0 700 90" style="width:100%;height:auto;display:block" aria-label="Tokens per day, stacked by account">${Array.from({ length: 7 }, (_, d) => { let y = 80; const h = [34, 22, 12, 4, 1].map((v) => v * (0.6 + r() * 0.8)); return h.map((v, i) => { y -= v; return `<rect x="${40 + d * 94}" y="${y}" width="60" height="${v}" fill="${['var(--ac)', 'var(--warn)', 'var(--r2)', '#6FA8F5', '#E384B8'][i]}" opacity="${d === 6 ? 1 : 0.7}"/>`; }).join('') + `<text x="${70 + d * 94}" y="89" font-size="8.5" text-anchor="middle" fill="var(--m)" font-family="var(--mono)">${d === 6 ? 'today' : new Date(NOW - (6 - d) * DAY).toLocaleDateString('en-GB', { weekday: 'short' })}</text>`; }).join('')}</svg></section>
      <div class="bar">${seg(['All', 'Claude Code', 'Codex', 'OpenCode', 'Pi', 'Antigravity'], 'All')}</div>
      ${Object.entries(byTool).map(([t, as]) => `<div class="grp"><div class="gh">${toolPill(t)}<b>${as.length}</b><span>${as.filter((a) => !a.needsLogin).length} logged in · ${as.reduce((s, a) => s + (acctX[a.id].open || 0), 0)} consoles open · ${fmt(as.reduce((s, a) => s + acctX[a.id].tokens7, 0))} tokens · ≈ ${money(as.reduce((s, a) => s + acctX[a.id].cost7, 0))} · 7 days</span></div>
        <table class="t"><tbody>${as.map((a) => { const x = acctX[a.id]; return `<tr${x.note && x.note[0] === 'ok' ? ' style="background:var(--acS)"' : ''}><td style="width:5rem">${ring(a, 64)}</td>
          <td style="width:16rem"><b>${a.label}</b>${x.main ? ' <span class="m xs">· main</span>' : ''}<small>${esc(x.email)}</small>${planPill(a)} <span class="m xs">${x.added}</span></td>
          <td style="min-width:16rem">${lim(a)}${x.note ? `<div class="note ${x.note[0]}">${x.note[1]}</div>` : ''}</td>
          <td class="n">${fmt(x.tokens7)}<br><span class="m">tokens · 7 d</span>${x.open ? `<br><span style="color:var(--ac)">● ${x.open} open</span>` : ''}</td>
          <td class="n">≈ ${money(x.cost7)}<br><span class="m">list price</span></td><td class="n hide-n">${x.read}</td>
          <td class="act">${a.needsLogin ? btn('Log in', 'pri sm') : btn('Start', 'pri sm')} ${btn('⋯', 'sm')}</td></tr>`; }).join('')}</tbody></table></div>`).join('')}
      <section class="pn" style="margin-top:var(--s3)"><div class="ph" style="margin:0"><h2>By model</h2><span>7 days · folded</span><span class="r">▸</span></div></section>`;
  } };

  P.pages.account = { nav: 'accounts', html: () => {
    const a = accounts[1], x = acctX[a.id];
    return head('<a>← Accounts</a>', `${ticon('codex', 22)}${a.label}`,
      `Codex · <span class="mono">${x.email}</span> · ${pill('Pro', 'ok')} · 1 console open in sanduq-api, on Lab.`,
      btn('Start in a folder…') + btn('Start Codex Pro', 'pri') + btn('⋯'))
    + `<div class="bar">${seg(['Overview', 'Sessions', 'Projects', 'Profile settings'], 'Overview')}</div>
      <div class="tiles">${tile('Tokens · 7 days', fmt(x.tokens7), '64% read from cache')}${tile('At list prices', '≈ ' + money(x.cost7), 'subscription: not billed per token')}${tile('First reply p95', '≈ 2.4 s', 'p50 0.9 s · 412 replies')}${tile('Sessions · 7 days', 4, '1 running now')}${tile('Busiest project', '58%', 'sanduq-api')}</div>
      <div class="cols"><div class="stack">
        <section class="pn">${ph('Limits', 'read 38 s ago · dashed is the estimate at the reset')}<div style="display:flex;gap:var(--s5);align-items:center">${ring(a, 96)}<div style="flex:1">${a.windows.map((w) => `<div style="margin-bottom:var(--s2)"><div style="display:flex;justify-content:space-between;font-size:var(--fs-sm)"><b>${w.label}</b><span class="mono xs ${w.used >= 75 ? 'warn' : 'm'}" style="color:${w.used >= 75 ? 'var(--warn)' : ''}">${100 - w.used}% left · resets in ${dur(w.reset)}</span></div><span class="track"><i class="${w.used >= 75 ? 'warn' : ''}" style="width:${w.used}%"></i></span><div class="note ${w.out ? 'warn' : 'ok'}">${w.out ? 'Runs out ' + HM.at(w.out) + ' at this pace, ' + dur(w.reset - w.out) + ' before the reset.' : 'Lasts until the reset at this pace.'}</div></div>`).join('')}</div></div></section>
        <section class="pn">${ph('Tokens per day', '30 days · measured', fmt(x.tokens7 * 4) + ' · ≈ ' + money(x.cost7 * 4, 0))}${dots(series(30, 1.3e6), 'var(--ac)', 30, 5)}</section>
        <section class="pn">${ph('By model', '30 days')}<table class="t"><tbody>${[['gpt-6.1-sol', 0.82, 29.1e6, 198.2], ['gpt-6.1-mini', 0.14, 5.1e6, 9.4], ['o5', 0.04, 1.4e6, 38.1]].map(([m, sh, t, c]) => `<tr><td class="mono xs">${m}</td><td style="width:40%"><span class="track"><i style="width:${sh * 100}%"></i></span></td><td class="n">${fmt(t)}</td><td class="n">≈ ${money(c)}</td></tr>`).join('')}</tbody></table></section></div>
      <div class="stack"><section class="pn">${ph('Login and folder')}<dl class="kv"><dt>Login</dt><dd><span style="color:var(--ac)">valid</span></dd><dt>Folder</dt><dd class="mono xs">C:\\Users\\sabus\\.codex-pro</dd><dt>Started with</dt><dd class="mono xs">CODEX_HOME</dd><dt>CLI</dt><dd class="mono xs">hrm start codex-pro</dd></dl><div style="margin-top:var(--s3)">${btn('Log in again', 'sm')}</div></section>
        <section class="pn">${ph('Open now')}<div class="xs"><span class="mono">D:\\Projects\\sanduq-api</span> on <span class="chip"><i style="background:${pcCol('lab')}"></i>Lab</span><br><span class="m mono">PID 18240 · since 15:08</span></div></section>
        <section class="pn">${ph('Recent sessions', '', '<a class="lnk">All 14</a>')}<table class="t"><tbody>${sessions.filter((s) => s.account === a.label).slice(0, 4).map((s) => `<tr><td><span class="dot ${s.open ? 'on' : ''}"></span></td><td><b>${esc(s.title)}</b><small>${s.project}</small></td><td class="tm">${s.open ? 'live' : at(s.when)}</td></tr>`).join('')}</tbody></table></section></div></div>`;
  } };

  const mxHead = (ids) => `<tr><th>Server</th>${ids.map((id) => `<th class="tl">${ticon(id, 16)}<small>${(tools.find((t) => t.id === id) || {}).label}</small></th>`).join('')}<th></th></tr>`;
  const ck = (on, some) => `<td class="ck"><i class="${on ? 'on' : some ? 'some' : ''}"></i></td>`;
  P.pages.mcp = { find: 'Find a server', html: () => {
    const ids = ['claude', 'codex', 'opencode', 'agy'], written = mcp.reduce((s, m) => s + m.on.length, 0), alone = mcp.filter((m) => m.alone).length;
    return head('MCP servers · checked 5 m ago',
      `${mcp.length} servers are written into ${written} places; <span class="w">${alone} entries were left alone</span> because a tool's file had them by hand.`,
      'Kept once here and written into every tool you tick, in that tool\'s own format. An entry written by hand is never replaced without asking.',
      btn('Import from apps') + btn('Add MCP server', 'pri'))
    + `<div class="bar">${pill(alone + ' left alone · checked 5 m ago', 'warn')}<span>${mcp.length} servers · ${ids.length} tools · ${written} entries written</span></div>
      <section class="pn"><table class="t mx"><thead>${mxHead(ids)}</thead><tbody>${mcp.map((m) => `<tr><td><b>${m.name}</b> <span class="tag">${m.tr}</span>${m.off ? pill('Off', 'warn') : ''}${m.alone ? ' ' + pill('left alone in ' + (tools.find((t) => t.id === m.alone) || {}).label, 'warn') : ''}<small>${esc(m.line)}${m.desc ? ' · ' + m.desc : ''}</small></td>${ids.map((id) => ck(m.on.includes(id))).join('')}<td class="act">${btn('✎', 'sm ghost')}${btn('⋯', 'sm ghost')}</td></tr>`).join('')}</tbody></table></section>`;
  } };

  P.pages.skills = { find: 'Find a skill', html: () => {
    const ids = ['claude', 'codex', 'opencode', 'pi'];
    return head('Skills · 61 in the store',
      `<span class="w">3 folders in the store are not managed yet</span>; 2 skills have an update from their source.`,
      'Skills live once, in C:\\Users\\sabus\\.hrm\\skills, and are linked into each tool you tick. Manage adds a folder the list did not know.',
      btn('Check for updates') + btn('Import from a tool') + btn('+ Install skill', 'pri'))
    + `<div class="bar">${seg(['Installed 42', 'Discover'], 'Installed 42')}<span class="r">Source ${seg(['all', 'anthropics/skills', 'Imported', 'Local', 'Store', 'Not managed'], 'all')} · 39 managed · 3 not managed · 2 updates</span></div>
      <section class="pn"><table class="t mx"><thead><tr><th>Skill</th><th>Source</th>${ids.map((id) => `<th class="tl">${ticon(id, 16)}<small>${skills.filter((s) => s.on.includes(id)).length}</small></th>`).join('')}<th></th></tr></thead><tbody>${skills.map((s) => `<tr><td><b>${s.name}</b>${s.upd ? ' ' + pill('⬇ Update', 'ok') : ''}<small>${s.desc}</small></td><td class="mono xs m">${s.src}</td>${ids.map((id) => ck(s.on.includes(id))).join('')}<td class="act">${btn('⋯', 'sm ghost')}</td></tr>`).join('')}</tbody></table></section>
      <section class="pn" style="margin-top:var(--s3)">${ph('Not managed', '3 folders in the store this list did not know', btn('Manage all', 'pri sm'))}<table class="t"><tbody>${[['gsap-core', 'linked in Claude Code, Codex'], ['remotion-render', 'not linked'], ['obsidian-cli', 'linked in Claude Code']].map(([n, l]) => `<tr><td><b>${n}</b></td><td class="xs m">${l}</td><td class="act">${btn('Manage', 'sm')}</td></tr>`).join('')}</tbody></table></section>`;
  } };

  P.pages.prompts = { html: () => head('System prompts',
      `Claude Code reads <span class="w">Default house rules</span>; Codex reads a file this library does not know.`,
      'The instructions each tool reads when it starts: CLAUDE.md for Claude Code, AGENTS.md for Codex and OpenCode, GEMINI.md for Antigravity.',
      btn('New prompt', 'pri'))
    + `<div class="bar">${seg([`${ticon('claude', 12)}Claude Code`, `${ticon('codex', 12)}Codex`, `${ticon('opencode', 12)}OpenCode`, `${ticon('agy', 12)}Antigravity`], `${ticon('claude', 12)}Claude Code`)}</div>
      <div class="cols list"><div class="pn railc" style="padding:var(--s2)"><a><b>The file now</b><em>${pill('live', 'ok')}</em></a>${prompts.map((p, i) => `<a class="${i ? '' : 'on'}"><span><b style="display:block;font-weight:600">${p.title}</b><span class="xs m">${esc(p.first)}</span></span>${p.on ? `<em>${pill('enabled', 'ok')}</em>` : ''}</a>`).join('')}</div>
      <section class="pn"><div class="ph"><input class="fld" value="Default house rules" style="font-weight:600;flex:1">${pill('enabled in Claude Code', 'ok')}</div>
        <div class="xs m" style="margin-bottom:var(--s2)">Writes to <span class="mono">C:\\Users\\sabus\\.claude\\CLAUDE.md</span> · every Claude Code account follows it</div>
        <textarea class="fld mono" rows="14">Write like a person. Short sentences, plain words, no filler.
Say what changed and why; never what you are about to do.
Use the project's own helpers before adding one.
Tests: one small test for new logic; none for trivial changes.
Never touch secrets, credentials or CI runners without asking.</textarea>
        <div class="bar" style="margin-top:var(--s3)"><span>This prompt is in the file now.</span><span class="r">${btn('Delete', 'neg sm')} ${btn('Show the live file', 'sm')} ${btn('Save', 'pri sm')}</span></div></section></div>` };

  /** The router's routes as a small map in the dial's language: accounts left, the router gate, providers right. */
  function routeMap() {
    const w = 640, lx = 130, rx = 505, gx = 318;
    const left = [['OpenCode routed', 30], ['Pi on OpenRouter', 110]], right = [['DeepSeek', 'ok', 20], ['OpenRouter', 'ok', 65], ['Groq', 'neg', 110]];
    let s = `<svg viewBox="0 0 ${w} 150" style="width:100%;height:auto;display:block" aria-label="Routed accounts, the router, the providers">`;
    for (const [n, y] of left) s += `<path d="M${lx + 8} ${y} C ${gx - 60} ${y}, ${gx - 60} 70, ${gx - 10} 70" fill="none" stroke="var(--r2)" stroke-width="1.5" opacity=".6"/><circle cx="${lx}" cy="${y}" r="5" fill="var(--r2)"/><text x="${lx - 12}" y="${y + 4}" text-anchor="end" font-size="11" fill="var(--ink)" font-family="var(--sans)">${n}</text>`;
    for (const [n, k, y] of right) s += `<path d="M${gx + 10} 70 C ${gx + 60} 70, ${gx + 60} ${y}, ${rx - 8} ${y}" fill="none" stroke="${k === 'neg' ? 'var(--neg)' : 'var(--r2)'}" stroke-width="1.5" opacity=".6" ${k === 'neg' ? 'stroke-dasharray="3 4"' : ''}/><circle cx="${rx}" cy="${y}" r="5" fill="${k === 'neg' ? 'var(--neg)' : 'var(--r2)'}"/><text x="${rx + 12}" y="${y + 4}" font-size="11" fill="var(--ink)" font-family="var(--sans)">${n}</text>`;
    s += `<rect x="${gx - 10}" y="60" width="20" height="20" rx="4" fill="var(--s)" stroke="var(--r2)" stroke-width="1.6"/><circle cx="${gx}" cy="70" r="3" fill="var(--r2)"/><text x="${gx}" y="98" text-anchor="middle" font-size="10.5" fill="var(--m)" font-family="var(--mono)">127.0.0.1:8787</text>`;
    s += `<text x="${rx + 12}" y="126" font-size="10" fill="var(--neg)" font-family="var(--mono)">paused · retry 4 min</text>`;
    return `<div class="routemap">${s}</svg></div>`;
  }
  P.pages.routing = { html: () => head('API keys & routing',
      `The router is up: 1,284 requests in 24 h; <span class="w">Groq is paused</span> after 3 errors in a row, cheap-fast falls over to DeepSeek.`,
      'Keys live only in Windows Credential Manager. The local router sends routed accounts to providers in the order you set. Every number here is measured by the router.',
      btn('Add provider') + btn('Add route', 'pri'))
    + `<section class="pn" style="margin-bottom:var(--s3)"><div class="ph"><span class="dot on"></span><h2>Local router · running</h2><span>127.0.0.1:8787 · token required · 2 routes · 2 accounts use it</span><span class="r">${btn('Stop router', 'sm')}</span></div>
      <div class="cols">${routeMap()}<div class="tiles" style="margin:0;grid-template-columns:1fr 1fr">${tile('Requests · 24 h', '1,284', 'measured')}${tile('Failovers', '7', 'to DeepSeek', 'warn')}${tile('Errors', '3', 'all Groq', 'neg')}${tile('Paused', '1', 'Groq · retry 4 min', 'neg')}</div></div></section>
      <div class="cols eq" style="margin-bottom:var(--s3)"><section class="pn">${ph('Routes')}<div class="stack">${[['cheap-fast', 'failover · used by OpenCode routed', [['DeepSeek', 'serving', 'ok'], ['Groq', 'paused · retry 4 m', 'neg'], ['OpenRouter', 'standby', '']]], ['openrouter', 'failover · used by Pi on OpenRouter', [['OpenRouter', 'serving', 'ok']]]].map(([id, sub, chain]) => `<div class="grp"><div class="gh"><b class="mono">${id}</b><span>${sub}</span><span class="r">${btn('Remove', 'neg sm ghost')}</span></div><div class="route">${chain.map(([n, st, k], i) => `${i ? '<span class="arrow">→</span>' : ''}<span class="pill ${k}">${n} · ${st}</span>`).join('')}</div></div>`).join('')}</div></section>
      <section class="pn">${ph('Providers')}<table class="t"><thead><tr><th>Provider</th><th>Key</th><th>Breaker</th><th class="n">24 h</th><th class="n">First chunk p95</th></tr></thead><tbody>${providers.map((p) => `<tr><td><b>${p.name}</b><small>${p.url}</small></td><td>${p.key ? pill('saved', 'ok') : pill('missing', 'neg')}</td><td>${p.paused ? pill(p.breaker, 'neg') : pill('closed', 'ok')}</td><td class="n">${p.req}</td><td class="n">${p.p95} s</td></tr>`).join('')}</tbody></table></section></div>
      <section class="pn">${ph('Recent requests through the router', 'the router\'s own log · measured')}<table class="t"><thead><tr><th>Time</th><th>Account</th><th>Route → provider</th><th>Model</th><th>Status</th><th class="n">First chunk</th><th class="n">Time</th><th class="n">Tokens</th></tr></thead><tbody>${reqs.map((q) => `<tr><td class="tm">${q.t}</td><td>${q.acct}</td><td class="xs">${q.route}</td><td class="mono xs">${q.model}</td><td>${pill(q.st, q.st === 200 ? 'ok' : 'neg')}</td><td class="n">${q.first == null ? '—' : q.first + ' s'}</td><td class="n">${q.time} s</td><td class="n">${q.tok == null ? '—' : fmt(q.tok)}</td></tr>`).join('')}</tbody></table></section>` };

  P.pages.apps = { html: () => head('Apps · checked 09:14',
      `<span class="w">Codex is behind</span>: 0.98.0 installed, 0.99.1 out; it is installed twice, and your terminals run the npm copy.`,
      'The tools on this computer, every copy of each, how it was installed, and the newest version its vendor offers.',
      btn('Check for updates'))
    + `<div class="warnbar">${ticon('codex', 16)}Codex is installed 2 times. Your terminals run the npm copy (0.98.0); the winget copy is 0.95.0. Keep one.${btn('Remove the winget copy', 'sm')}</div>
      <section class="pn">${ph('Installed', '5 tools')}<table class="t"><thead><tr><th>Tool</th><th>Installed</th><th>Newest</th><th>Installed by</th><th></th><th>Show</th></tr></thead><tbody>${apps.filter((a) => a.ver).map((a) => `<tr><td>${ticon(a.id, 20)}<b>${a.label}</b><small>${a.path}</small></td><td class="mono xs">${a.ver}</td><td class="mono xs" style="color:${a.newest && a.newest !== a.ver ? 'var(--warn)' : ''}">${a.newest || '<span class="m">could not check</span>'}</td><td class="xs">${a.by}</td><td class="act">${a.newest && a.newest !== a.ver ? btn('Update', 'pri sm') : a.by.startsWith('native') ? btn('Reinstall ' + a.ver, 'sm') : btn('Up to date', 'sm ghost')} ${btn('⋯', 'sm ghost')}</td><td class="ck"><i class="on"></i></td></tr>${a.dup ? `<tr><td style="padding-left:2.4rem"><span class="xs m">${a.dup.path} · second copy, not on PATH first</span></td><td class="mono xs">${a.dup.ver}</td><td></td><td class="xs">${a.dup.by}</td><td class="act">${btn('Remove…', 'neg sm ghost')}</td><td></td></tr>` : ''}`).join('')}</tbody></table></section>
      <section class="pn" style="margin-top:var(--s3)">${ph('Not installed', '1')}<table class="t"><tbody>${apps.filter((a) => !a.ver).map((a) => `<tr><td>${ticon(a.id, 20)}<b>${a.label}</b></td><td class="mono xs">—</td><td class="mono xs">${a.newest}</td><td></td><td class="act">${btn('Install ▾', 'pri sm')}</td><td></td></tr>`).join('')}</tbody></table></section>
      <section class="pn" style="margin-top:var(--s3)"><div class="ph"><span class="mark hm" style="width:20px;height:20px;display:inline-grid">${LOGOS.hm}</span><h2>Harness Manager</h2><span>1.6.2 · 1.6.3 is available: Settings › Updates installs it</span><span class="r">${pill('update available', 'warn')}</span></div></section>` };

  P.pages.cli = { html: () => head('CLI reference', '<span class="mono">hrm</span> does everything this window does.', 'Add --json to any command for scripts.')
    + `<pre class="ex">hrm list                                     <i># accounts, plans, limits, tokens, open consoles</i>
hrm start work-max --cwd D:\\Projects\\my-app   <i># a new terminal, in that folder</i>
hrm quota work-max --live                    <i># limits straight from Anthropic</i>
hrm sessions --group project --json          <i># every tool's sessions, for scripts</i></pre>
      <div class="cli">${[['Accounts', [['list', 'every account, with limits'], ['show <name>', 'one account in full'], ['add <name>', 'a new account, then log in'], ['login <name>', 'log in again'], ['remove <name>', 'remove it'], ['key <name>', 'set or replace its API key']]],
        ['Run', [['run <name>', 'in this folder'], ['start <name> --cwd', 'a new terminal there'], ['resume <name> <id>', 'continue a session'], ['open <name>', 'the account\'s folder']]],
        ['Usage and limits', [['usage', 'tokens and cost by account'], ['overview', 'what the Overview shows'], ['requests', 'the request log'], ['quota <name> --live', 'limits from the provider']]],
        ['Sessions and projects', [['sessions', 'every tool\'s sessions'], ['session <id>', 'one session'], ['projects', 'with costs'], ['tag <id> <tags>', 'tag a session'], ['tags', 'every tag']]],
        ['Shared setup', [['sync', 'write MCP, skills, prompts'], ['mcp', 'servers'], ['skills', 'the store'], ['prompts', 'the library']]],
        ['Routing and setup', [['provider', 'add, list, remove'], ['route', 'add, list, remove'], ['router', 'start, stop, status'], ['settings', 'read or set one'], ['init', 'first run'], ['import', 'the old launcher']]],
        ['Computers', [['computers status', 'every computer'], ['computers now', 'what runs where'], ['hub join | invite | status | leave', 'a hub you host'], ['ring show | enter', 'the recovery key'], ['computers pair | peers | forget | serve', 'direct connections']]]]
        .map(([t, rows]) => `<section class="pn"><h2>${t}</h2><div class="cmd">${rows.map(([c, d]) => `<code>hrm ${esc(c)}</code><span>${d}</span>`).join('')}</div></section>`).join('')}</div>` };

  const sw = (on, t) => `<span class="sw ${on ? 'on' : ''}"><i></i>${t}</span>`;
  const srow = (t, d, c) => `<div class="srow"><div><b>${t}</b><p>${d}</p></div><div class="c">${c}</div></div>`;
  P.pages.settings = { html: () => head('Settings', 'Saved as you change them, for this computer.', 'Keys and logins are never in these files or in backups.')
    + `<div class="stack" style="max-width:60rem">
      <section class="pn sec">${ph('Appearance', '3 settings')}${srow('Theme', 'Harness or VektraGrid; light, dark or with the system.', seg(['Harness', 'VektraGrid'], 'Harness') + seg(['System', 'Light', 'Dark'], 'System'))}${srow('Limits shown as', 'Room left, or what is used.', seg(['Left', 'Used'], 'Left'))}${srow('Moving routes', 'Animate the routes on the Overview. Off when Windows asks for less motion.', sw(true, 'Animate the routes'))}</section>
      <section class="pn sec">${ph('Window and terminal', '3 settings')}${srow('Close to the tray', 'Keep running in the tray when the window closes.', sw(true, 'Keep running in the tray'))}${srow('Launch at sign-in', 'Start with Windows.', sw(false, 'Start with Windows'))}${srow('Terminal', 'Where sessions open.', seg(['Windows Terminal', 'Console'], 'Windows Terminal'))}</section>
      <section class="pn sec">${ph('Updates', '2 settings')}${srow('Harness Manager', 'Version <b>1.6.3</b> is available; you have 1.6.2. New versions are looked for at launch and every hour.', btn('Update and restart', 'pri sm') + btn('Check for updates', 'sm'))}${srow('New versions of your tools', 'Ask each vendor at launch.', sw(true, 'Check at launch'))}</section>
      <section class="pn sec">${ph('Usage and limits', '2 settings')}${srow('Refresh', 'How often usage and limits are read.', seg(['1 min', '5 min', '15 min'], '5 min'))}${srow('Live Claude limits', 'Ask Anthropic for current limits instead of the copy Claude Code last saved.', sw(true, 'Ask Anthropic for current limits'))}</section>
      <section class="pn sec">${ph('Connections', '2 settings')}${srow('Router port', 'Where the local router listens.', '<input class="fld mono" value="8787" style="width:6rem">' + btn('Change', 'sm'))}${srow('Account list', 'The file that names your accounts.', '<input class="fld mono" value="C:\\Users\\sabus\\.hrm\\accounts.json" style="width:18rem">' + btn('Move…', 'sm'))}</section>
      <section class="pn sec">${ph('Computers', '3 computers · synced through a folder · last round 40 s ago', btn('↻ Sync now', 'pri sm'))}${srow('This computer', 'Its name on every other computer.', '<input class="fld" value="Studio" style="width:12rem">' + btn('Rename', 'sm'))}
        <div class="srow"><div><b>How your computers sync</b><p>A shared folder, or a hub you host. Both carry the journal, the backups and the archive.</p></div><div></div></div>
        <div class="cols eq" style="margin:0 0 var(--s3)"><div class="grp" style="border-color:var(--ac)"><b>A shared folder</b><p class="xs m" style="margin:2px 0 var(--s2)">OneDrive, Google Drive, SMB or a NAS.</p><span class="mono xs">D:\\OneDrive\\HM</span><div style="margin-top:var(--s2)">${btn('Change…', 'sm')} ${btn('Stop syncing', 'sm ghost')}</div></div><div class="grp"><b>A hub you host</b><p class="xs m" style="margin:2px 0 var(--s2)">Docker behind Caddy, sealed with a ring key.</p><span class="xs m">Not joined yet</span><div style="margin-top:var(--s2)">${btn('Join with a code…', 'sm')}</div></div></div>
        ${srow('Recovery key', 'The ring key that seals a hub. Only with a hub.', btn('Show again', 'sm') + btn('Enter recovery key…', 'sm'))}
        ${srow('Carry transcripts', 'Carried 214 · 48.2 MB · 2 skipped over 20 MB; 200 MB cap.', sw(true, 'Carry transcripts'))}
        ${srow('Presence', 'Share what runs here with your other computers.', sw(true, 'Share what runs here'))}
        ${srow('Direct connection', 'Computers on the same network or paired by code talk directly; listens on port <span class="mono">7421</span> · <a class="lnk xs">Change…</a>', sw(true, 'Connect directly') + btn('Pair a computer…', 'sm'))}
        <div class="gh" style="margin-top:var(--s3)"><b>3 computers</b><span>synced through a folder · last round 40 s ago</span></div>
        <div class="cards" style="grid-template-columns:repeat(auto-fill,minmax(14rem,1fr))">${computers.map((c) => `<div class="grp"><div class="gh"><i class="dot" style="background:${c.col};opacity:1"></i><b>${c.name}</b>${c.me ? pill('this one', 'ok') : ''}</div><div class="xs" style="color:${c.seen > 60e3 ? 'var(--warn)' : 'var(--ac)'}">${c.me ? 'Windows · 1.6.2 · writing now' : 'heard ' + dur(c.seen) + ' ago'}</div><span class="track" style="margin:6px 0"><i style="width:${c.me ? 100 : 100 - c.seen / 2e3}%"></i></span><div class="xs m">${c.me ? '14 projects here · 5 accounts · 2 running' : (c.id === 'lab' ? '9 projects · 3 accounts · 2 running' : '4 projects · 1 account · 1 running')}</div>${c.me ? '' : `<div class="xs m" style="margin-top:4px">${c.id === 'lab' ? 'direct · mDNS · connected' : 'folder · heard ' + dur(c.seen) + ' ago'}</div><div style="display:flex;gap:4px;margin-top:var(--s2);align-items:center"><input class="fld mono" value="${c.id === 'lab' ? 'lab.local:7421' : ''}" placeholder="name:port" style="width:9rem;height:1.6rem">${btn('Save', 'sm ghost')}${btn('Forget', 'sm ghost neg')}</div>`}</div>`).join('')}</div></section>
      <section class="pn sec">${ph('Backups and archive', '8 backups · 42.1 MB', btn('Import…', 'sm') + ' ' + btn('Export…', 'sm') + ' ' + btn('⬇ Back up now', 'pri sm'))}
        ${srow('Data folder', 'Where this computer keeps its database and journal.', '<span class="mono xs">C:\\Users\\sabus\\.hrm</span>' + btn('Open', 'sm'))}
        ${srow('Automatic backups', 'How often, and how many to keep.', '<select class="fld"><option>Every 12 hours</option></select><select class="fld"><option>Keep 10</option></select>')}
        ${srow('Also in the sync folder', 'A copy every computer can restore from.', sw(true, 'Also in the sync folder'))}
        ${srow('Include transcripts', 'Backups carry the session transcripts too.', sw(false, 'Include transcripts'))}
        <table class="t" style="margin:var(--s2) 0"><tbody>${[['5 Nov 2026, 21:00', 'Automatic', '5.2 MB'], ['5 Nov 2026, 09:00', 'Automatic', '5.1 MB'], ['4 Nov 2026, 18:12', 'Before restore', '4.9 MB'], ['4 Nov 2026, 09:00', 'Automatic', '4.9 MB']].map(([t, l, s]) => `<tr><td class="tm">${t}</td><td>${l}</td><td class="n">${s}</td><td class="act">${btn('✎', 'sm ghost')}${btn('✕', 'sm ghost')}${btn('↻ Restore', 'sm')}</td></tr>`).join('')}
        <tr><td colspan="4" class="xs m" style="padding-top:var(--s3)">In the sync folder · every computer's</td></tr>${[['Automatic · Lab', '5 Nov 2026, 20:00', '5.1 MB'], ['Automatic · Surface', '5 Nov 2026, 19:30', '2.4 MB']].map(([l, t, s]) => `<tr><td class="tm">${t}</td><td>${l}</td><td class="n">${s}</td><td class="act">${btn('↻ Restore…', 'sm')}</td></tr>`).join('')}</tbody></table>
        ${srow('Archive sessions older than', 'Transcripts move out of the database into the archive.', seg(['Never', '30 days', '90 days', '1 year'], '90 days') + '<input class="fld mono" value="90" style="width:4rem"> days')}
        ${srow('Archive location', 'The sync folder, so every computer can read it, or a folder of your own.', sw(true, 'The sync folder') + '<span class="mono xs">D:\\OneDrive\\HM\\archive</span>' + btn('Change…', 'sm'))}
        ${srow('Archive now', '312 archived · 1.2 GB · sync folder 300 · folder 12 · last run Today 03:10.', btn('Archive now', 'sm'))}
        ${srow('Restore', 'From the sync folder, a backup, a folder or a zip file.', btn('Restore…', 'sm'))}</section>
      <section class="pn sec">${ph('Diagnostic log', '3 settings')}${srow('Keep a log', 'A file of what the app did, for a bug report.', sw(true, 'Keep a log'))}${srow('Level', 'How much it writes.', seg(['Errors', 'Warnings', 'Everything'], 'Warnings'))}${srow('Log file', 'C:\\Users\\sabus\\.hrm\\hrm.log', btn('Open log', 'sm'))}</section>
      <section class="pn sec">${ph('Maintenance', '3 settings')}${srow('Find tools again', 'Look for tools installed since the first run.', btn('Find tools', 'sm'))}${srow('Old launcher', 'Import accounts and keys from it.', btn('Import', 'sm'))}${srow('Tours', 'Each page shows its tour again.', btn('Show the tours again', 'sm'))}</section></div>` };
})();
