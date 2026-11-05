// Overview: the whole setup at a glance. Key numbers against the period before, live routes drawn with WebGL, every
// limit on a seven-day line, the activity grid, and the latest replies or the request log.

import { S, call, view, render, attempt, changed, toast, loadState, action, acctName, fit, refreshText, shownTools } from './core.js';
import { esc, fmt, hm, dur, toolIcon, toolPill, ic } from './lib.js';
import { findings, limitRows } from './accounts.js';
import { computersStatus, syncOn, computers, nameOf, cdot, cdots, splitBar, tileSplit, scope, scopeSeg } from './computers.js';

const TOOL = { claude: 'Claude Code', codex: 'Codex', opencode: 'OpenCode', agy: 'Antigravity', pi: 'Pi', omp: 'Oh My Pi' };
const VENDOR = { claude: 'Anthropic', codex: 'OpenAI', opencode: 'OpenCode', agy: 'Google', pi: 'Pi', omp: 'Oh My Pi' };
const RANGES = [['24h', 24], ['7d', 168], ['30d', 720]];
// device: the Activity grid's own computer ('' is every computer added up)
const st = { harness: '', range: '24h', metric: 'tokens', rows: null, total: 0, log: false, device: '' };
const money = (c, digits = 2) => (c == null ? '—' : '$' + c.toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits }));
const secs = (ms) => (ms == null ? '—' : (ms / 1000).toFixed(1));
const when = (ms) => new Date(ms).toLocaleString([], { weekday: 'short', hour: '2-digit', minute: '2-digit', hour12: false });

/** GitHub-style grid: weeks across, Monday to Sunday down, five shades from the day's share of the busiest days. */
// pc: the computers and each day's tokens by computer (by[day] = {id: tokens}); dev: one computer's tokens only
function heatmap(days, metric, width = 900, pc = null) {
  const byDev = (d) => (pc && pc.by[d.day]) || null;
  const val = (d) => (pc && pc.dev ? (byDev(d) || {})[pc.dev] || 0 : metric === 'cost' ? d.cost : metric === 'requests' ? d.requests : d.tokens);
  const nz = days.map(val).filter((v) => v > 0).sort((a, b) => a - b);
  const q = (p) => nz[Math.min(nz.length - 1, Math.floor(p * nz.length))];
  const cuts = nz.length ? [q(0.25), q(0.5), q(0.75)] : [];
  const shade = ['var(--h0)', 'var(--h1)', 'var(--h2)', 'var(--h3)', 'var(--h4)'];
  const top1 = nz[nz.length - 1];
  // the busiest day is always the darkest
  const level = (v) => (v <= 0 ? 0 : v >= top1 ? 4 : v <= cuts[0] ? 1 : v <= cuts[1] ? 2 : v <= cuts[2] ? 3 : 4);
  const gap = 3, left = 30, top = 18;
  const first = new Date(days[0].day + 'T12:00');
  const offset = (first.getDay() + 6) % 7; // Monday = 0
  const cols = Math.ceil((days.length + offset) / 7);
  const cell = Math.max(5, Math.min(40, Math.floor((width - left) / cols) - gap));
  const w = left + cols * (cell + gap), h = top + 7 * (cell + gap) + 4;
  let s = `<svg width="${w}" height="${h}" style="max-width:100%;height:auto" viewBox="0 0 ${w} ${h}" role="img" aria-label="Daily usage over the last 53 weeks" data-testid="heatmap">`;
  let lastMonth = -1, lastLabelCol = -9;
  days.forEach((d, i) => {
    const date = new Date(d.day + 'T12:00');
    const col = Math.floor((i + offset) / 7), row = (i + offset) % 7;
    const x = left + col * (cell + gap), y = top + row * (cell + gap);
    if (row === 0 || i === 0) {
      const m = date.getMonth();
      if (m !== lastMonth && col - lastLabelCol >= 4) { s += `<text x="${x}" y="11" font-size="10.5" style="fill:var(--m)">${date.toLocaleDateString([], { month: 'short' })}</text>`; lastMonth = m; lastLabelCol = col; }
    }
    const v = val(d);
    // with several computers the caption splits the day's tokens by computer
    const split = byDev(d) && Object.keys(byDev(d)).length > 1 ? ' · ' + Object.entries(byDev(d)).filter(([, t]) => t).map(([id, t]) => `${nameOf(pc.c, id)} ${fmt(t)}`).join(' · ') : '';
    const label = `${date.toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' })}: ${pc && pc.dev ? `${fmt(val(d))} tokens on ${nameOf(pc.c, pc.dev)}` : `${fmt(d.tokens)} tokens · ${d.requests} requests · ${money(d.cost)}`}${split}`;
    s += `<rect class="hm" x="${x}" y="${y}" width="${cell}" height="${cell}" rx="2" style="fill:${shade[level(v)]}" data-l="${level(v)}"><title>${esc(label)}</title></rect>`;
  });
  [['Mon', 0], ['Wed', 2], ['Fri', 4]].forEach(([t, r]) => { s += `<text x="0" y="${top + r * (cell + gap) + Math.min(10, cell)}" font-size="10.5" style="fill:var(--m)">${t}</text>`; });
  s += '</svg>';
  return s + `<div class="hmlegend"><span>Less</span>${shade.map((c) => `<i style="background:${c}"></i>`).join('')}<span>More</span></div>`;
}

// from the times the parts of an answer reached the log: close, not exact
const speed = (r) => (r.gen_ms >= 300 && r.output > 0 ? `≈${Math.round(r.output / (r.gen_ms / 1000)).toLocaleString()} <small>tok/s</small>` : '<span class="dash">—</span>');
const firstToken = (r) => (r.ttft_ms != null ? `≈${(r.ttft_ms / 1000).toFixed(1)} s` : '<span class="dash">—</span>');
const TIMING = 'Approximate: worked out from the times each tool wrote the parts of the answer to its log';

function requestTable(rows) {
  return `<table class="grid reqlog" style="margin-top:0" data-testid="request-log"><thead><tr><th>Time</th><th>Tool</th><th>Account</th><th>Model</th><th class="num">Cost</th><th class="num">Fresh input</th><th class="num">Output</th><th class="num">Cache read</th><th class="num">Cache write</th><th class="num" title="${TIMING}">Speed</th><th class="num" title="${TIMING}">First reply</th></tr></thead><tbody>
    ${rows.map((r) => `<tr><td style="white-space:nowrap" class="mono">${new Date(r.ts).toDateString() === new Date().toDateString() ? hm(r.ts) + ':' + String(new Date(r.ts).getSeconds()).padStart(2, '0') : new Date(r.ts).toLocaleDateString([], { day: 'numeric', month: 'short' }) + ' ' + hm(r.ts)}</td>
      <td>${toolPill(r.harness, TOOL[r.harness])}</td><td>${esc(acctName(r.profile))}</td><td class="mono">${esc(r.model)}</td><td class="num"><b>${money(r.cost, 4)}</b></td>
      <td class="num">${fmt(r.input)}</td><td class="num">${fmt(r.output)}</td><td class="num">${fmt(r.cache_read)}</td><td class="num">${fmt(r.cache_write)}</td>
      <td class="num" data-testid="speed">${speed(r)}</td><td class="num" data-testid="ttft">${firstToken(r)}</td></tr>`).join('') || '<tr><td colspan="11" class="empty">No replies in this range.</td></tr>'}</tbody></table>`;
}

/** The design's short table: the latest replies with their speed and time to first reply. */
function latestTable(rows) {
  return `<table class="grid" data-testid="latest-replies"><thead><tr><th>Time</th><th>Account</th><th>Model</th><th class="num">Tokens</th><th class="num" title="${TIMING}">Speed</th><th class="num" title="${TIMING}">First reply</th></tr></thead><tbody>
    ${rows.slice(0, 5).map((r) => `<tr><td class="mono">${hm(r.ts)}:${String(new Date(r.ts).getSeconds()).padStart(2, '0')}</td><td>${esc(acctName(r.profile))}</td><td class="mono">${esc(r.model)}</td>
      <td class="num">${fmt(r.input + r.output + r.cache_read + r.cache_write)}</td><td class="num">${speed(r)}</td><td class="num ${r.ttft_ms > 4000 ? 'warnT' : ''}">${firstToken(r)}</td></tr>`).join('') || '<tr><td colspan="6" class="empty">No replies in this range.</td></tr>'}</tbody></table>`;
}

export function setTodayBadge(cost) {
  const el = document.getElementById('today-cost');
  if (el) el.textContent = cost == null ? '' : 'Today ' + money(cost);
}

/** "Busiest day: Tue 28 Oct · 9.1M" under the activity grid. */
function busiest(days, metric) {
  const val = (d) => (metric === 'cost' ? d.cost : metric === 'requests' ? d.requests : d.tokens);
  const top = days.reduce((a, d) => (val(d) > val(a) ? d : a), days[0]);
  if (!top || !val(top)) return '';
  const v = metric === 'cost' ? money(top.cost) : metric === 'requests' ? top.requests.toLocaleString() + ' requests' : fmt(top.tokens);
  return `<p class="hint mono">Busiest day: ${new Date(top.day + 'T12:00').toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' })} · ${v}</p>`;
}

// ---------- key numbers ----------
function spark(vals) {
  if (vals.length < 2) return '<svg viewBox="0 0 120 28" aria-hidden="true"></svg>';
  const v = vals.map((x) => x || 0), max = Math.max(...v), min = Math.min(...v), span = max - min || 1;
  const pts = v.map((x, i) => [(i / (v.length - 1)) * 120, 26 - ((x - min) / span) * 22]);
  const line = pts.map((p, i) => (i ? 'L' : 'M') + p[0].toFixed(1) + ' ' + p[1].toFixed(1)).join(' ');
  return `<svg viewBox="0 0 120 28" preserveAspectRatio="none" aria-hidden="true"><path d="${line} L120 28 L0 28 Z" class="sa"></path><path d="${line}" class="sl"></path></svg>`;
}
function change(cur, prev, kind) {
  if (cur == null || prev == null) return ['', 'flat'];
  if (kind === 'pct') { if (!prev) return [cur ? 'new' : '', 'flat']; const d = Math.round(((cur - prev) / prev) * 100); return [(d > 0 ? '+' : d < 0 ? '−' : '±') + Math.abs(d) + '%', d > 0 ? 'up' : d < 0 ? 'down' : 'flat']; }
  if (kind === 'money') { const d = cur - prev; return [(d >= 0 ? '+' : '−') + money(Math.abs(d)), d > 0 ? 'warn' : 'flat']; }
  if (kind === 'pt') { const d = Math.round((cur - prev) * 100); return [(d >= 0 ? '+' : '−') + Math.abs(d) + ' pt', d > 0 ? 'up' : d < 0 ? 'warn' : 'flat']; }
  const d = (cur - prev) / 1000; // seconds: lower is better
  return [(d > 0 ? '+' : '−') + Math.abs(d).toFixed(1) + ' s', d > 0 ? 'warn' : 'up'];
}
// home: the whole answer of the home call (its tiles carry each number split by computer); pcs: the computers
function kpis(d, ps, risks, home, pcs) {
  const c = d.cur, p = d.prev, sp = d.spark, fresh = !p.requests;
  const best = d.mix.account.filter((m) => m.tokens > 0)[0];
  // no change to show when the period before had no usage at all
  const vs = (...a) => (fresh ? [c.requests ? 'new' : '', 'flat'] : change(...a));
  const tile = (key, label, value, unit, [delta, tone], vals, note, click) => {
    // with several computers: a thin bar of the number by computer, and the largest named in the note
    const parts = tileSplit(home, key);
    // the cache hit is a rate, not a sum: each computer's own, no bar
    const sb = key === 'hit' ? { bar: '', note: parts.length > 1 ? parts.map((x) => `${esc(x.name || nameOf(pcs, x.id))} ${Math.round(x.value * 100)}%`).join(' · ') : '' } : splitBar(pcs, parts);
    return `<div class="ovk ${tone} ${click ? 'click' : ''} ${st.metric === key ? 'on' : ''}" ${click ? `data-act="home-metric" data-v="${key}"` : ''} data-testid="kpi-${key}">
      <div class="kl"><span>${label}</span><span class="kd">${esc(delta)}</span></div><strong>${value}<small> ${unit}</small></strong>${spark(vals)}${sb.bar}<div class="kn">${sb.note ? sb.note + ' · ' : ''}${note}</div></div>`;
  };
  const first = risks[0];
  return `<section class="ovkpis" aria-label="Key numbers">
    ${tile('tokens', 'Tokens', fmt(c.tokens), '', vs(c.tokens, p.tokens, 'pct'), sp.tokens, `${Math.round(c.hit * 100)}% read from cache · ${c.requests.toLocaleString()} ${c.requests === 1 ? 'request' : 'requests'}`, true)}
    ${tile('cost', 'Cost at list prices', money(c.cost), '', vs(c.cost, p.cost, 'money'), sp.cost, c.billed ? `Billed per token: ${money(c.billed)}` : 'Subscriptions are not billed per token', true)}
    ${tile('hit', 'Cache hit', Math.round(c.hit * 100), '%', vs(c.hit, p.hit, 'pt'), sp.hit, best ? `Most tokens: ${esc(acctName(best.key))}` : 'No usage in this range')}
    ${tile('risk', 'Limit risk', risks.length, risks.length === 1 ? 'account' : 'accounts', [first ? when(first.out) : '', risks.length ? 'warn' : 'flat'], [], first ? `<b>${esc(first.name)}</b> runs out ${dur(first.reset - first.out)} before its reset` : 'Every limit lasts until its reset at this pace')}
  </section>`;
}

// ---------- live routes ----------
/** Accounts, the tools they run, the router and the providers, with the traffic of the last five minutes between them. */
// pcs: the computers; with peers, an account card gets a dot per computer it runs on, and traffic only from another
// computer is drawn in that colour
function topology(state, d, pcs) {
  const ps = state.profiles.filter((p) => shownTools(state).some((h) => h.id === p.harness)).slice(0, 8);
  const live = d.live, breakers = live.breakers, now = d.now;
  const peers = syncOn(pcs) && pcs.peers.length > 0;
  const peerRuns = (p) => (peers ? (live.running || []).filter((r) => r.profile === p.name && r.device && r.device !== pcs.this.id) : []);
  const paused = (id) => breakers[id] && breakers[id].state === 'open' && (!breakers[id].retry_at || breakers[id].retry_at > now);
  const rate = (t) => (t > 0 ? Math.max(1, Math.min(12, Math.round(Math.log10(t + 1) * 2))) : 0);
  const nodes = [], edges = [], tools = [], vendors = [], provs = [];
  const add = (list, id, x) => { if (!list.some((n) => n.id === id)) list.push(x); };
  let routerUsed = false;
  for (const p of ps) {
    const tk = (live.accounts[p.name] || {}).tokens || 0;
    const routed = p.kind === 'routed';
    add(tools, 't:' + p.harness, { id: 't:' + p.harness, label: TOOL[p.harness] || p.harness, tokens: 0, open: 0 });
    const tn = tools.find((t) => t.id === 't:' + p.harness);
    tn.tokens += tk;
    const far = peerRuns(p);
    tn.open += p.running.length + far.length;
    const tiers = (p.quota && p.quota.tiers) || [], left = tiers.length ? Math.max(0, Math.round(100 - Math.max(...tiers.map((t) => t.used)))) : null;
    const sub = p.kind === 'subscription' ? (p.identity.needs_login || !p.identity.logged_in ? 'Needs login' : [p.identity.plan || 'logged in', left != null ? left + '% left' : ''].filter(Boolean).join(' · '))
      : routed ? ['Routed', p.model].filter(Boolean).join(' · ') : 'API key';
    const tone = sub === 'Needs login' ? 'neg' : left != null && left < 20 ? 'neg' : left != null && left < 45 ? 'warn' : p.kind === 'subscription' ? 'ok' : '';
    const devs = peers ? [...(p.running.length ? [pcs.this.id] : []), ...far.map((r) => r.device)] : [];
    const onlyFar = far.length && !p.running.length;
    nodes.push({ id: 'a:' + p.name, col: 0, label: p.label, sub, tone, hot: tk ? (routed ? 'a' : onlyFar ? 'p' : 'c') : '', act: `data-act="open-account" data-name="${esc(p.name)}"`, dots: devs.length ? cdots(pcs, devs) : '' });
    edges.push(['a:' + p.name, 't:' + p.harness, !p.identity.logged_in && p.kind === 'subscription' ? 'r' : routed ? 'a' : onlyFar ? 'p' : 'c', rate(tk)]);
    if (routed) {
      routerUsed = true;
      edges.push(['t:' + p.harness, 'router', 'a', rate(tk)]);
      const r = (state.routes || []).find((x) => x.id === p.route);
      for (const pid of (r ? r.providers : [])) {
        const pt = (live.providers[pid] || {}).tokens || 0;
        const lp = live.providers[pid], b = breakers[pid];
        add(provs, 'p:' + pid, { id: 'p:' + pid, label: pid, bad: paused(pid), tone: paused(pid) ? 'neg' : lp ? 'ok' : '',
          sub: paused(pid) ? `paused${b.retry_at ? ' · retry in ' + dur(b.retry_at - now) : ''}` : lp && lp.first_ms != null ? `${secs(lp.first_ms)} s first chunk` : lp ? `${fmt(lp.tokens)} tokens` : 'no traffic' });
        if (!edges.some((e) => e[0] === 'router' && e[1] === 'p:' + pid)) edges.push(['router', 'p:' + pid, paused(pid) ? 'r' : 'a', rate(pt)]);
      }
    } else {
      const v = p.kind === 'api_key' && p.base_url ? (p.base_url.replace(/^https?:\/\//, '').split(/[/:]/)[0]) : VENDOR[p.harness] || p.harness;
      add(vendors, 'v:' + v, { id: 'v:' + v, label: v, tokens: 0 });
      vendors.find((x) => x.id === 'v:' + v).tokens += tk;
      const e = edges.find((x) => x[0] === 't:' + p.harness && x[1] === 'v:' + v);
      if (e) e[3] = Math.max(e[3], rate(tk)); else edges.push(['t:' + p.harness, 'v:' + v, 'c', rate(tk)]);
    }
  }
  const ver = (id) => (S.appVersions || {})[id.slice(2)];
  for (const t of tools) nodes.push({ ...t, col: 1, sub: [ver(t.id), t.open ? t.open + ' open' : '', t.tokens && !ver(t.id) ? fmt(t.tokens) + ' tokens' : ''].filter(Boolean).join(' · ') || 'idle', hot: t.tokens ? 'c' : '', act: 'data-href="#apps"' });
  const nr = (state.routes || []).length, errs = Object.values(live.providers).reduce((a, x) => a + x.errors, 0);
  if (routerUsed || nr) nodes.push({ id: 'router', col: 2, label: 'Local router', sub: `${state.router.running ? ':' + state.router.port : 'stopped'} · ${nr} ${nr === 1 ? 'route' : 'routes'} · ${errs} ${errs === 1 ? 'error' : 'errors'}`, hot: routerUsed && Object.keys(live.providers).length ? 'a' : '', router: true, act: 'data-href="#routing"' });
  for (const v of vendors) nodes.push({ ...v, col: 3, sub: v.tokens ? `${fmt(v.tokens)} tokens` : 'no traffic', tone: v.tokens ? 'ok' : '', act: '' });
  for (const p of provs) nodes.push({ ...p, col: 3, act: 'data-href="#routing"' }); // below the vendors, beside the router
  // place: each column top to bottom, the router in the middle
  // columns spread over the panel's width (at least 600 px): accounts, tools, the router, providers
  const X = ['12px', 'calc(30% - 12px)', 'calc(56% - 40px)', 'calc(100% - 172px)'], W = [160, 128, 150, 160], rows = Math.max(...[0, 1, 3].map((c) => nodes.filter((n) => n.col === c).length), 3);
  const H = rows * 70 + 20;
  for (const c of [0, 1, 3]) {
    const list = nodes.filter((n) => n.col === c), gap = (H - 20 - list.length * 54) / Math.max(1, list.length);
    list.forEach((n, i) => { n.x = X[c]; n.y = 10 + gap / 2 + i * (54 + gap); });
  }
  // the router sits level with the providers it sends to, so the tools' own lines to their providers pass clear of it
  const rt = nodes.find((n) => n.router);
  if (rt) {
    const mine = provs.map((p) => nodes.find((n) => n.id === p.id)).filter(Boolean);
    const mid = mine.length ? mine.reduce((a, n) => a + n.y + 27, 0) / mine.length : H / 2;
    rt.x = X[2]; rt.y = Math.max(10, Math.min(H - 70, mid - 30));
  }
  const tps = Object.values(live.accounts).reduce((a, x) => a + x.tokens, 0) / (live.window_ms / 1000);
  const html = `<div class="topo" data-testid="routes" style="height:${H}px"><canvas aria-hidden="true"></canvas>
    ${nodes.map((n) => `<div class="tn ${n.router ? 'router' : ''} ${n.bad ? 'bad' : ''} ${n.col === 1 ? 'tool' : ''}" data-node="${esc(n.id)}" ${n.hot ? `data-hot="${n.hot}"` : ''} ${n.act} style="left:${n.x};top:${n.y}px;width:${W[n.col]}px;min-height:${n.router ? 64 : 54}px" title="${esc(n.label + (n.sub ? ' · ' + n.sub : ''))}"><b>${esc(n.label)}</b><small class="${n.tone ? n.tone + 'T' : ''}">${esc(n.sub || '')}</small>${n.dots || ''}</div>`).join('')}</div>`;
  return { html, edges, tps, empty: !ps.length, peers };
}

let topoRaf = 0;
/** WebGL: a moving dot grid behind the cards, beads along every route, and pulses as fast as the traffic on it. */
function startTopo(host, edges, still) {
  cancelAnimationFrame(topoRaf);
  const cv = host.querySelector('canvas');
  const gl = cv && cv.getContext('webgl', { alpha: true, premultipliedAlpha: true, antialias: true });
  if (!gl) return; // no WebGL: the cards and the legend still read
  const sh = (type, src) => { const x = gl.createShader(type); gl.shaderSource(x, src); gl.compileShader(x); return x; };
  const prog = (vs, fs) => { const p = gl.createProgram(); gl.attachShader(p, sh(gl.VERTEX_SHADER, vs)); gl.attachShader(p, sh(gl.FRAGMENT_SHADER, fs)); gl.linkProgram(p); return p; };
  const bg = prog('attribute vec2 q;void main(){gl_Position=vec4(q,0.,1.);}',
    'precision mediump float;uniform vec2 r;uniform float t,d;uniform vec3 k;void main(){vec2 p=gl_FragCoord.xy;vec2 g=mod(p,22.*d)-11.*d;float o=smoothstep(1.5*d,.2*d,length(g));float s=exp(-pow((p.x/r.x-fract(t*.06))*7.,2.));float a=o*(.07+.30*s);gl_FragColor=vec4(k*a,a);}');
  const pt = prog('attribute vec2 p;attribute float s;attribute vec4 c;uniform vec2 r;varying vec4 v;void main(){vec2 z=p/r*2.-1.;gl_Position=vec4(z.x,-z.y,0.,1.);gl_PointSize=s;v=c;}',
    'precision mediump float;varying vec4 v;void main(){float q=length(gl_PointCoord-.5)*2.;float a=exp(-q*q*3.5)*(1.-smoothstep(.8,1.,q))*v.a;gl_FragColor=vec4(v.rgb*a,a);}');
  const quad = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, quad);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
  const buf = gl.createBuffer(), A = new Float32Array(7 * 8000), maxPt = gl.getParameter(gl.ALIASED_POINT_SIZE_RANGE)[1], t0 = performance.now();
  const frame = (t) => {
    const th = document.documentElement.dataset.theme, light = th === 'light' || (th === 'system' && window.matchMedia('(prefers-color-scheme: light)').matches);
    const d = Math.min(window.devicePixelRatio || 1, 2), W = host.clientWidth, H = host.clientHeight;
    if (cv.width !== Math.round(W * d) || cv.height !== Math.round(H * d)) { cv.width = Math.round(W * d); cv.height = Math.round(H * d); }
    gl.viewport(0, 0, cv.width, cv.height); gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT); gl.enable(gl.BLEND);
    for (let i = 0; i < 4; i++) gl.disableVertexAttribArray(i);
    gl.useProgram(bg); gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    const q = gl.getAttribLocation(bg, 'q'); gl.enableVertexAttribArray(q); gl.vertexAttribPointer(q, 2, gl.FLOAT, false, 0, 0);
    gl.uniform2f(gl.getUniformLocation(bg, 'r'), cv.width, cv.height); gl.uniform1f(gl.getUniformLocation(bg, 't'), t); gl.uniform1f(gl.getUniformLocation(bg, 'd'), d);
    gl.uniform3fv(gl.getUniformLocation(bg, 'k'), light ? [0.06, 0.43, 0.48] : [0.21, 0.78, 0.77]);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA); gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4); gl.disableVertexAttribArray(q);
    const hr = host.getBoundingClientRect(), k = hr.width / W || 1, box = {};
    host.querySelectorAll('[data-node]').forEach((el) => {
      const r = el.getBoundingClientRect();
      box[el.dataset.node] = { l: ((r.left - hr.left) / k) * d, t: ((r.top - hr.top) / k) * d, w: (r.width / k) * d, h: (r.height / k) * d, hot: el.dataset.hot };
    });
    // teal: logins straight to the provider; iris: through the router; red: paused or logged out; sand: from another computer
    const COL = light ? { c: [0.06, 0.43, 0.48], a: [0.43, 0.36, 0.82], r: [0.86, 0.15, 0.15], p: [0.78, 0.64, 0.35] } : { c: [0.21, 0.78, 0.77], a: [0.62, 0.56, 0.94], r: [0.94, 0.27, 0.27], p: [0.78, 0.64, 0.35] };
    let n = 0;
    const put = (x, y, s, c, a) => { if (n >= 8000) return; const o = n * 7; A[o] = x; A[o + 1] = y; A[o + 2] = Math.min(s, maxPt); A[o + 3] = c[0]; A[o + 4] = c[1]; A[o + 5] = c[2]; A[o + 6] = a; n++; };
    let hi = 0;
    for (const key in box) { const b = box[key]; if (b.hot) put(b.l + b.w / 2, b.t + b.h / 2, Math.max(b.w, b.h) * 1.1, COL[b.hot], (light ? 0.07 : 0.1) + 0.05 * Math.sin(t * 2 + hi++)); }
    for (const e of edges) {
      const a = box[e[0]], b = box[e[1]]; if (!a || !b) continue;
      const x1 = a.l + a.w, y1 = a.t + a.h / 2, x2 = b.l, y2 = b.t + b.h / 2, dx = Math.max(30 * d, (x2 - x1) * 0.5);
      const P = (u) => { const v = 1 - u; return [v * v * v * x1 + 3 * v * v * u * (x1 + dx) + 3 * v * u * u * (x2 - dx) + u * u * u * x2, v * v * v * y1 + 3 * v * v * u * y1 + 3 * v * u * u * y2 + u * u * u * y2]; };
      const c = COL[e[2]], broken = e[2] === 'r', beads = Math.max(10, Math.round(Math.hypot(x2 - x1, y2 - y1) / (6 * d)));
      for (let i = 0; i <= beads; i++) { const p = P(i / beads); put(p[0], p[1], 2.6 * d, c, broken ? 0.22 + 0.2 * Math.sin(t * 5) : e[3] ? (light ? 0.45 : 0.3) : 0.14); }
      for (let i = 0; i < e[3]; i++) {
        const u = (t * (0.1 + e[3] * 0.012) + i / e[3]) % 1;
        for (let j = 0; j < 5; j++) { const uu = u - j * 0.01; if (uu < 0) continue; const p = P(uu); put(p[0], p[1], (8 - j * 1.4) * d, c, 0.95 - j * 0.17); }
      }
    }
    gl.useProgram(pt); gl.bindBuffer(gl.ARRAY_BUFFER, buf); gl.bufferData(gl.ARRAY_BUFFER, A.subarray(0, n * 7), gl.DYNAMIC_DRAW);
    const Pa = gl.getAttribLocation(pt, 'p'), Sa = gl.getAttribLocation(pt, 's'), Ca = gl.getAttribLocation(pt, 'c');
    gl.enableVertexAttribArray(Pa); gl.vertexAttribPointer(Pa, 2, gl.FLOAT, false, 28, 0);
    gl.enableVertexAttribArray(Sa); gl.vertexAttribPointer(Sa, 1, gl.FLOAT, false, 28, 8);
    gl.enableVertexAttribArray(Ca); gl.vertexAttribPointer(Ca, 4, gl.FLOAT, false, 28, 12);
    gl.uniform2f(gl.getUniformLocation(pt, 'r'), cv.width, cv.height);
    gl.blendFunc(gl.ONE, light ? gl.ONE_MINUS_SRC_ALPHA : gl.ONE); gl.drawArrays(gl.POINTS, 0, n);
  };
  const reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  let drawnFor = '';
  const loop = (now) => {
    if (!host.isConnected) return; // the page moved on
    const look = document.documentElement.dataset.theme + host.clientWidth;
    // a still picture (setting, reduced motion, or a hidden window) is drawn once per look
    if (!(still || reduce || document.hidden) || drawnFor !== look) { frame(still || reduce ? 4 : (now - t0) / 1000); drawnFor = look; }
    topoRaf = requestAnimationFrame(loop);
  };
  topoRaf = requestAnimationFrame(loop);
}

// ---------- limits ----------
/** Every limit with a window, tightest first, with its forecast at the reset. */
function burnRows(ps, d) {
  const out = [];
  for (const p of ps) for (const t of (p.quota && p.quota.tiers) || []) {
    if (!t.resets_at || t.resets_at <= d.now) continue;
    const f = d.forecast[p.name + '|' + t.id];
    const left = Math.max(0, Math.round(100 - t.used));
    const note = f && f.runs_out && f.runs_out < t.resets_at ? `Runs out ${when(f.runs_out)}, ${dur(t.resets_at - f.runs_out)} before the reset.`
      : f ? `Resets in ${dur(t.resets_at - d.now)}; lasts until then at this pace.` : `Resets in ${dur(t.resets_at - d.now)}.`;
    out.push({ name: p.label, win: t.label, used: Math.min(100, t.used), left, at: f ? Math.min(99.5, f.at_reset) : null, note, out: f && f.runs_out && f.runs_out < t.resets_at ? f.runs_out : null, reset: t.resets_at });
  }
  // routed accounts with a daily spend cap, which the router enforces
  const midnight = new Date(d.now); midnight.setHours(24, 0, 0, 0);
  for (const p of ps) {
    const c = d.caps[p.name];
    if (!c) continue;
    const used = Math.min(100, (c.spent / c.cap) * 100), left = Math.max(0, Math.round(100 - used));
    out.push({ name: p.label, win: `$${c.cap.toFixed(2)} daily cap`, used, left, at: null, out: null, reset: midnight.getTime(),
      note: `${money(c.spent)} of ${money(c.cap)} today. The router stops at the cap until midnight.` });
  }
  return out.sort((a, b) => a.left - b.left);
}
const tone = (left) => (left < 20 ? 'neg' : left < 45 ? 'warn' : '');

/** One panel for every limit: a bar from now to its reset on a seven-day line, filled by what is used, amber from
 * where it runs out at this pace. Accounts whose limits are not read yet are listed without a bar. */
function limitsPanel(burn, ps, now) {
  const DAY = 86400000, pct = (ts) => Math.min(100, Math.max(0, ((ts - now) / (7 * DAY)) * 100));
  const days = Array.from({ length: 7 }, (_, i) => new Date(now + i * DAY).toLocaleDateString([], { weekday: 'short' }));
  const leftOf = (b) => (b.left === 0 ? 'used up' : b.left + '% left');
  const rows = burn.map((b) => {
    // the number sits in the bar when it has room; a bar near the week's end carries its label on its left
    const w = pct(b.reset), inner = w > 14 && w <= 72, outP = b.out ? Math.round(((b.out - now) / (b.reset - now)) * 100) : null;
    const text = `${inner ? '' : leftOf(b) + ' · '}${b.out ? 'runs out ' + when(b.out) + ' · ' : ''}resets ${when(b.reset)}`;
    return `<div class="limw"><b>${esc(b.name)}</b><span class="mono muted">${esc(b.win)}</span></div>
      <div class="limr" title="${esc(b.note)}"><div class="limb" style="width:${Math.max(w, 1.5).toFixed(1)}%"><i class="${tone(b.left)}" style="width:${Math.round(b.used)}%"></i>${outP != null ? `<i class="out" style="left:${outP}%;width:${100 - outP}%"></i><em style="left:${outP}%"></em>` : ''}${inner ? `<span>${leftOf(b)}</span>` : ''}</div><span class="limt ${b.out ? 'warnT' : ''}" style="${w > 72 ? `right:calc(${(100 - w).toFixed(1)}% + 8px)` : `left:calc(${w.toFixed(1)}% + 8px)`}">${text}</span></div>`;
  }).concat(limitRows(ps).filter((r) => r.note).map((r) => `<div class="limw"><b>${esc(r.name)}</b></div><div class="limr"><span class="limt muted" style="left:8px">${esc(r.note)}</span></div>`)).join('');
  return `<section class="panel" data-testid="limits"><div class="ph"><h2>Limits</h2><span class="muted sm">each bar runs from now to its reset · the fill is what is used · amber from where it runs out at this pace</span></div>
    ${rows ? `<div class="limax">${days.map((d, i) => `<span>${esc(d)}${i ? '' : ' now'}</span>`).join('')}</div><div class="limg">${rows}</div>` : '<p class="empty">Limits show once an account with a plan has run on this computer.</p>'}</section>`;
}

// ---------- the page ----------
/** The Running now panel: every session open on any computer (live.running), with the computer's name. */
const runningNow = (running, pcs, now) => (running.length ? `<section class="panel" data-testid="running-now"><div class="ph"><h2>Running now</h2><span class="muted mono sm">${running.length} ${running.length === 1 ? 'session' : 'sessions'} on your computers</span></div>
      ${running.map((r) => `<div class="runrow">${cdot(pcs, r.device)}<b>${esc(r.project_name || r.project || (r.cwd || '').split(/[\\/]/).filter(Boolean).pop() || 'no folder')}</b><span class="muted">${esc(nameOf(pcs, r.device))} · ${esc(acctName(r.profile))} · ${esc(TOOL[r.harness] || r.harness || '')}</span><span class="mono muted">${r.started_at ? dur(now - r.started_at) : ''}</span></div>`).join('')}</section>` : '');
// presence from a connected computer arrives every 10 s: only the Running now panel is drawn again, in place
document.addEventListener('presence', async (e) => {
  const sec = document.querySelector('[data-testid=running-now]');
  if (S.route.view !== 'home' || !sec || document.getElementById('dialog-root').innerHTML) return;
  const d = await call('live').catch(() => null);
  if (d && S.route.view === 'home' && sec.isConnected) sec.outerHTML = runningNow(d.live.running || [], e.detail, d.now) || '<span data-testid="running-now" hidden></span>';
});
view('home', async (sheet) => {
  sheet.className = 'sheet';
  const hours = RANGES.find((r) => r[0] === st.range)[1];
  const days = Math.round(hours / 24);
  // scope: every computer added up, or this one only; the sync core answers the same shape either way
  const [state, d, pcs] = await Promise.all([loadState(), call('home', { days, hours, harness: st.harness, limit: 50, scope: scope() }), computersStatus().catch(() => null)]);
  st.rows = d.requests.rows;
  st.total = d.requests.total;
  const o = d.overview, k = d.dash, s = state.settings;
  if (!st.harness) setTodayBadge(o.today_cost);
  const shown = shownTools(state), ps = state.profiles.filter((p) => shown.some((h) => h.id === p.harness)), now = k.now;
  const { best, text } = findings(ps, now);
  const burn = burnRows(ps, k), risks = burn.filter((b) => b.out).sort((a, b) => a.out - b.out);
  const tools = state.harnesses.filter((h) => shown.some((x) => x.id === h.id));
  const logged = ps.filter((p) => p.identity.logged_in).length, open = ps.reduce((a, p) => a + p.running.length, 0);
  // the tools' versions, read once (each tool is asked for its version), then the cards are drawn again
  if (!S.appVersions) {
    S.appVersions = {};
    call('apps').then((a) => { S.appVersions = Object.fromEntries(a.apps.filter((x) => x.version).map((x) => [x.id, x.version])); if (S.route.view === 'home') render(); }).catch(() => {});
  }
  const topo = topology(state, k, pcs);
  // the Activity grid: each day's tokens by computer (activity.days[*].by_device), and the switch when there are peers
  const byDay = Object.fromEntries((((d.activity || {}).days) || []).filter((x) => x.by_device).map((x) => [x.day, x.by_device]));
  const pc = topo.peers ? { c: pcs, by: byDay, dev: computers(pcs).some((x) => x.id === st.device) ? st.device : '' } : null;
  const running = topo.peers ? (k.live.running || []) : [];
  sheet.innerHTML = `
    <div class="trow ovhead"><div>
        <div class="live"><i></i>Live · ${esc(state.computer || 'this computer')} · ${ps.length} accounts · ${logged} logged in · ${open ? `<a href="#sessions?live=1" class="acT" data-testid="home-live">${open} ${open === 1 ? 'console' : 'consoles'} open</a>` : '0 consoles open'} · router :${state.router.port} ${state.router.running ? 'running' : 'stopped'}</div>
        <h1>Overview</h1><p class="find" data-testid="finding">${text}${risks[0] ? ` <b class="warnT">${esc(risks[0].name)}</b> runs out of its ${esc(risks[0].win.toLowerCase())} ${when(risks[0].out)} at this pace.` : ''}</p></div>
      <div class="acts">
        <div class="toolfilter" role="group" aria-label="Tool"><a class="${!st.harness ? 'on' : ''}" data-act="home-tool" data-v="" data-testid="tool-all">All</a>
          ${tools.map((h) => `<a class="${st.harness === h.id ? 'on' : ''}" data-act="home-tool" data-v="${h.id}" data-testid="tool-${h.id}">${esc(h.label)}</a>`).join('')}</div>
        ${syncOn(pcs) ? scopeSeg() : ''}<span class="seg3" role="group" aria-label="Time range">${RANGES.map(([r]) => `<a class="${st.range === r ? 'on' : ''} mono" data-act="home-range" data-v="${r}" data-testid="range-${r}">${r}</a>`).join('')}</span>
        ${best ? `<button class="btn primary" data-act="start" data-name="${esc(best.name)}" data-testid="start-best">${ic.play} Start ${esc(best.label)}</button>` : ''}</div></div>
    ${kpis(k, ps, risks, d, pcs)}
    <section class="panel ovroutes"><div class="ph"><h2>Live routes <span class="mono acT">${topo.tps >= 1 ? Math.round(topo.tps) : topo.tps.toFixed(1)} tok/s now</span></h2><span class="muted mono sm">particles = tokens per second, last 5 minutes, updated each minute</span></div>
      ${topo.empty ? '<p class="empty">Add an account to see its route.</p>' : `<div class="topowrap">${topo.html}</div>`}
      <div class="legend"><span><i class="lc"></i>Subscription login, straight to the provider</span><span><i class="la"></i>API key, through the local router</span><span><i class="lr"></i>Paused, or a login that needs renewing</span>${topo.peers ? `<span><i class="lp"></i>Traffic from another computer</span><span>${cdots(pcs, computers(pcs).map((x) => x.id))} in use on ${esc(computers(pcs).map((x) => x.name).join(', '))}</span>` : ''}<span>Click a card to open it</span></div></section>
    ${runningNow(running, pcs, now)}
    ${limitsPanel(burn, ps, now)}
    <section class="panel" data-testid="activity"><div class="ph"><h2>Activity, 53 weeks</h2><span class="acts">${pc ? `<span class="seg3" role="group" aria-label="Computer" data-testid="activity-pc">${[['', 'All'], ...computers(pcs).map((x) => [x.id, x.name])].map(([v, l]) => `<a class="${pc.dev === v ? 'on' : ''}" data-act="home-device" data-v="${esc(v)}" data-testid="activity-pc-${esc(v || 'all')}">${esc(l)}</a>`).join('')}</span>` : ''}<span class="muted mono sm">daily ${pc && pc.dev ? 'tokens on ' + esc(nameOf(pcs, pc.dev)) : (st.metric === 'cost' ? 'cost' : st.metric) + (pc ? ' · all computers' : '')}</span></span></div><div class="hmwrap"></div>${busiest(o.heatmap, st.metric)}</section>
    <section class="panel" style="padding-bottom:6px"><div class="ph"><h2>${st.log ? 'Request log' : 'Latest replies'} <span class="muted mono sm">${d.requests.total.toLocaleString()}</span></h2>
        <a class="acT sm linkb" data-act="home-log" data-testid="request-log-open">${st.log ? 'Latest replies' : 'Request log'}</a></div>
      ${st.log ? `<p class="hint">Costs are what these tokens would cost at API list prices (models.dev). Subscriptions are not billed per token.</p>
      <div class="tscroll" id="home-table">${requestTable(st.rows)}</div>
      ${st.rows.length < st.total ? '<div style="padding:10px 0"><button class="btn outline sm" data-act="home-more" data-testid="home-more">Show 50 more</button></div>' : ''}` : latestTable(st.rows)}</section>`;
  fit(sheet.querySelector('.hmwrap'), (w) => heatmap(o.heatmap, st.metric, w, pc));
  const host = sheet.querySelector('.topo');
  if (host) startTopo(host, topo.edges, !s.moving_routes);
});

action('home-metric', (el) => { st.metric = el.dataset.v; render(); });
action('home-device', (el) => { st.device = el.dataset.v; render(); });
action('home-log', () => { st.log = !st.log; render(); });
action('home-tool', (el) => { st.harness = el.dataset.v; render(); });
action('home-range', (el) => { st.range = el.dataset.v; render(); });
action('home-more', async (el) => {
  if (el.disabled) return;
  el.disabled = true;
  const base = st.rows;
  const days = Math.round(RANGES.find((r) => r[0] === st.range)[1] / 24);
  const r = await attempt(() => call('requests', { days, hours: RANGES.find((x) => x[0] === st.range)[1], harness: st.harness, limit: 50, offset: base.length }));
  el.disabled = false;
  if (!r || st.rows !== base) return; // the list was redrawn (another filter) while this page loaded
  st.rows = base.concat(r.rows);
  document.getElementById('home-table').innerHTML = requestTable(st.rows);
  if (st.rows.length >= r.total) document.querySelector('[data-act=home-more]')?.remove();
});
action('home-sync', async () => {
  const r = await attempt(() => call('refresh', { live: S.state && S.state.live_quota }));
  if (r) { S.lastSync = Date.now(); toast(refreshText(r)); changed(); }
});
// cards that open another page
document.addEventListener('click', (e) => { const a = e.target.closest('[data-href]'); if (a) location.hash = a.dataset.href; });
