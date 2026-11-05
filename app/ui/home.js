// Overview: the finding leads, the Horizon Dial shows every route a token takes (computers at the docks, accounts,
// tools, the router gate, providers), the year clock and what needs attention sit beside it, and below: every limit
// as a ring, what runs now, the latest replies or the request log. Measured numbers are plain; estimates carry ≈.

import { S, call, view, render, attempt, changed, toast, loadState, action, acctName, fit, refreshText, shownTools, warmPage, keepPage } from './core.js';
import { esc, fmt, hm, dur, toolIcon, toolPill } from './lib.js';
import { findings, limitRows } from './accounts.js';
import { computersStatus, syncOn, computers, nameOf, cdot, scope, scopeSeg } from './computers.js';

const TOOL = { claude: 'Claude Code', codex: 'Codex', opencode: 'OpenCode', agy: 'Antigravity', pi: 'Pi', omp: 'Oh My Pi' };
const VENDOR = { claude: 'Anthropic', codex: 'OpenAI', opencode: 'OpenCode', agy: 'Google', pi: 'Pi', omp: 'Oh My Pi' };
const RANGES = [['today', 0], ['7d', 168], ['30d', 720], ['all', 24 * 400]], RANGE_LABEL = { today: 'Today', '7d': '7 d', '30d': '30 d', all: 'All' };
// device: the year clock's own computer ('' is every computer added up)
const st = { harness: '', range: 'today', metric: 'tokens', rows: null, total: 0, log: false, device: '' };
const money = (c, digits = 2) => (c == null ? '—' : '$' + c.toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits }));
const secs = (ms) => (ms == null ? '—' : (ms / 1000).toFixed(1));
const when = (ms) => new Date(ms).toLocaleString([], { weekday: 'short', hour: '2-digit', minute: '2-digit', hour12: false });
const ago = (ms) => (ms < 60000 ? 'just now' : dur(ms) + ' ago');
const PI = Math.PI, deg = (d) => (d * PI) / 180;
const SVGNS = 'http://www.w3.org/2000/svg';
const el = (tag, attrs = {}, parent) => { const e = document.createElementNS(SVGNS, tag); for (const k in attrs) e.setAttribute(k, attrs[k]); if (parent) parent.append(e); return e; };
const h = (html) => { const t = document.createElement('template'); t.innerHTML = html.trim(); return t.content.firstElementChild; };
// a computer's colour, as its dot has it: this one teal, then sand, iris, amber
const PC_COL = ['var(--ac)', 'var(--sand)', 'var(--r2)', 'var(--warn)'];
const pcCol = (pcs, id) => PC_COL[Math.max(0, computers(pcs).findIndex((d) => d.id === id)) % 4];

// ---------- the year clock: 53 weeks around, Monday inside to Sunday outside ----------
// pc: the computers and each day's tokens by computer (by[day] = {id: tokens}); dev: one computer's tokens only
function yearClock(days, metric, pc = null) {
  const byDev = (d) => (pc && pc.by[d.day]) || null;
  const val = (d) => (pc && pc.dev ? (byDev(d) || {})[pc.dev] || 0 : metric === 'cost' ? d.cost : metric === 'requests' ? d.requests : d.tokens);
  const nz = days.map(val).filter((v) => v > 0).sort((a, b) => a - b);
  const q = (p) => nz[Math.min(nz.length - 1, Math.floor(p * nz.length))];
  const cuts = nz.length ? [q(0.25), q(0.5), q(0.75)] : [], top1 = nz[nz.length - 1];
  const level = (v) => (v <= 0 || !nz.length ? 0 : v >= top1 ? 4 : v <= cuts[0] ? 1 : v <= cuts[1] ? 2 : v <= cuts[2] ? 3 : 4), op = [0, 0.28, 0.5, 0.75, 1];
  const S = 300, c = 150, r0 = 58, r1 = 138, band = (r1 - r0) / 7;
  const first = new Date(days[0].day + 'T12:00'), off = (first.getDay() + 6) % 7, weeks = Math.ceil((days.length + off) / 7);
  const P = (r, a) => `${(c + r * Math.cos(a)).toFixed(2)} ${(c + r * Math.sin(a)).toFixed(2)}`;
  let s = `<svg viewBox="0 0 ${S} ${S}" role="img" aria-label="Daily usage over the last 53 weeks, as a year clock" data-testid="heatmap">`;
  let lastM = -1;
  const best = days.reduce((a, d) => (val(d) > val(a) ? d : a), days[0]);
  days.forEach((d, i) => {
    const date = new Date(d.day + 'T12:00'), w = Math.floor((i + off) / 7), r = (i + off) % 7;
    const a0 = (w / weeks) * 2 * PI - PI / 2 + 0.004, a1 = ((w + 1) / weeks) * 2 * PI - PI / 2 - 0.004, ra = r0 + r * band + 0.6, rb = r0 + (r + 1) * band - 0.6, l = level(val(d));
    const split = byDev(d) && Object.keys(byDev(d)).length > 1 ? ' · ' + Object.entries(byDev(d)).filter(([, t]) => t).map(([id, t]) => `${nameOf(pc.c, id)} ${fmt(t)}`).join(' · ') : '';
    const label = `${date.toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' })}: ${pc && pc.dev ? `${fmt(val(d))} tokens on ${nameOf(pc.c, pc.dev)}` : `${fmt(d.tokens)} tokens · ${d.requests} requests · ${money(d.cost)}`}${split}`;
    s += `<path class="hm" data-l="${l}" d="M${P(ra, a0)} A${ra} ${ra} 0 0 1 ${P(ra, a1)} L${P(rb, a1)} A${rb} ${rb} 0 0 0 ${P(rb, a0)} Z" fill="${l ? 'var(--ac)' : 'var(--hm0)'}" fill-opacity="${l ? op[l] : 1}"${d === best && val(best) ? ' stroke="var(--ink)" stroke-width="1.2"' : ''}><title>${esc(label)}</title></path>`;
    if (r === 0 && date.getMonth() !== lastM) { lastM = date.getMonth(); const am = (w / weeks) * 2 * PI - PI / 2; s += `<text x="${(c + (r1 + 9) * Math.cos(am)).toFixed(1)}" y="${(c + (r1 + 9) * Math.sin(am) + 3).toFixed(1)}" font-size="8.5" text-anchor="middle" fill="var(--m)" font-family="var(--mono)">${date.toLocaleDateString([], { month: 'short' })[0]}</text>`; }
  });
  const bv = val(best);
  s += `<text x="${c}" y="${c - 8}" text-anchor="middle" font-size="11" fill="var(--m)" font-family="var(--sans)">busiest day</text><text x="${c}" y="${c + 10}" text-anchor="middle" font-size="15" font-weight="600" fill="var(--ink)" font-family="var(--sans)">${bv ? new Date(best.day + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short' }) : '—'}</text><text x="${c}" y="${c + 25}" text-anchor="middle" font-size="10.5" fill="var(--m)" font-family="var(--mono)">${bv ? (metric === 'cost' && !(pc && pc.dev) ? money(bv) : metric === 'requests' && !(pc && pc.dev) ? bv.toLocaleString() + ' requests' : fmt(bv)) : 'no usage yet'}</text>`;
  return s + '</svg>';
}

// ---------- the readouts: a dot-matrix trend, the number, the change against the period before ----------
function dots(vals, color, cols = 10, rows = 3) {
  const v = (vals || []).map((x) => x || 0), max = Math.max(...v, 1e-9), w = cols * 6, hh = rows * 6;
  let s = `<svg width="${w}" height="${hh}" viewBox="0 0 ${w} ${hh}" aria-hidden="true">`;
  v.slice(-cols).forEach((x, c) => { const lit = x > 0 ? Math.max(1, Math.round((x / max) * rows)) : 0; for (let r = 0; r < rows; r++) s += `<circle cx="${c * 6 + 3}" cy="${hh - r * 6 - 3}" r="1.7" fill="${r < lit ? color : 'var(--line)'}"/>`; });
  return s + '</svg>';
}
function change(cur, prev, kind) {
  if (cur == null || prev == null) return ['—', 'flat'];
  if (kind === 'pct') { if (!prev) return [cur ? 'new' : '—', 'flat']; const d = Math.round(((cur - prev) / prev) * 100); return [(d > 0 ? '+' : d < 0 ? '−' : '±') + Math.abs(d) + '%', d > 0 ? 'up' : d < 0 ? 'down' : 'flat']; }
  if (kind === 'money') { const d = cur - prev; return [(d >= 0 ? '+' : '−') + money(Math.abs(d), 0), d > 0 ? 'warn' : 'flat']; }
  const d = Math.round((cur - prev) * 100); return [(d >= 0 ? '+' : '−') + Math.abs(d) + ' pt', d > 0 ? 'up' : d < 0 ? 'warn' : 'flat'];
}
function readouts(d) {
  const c = d.cur, p = d.prev, sp = d.spark || {}, fresh = !p.requests || st.range === 'all';
  const vs = (...a) => (fresh ? ['—', 'flat'] : change(...a));
  const rd = (key, label, value, [delta, tone], vals, color, click) => `<div class="rd ${tone} ${st.metric === key ? 'on' : ''}" ${click ? `data-act="home-metric" data-v="${key}" role="button" tabindex="0"` : ''} data-testid="kpi-${key}"><label>${label}</label><strong>${value}</strong>${dots(vals, color)}<b class="d ${tone === 'warn' ? 'warn' : ''}">${esc(delta)}</b></div>`;
  return `<div class="rds"><div class="rng" role="tablist" aria-label="Range">${RANGES.map(([r]) => `<button role="tab" aria-selected="${st.range === r}" class="${st.range === r ? 'on' : ''}" data-act="home-range" data-v="${r}" data-testid="range-${r}">${RANGE_LABEL[r]}</button>`).join('')}</div>
    ${rd('tokens', 'Tokens', fmt(c.tokens), vs(c.tokens, p.tokens, 'pct'), sp.tokens, 'var(--ac)', true)}
    ${rd('cost', 'Cost', money(c.cost, c.cost >= 100 ? 0 : 2), vs(c.cost, p.cost, 'money'), sp.cost, 'var(--warn)', true)}
    ${rd('hit', 'Cached', `${Math.round(c.hit * 100)}<small>%</small>`, vs(c.hit, p.hit, 'pt'), (sp.hit || []).map((x) => Math.max(0, x - 0.5)), 'var(--ac)', false)}</div>`;
}

// ---------- the graph: every hop a token takes, from the accounts as they are ----------
// pcs: the computers; with peers, an account's dots say where it runs and traffic from elsewhere is drawn in that colour
function graph(state, d, pcs) {
  const ps = state.profiles.filter((p) => shownTools(state).some((h) => h.id === p.harness)).slice(0, 12);
  const live = d.live, breakers = live.breakers, now = d.now;
  const peers = syncOn(pcs) && pcs.peers.length > 0;
  const me = pcs ? pcs.this.id : 'this';
  const pcList = peers ? computers(pcs) : [{ id: me, name: (pcs && pcs.this.name) || state.computer || 'this computer' }];
  const peerRuns = (p) => (peers ? (live.running || []).filter((r) => r.profile === p.name && r.device && r.device !== me) : []);
  const paused = (id) => breakers[id] && breakers[id].state === 'open' && (!breakers[id].retry_at || breakers[id].retry_at > now);
  const N = [], E = [], add = (x) => { if (!N.some((n) => n.id === x.id)) N.push(x); return N.find((n) => n.id === x.id); };
  for (const c of pcList) N.push({ id: c.id, kind: 'pc', label: c.name, sub: c.id === me ? 'this computer' : '', col: peers ? pcCol(pcs, c.id) : 'var(--ac)' });
  let routerUsed = false;
  for (const p of ps) {
    const tk = (live.accounts[p.name] || {}).tokens || 0, routed = p.kind === 'routed';
    const tiers = (p.quota && p.quota.tiers) || [];
    const win = (t) => { const f = d.forecast[p.name + '|' + t.id], out = f && f.runs_out && f.runs_out < t.resets_at ? f.runs_out : null; return { label: t.label, used: Math.min(100, t.used), reset: t.resets_at, at: f ? Math.min(100, f.at_reset) : null, out }; };
    const fh = tiers.find((t) => t.id === 'session'), wk = tiers.find((t) => t.id === 'week') || tiers.find((t) => t.id.startsWith('week'));
    const windows = fh || wk ? [win(fh || wk), win(wk || fh)] : null;
    const cap = d.caps[p.name] ? { cap: d.caps[p.name].cap, spent: d.caps[p.name].spent } : null;
    const needsLogin = p.kind === 'subscription' && (p.identity.needs_login || !p.identity.logged_in);
    const sub = needsLogin ? 'login needed' : cap ? `${money(cap.spent)} of ${money(cap.cap)} today` : windows ? `${(wk || fh).label.toLowerCase()} ${Math.round((windows[1].used))}%${windows[1].out ? ' · ≈ out ' + when(windows[1].out) : ''}` : p.kind === 'api_key' ? 'API key, no limit' : '';
    const far = peerRuns(p), on = [...new Set([...(p.running.length || !far.length ? [me] : []), ...far.map((r) => r.device)])].filter((id) => pcList.some((c) => c.id === id));
    const acct = { needsLogin, cap, windows, kind: routed ? 'routed' : p.kind };
    add({ id: 'a:' + p.name, kind: 'account', label: p.label, sub, tone: needsLogin ? 'neg' : windows && windows.some((w) => w.out) ? 'warn' : '', k: needsLogin ? 'r' : routed ? 'a' : 'c', rate: tk, on, acct, tool: p.harness, href: `#account?name=${encodeURIComponent(p.name)}` });
    for (const pc of on) E.push({ a: pc, b: 'a:' + p.name, k: 'pc:' + pc, rate: tk / on.length });
    const t = add({ id: 't:' + p.harness, kind: 'tool', label: TOOL[p.harness] || p.harness, sub: '', rate: 0, open: 0, harness: p.harness, href: '#apps' });
    t.rate += tk; t.open += p.running.length + far.length;
    E.push({ a: 'a:' + p.name, b: 't:' + p.harness, k: needsLogin ? 'r' : routed ? 'a' : 'c', rate: tk });
    if (routed) {
      routerUsed = true;
      if (!E.some((e) => e.a === 't:' + p.harness && e.b === 'router')) E.push({ a: 't:' + p.harness, b: 'router', k: 'a', rate: tk });
      const r = (state.routes || []).find((x) => x.id === p.route);
      for (const pid of (r ? r.providers : [])) {
        const lp = live.providers[pid], b = breakers[pid], bad = paused(pid);
        add({ id: 'p:' + pid, kind: 'provider', label: pid, k: bad ? 'r' : 'a', via: 'router', rate: (lp || {}).tokens || 0, tone: bad ? 'neg' : lp ? 'ok' : '',
          sub: bad ? `paused${b.retry_at ? ' · retry in ' + dur(b.retry_at - now) : ''}` : lp && lp.first_ms != null ? `≈ ${secs(lp.first_ms)} s first chunk` : lp ? `${fmt(lp.tokens)} tokens` : 'no traffic', href: '#routing' });
        if (!E.some((e) => e.a === 'router' && e.b === 'p:' + pid)) E.push({ a: 'router', b: 'p:' + pid, k: bad ? 'r' : 'a', rate: (lp || {}).tokens || 0 });
      }
    } else {
      const v = p.kind === 'api_key' && p.base_url ? p.base_url.replace(/^https?:\/\//, '').split(/[/:]/)[0] : VENDOR[p.harness] || p.harness;
      const vn = add({ id: 'v:' + v, kind: 'provider', label: v, k: 'c', via: 'direct', rate: 0, sub: '', tone: '' });
      vn.rate += tk; vn.sub = vn.rate ? `${fmt(vn.rate)} tokens, 5 min` : 'no traffic'; vn.tone = vn.rate ? 'ok' : '';
      const e = E.find((x) => x.a === 't:' + p.harness && x.b === 'v:' + v);
      if (e) e.rate += tk; else E.push({ a: 't:' + p.harness, b: 'v:' + v, k: 'c', rate: tk });
    }
  }
  const ver = (id) => (S.appVersions || {})[id];
  for (const t of N.filter((n) => n.kind === 'tool')) t.sub = [ver(t.harness), t.open ? t.open + ' open' : ''].filter(Boolean).join(' · ');
  const nr = (state.routes || []).length, errs = Object.values(live.providers).reduce((a, x) => a + x.errors, 0);
  if (routerUsed || nr) N.push({ id: 'router', kind: 'router', label: 'Local router', sub: `${state.router.running ? ':' + state.router.port : 'stopped'} · ${nr} ${nr === 1 ? 'route' : 'routes'} · ${errs} ${errs === 1 ? 'error' : 'errors'}`, k: 'a', rate: 0, tone: errs ? 'warn' : '', href: '#routing' });
  const by = Object.fromEntries(N.map((n) => [n.id, n]));
  return { N, E: E.filter((e) => by[e.a] && by[e.b]), by, empty: !ps.length, peers, pcList };
}
// what a body touches: everything upstream and downstream of it
function chain(E, id) {
  const lit = new Set([id]);
  const walk = (dir) => { const q = [id]; while (q.length) { const x = q.shift(); for (const e of E) { const [a, b] = dir ? [e.a, e.b] : [e.b, e.a]; if (a === x && !lit.has(b)) { lit.add(b); q.push(b); } } } };
  walk(true); walk(false);
  return lit;
}
const edgeCol = (k, G) => (k.startsWith('pc:') ? (G.by[k.slice(3)] || {}).col || 'var(--ac)' : k === 'a' ? 'var(--r2)' : k === 'r' ? 'var(--neg)' : 'var(--ac)');
const nodeCol = (n, G) => (n.kind === 'pc' ? n.col : n.kind === 'tool' ? 'var(--m)' : n.kind === 'router' ? 'var(--r2)' : n.k === 'r' ? 'var(--neg)' : n.kind === 'provider' && !n.rate ? 'var(--m)' : edgeCol(n.k || 'c', G));
const arcPath = (cx, cy, r, f0, f1) => { if (f1 - f0 >= 0.999) f1 = f0 + 0.999; const a0 = f0 * 2 * PI - PI / 2, a1 = f1 * 2 * PI - PI / 2; return `M${cx + r * Math.cos(a0)} ${cy + r * Math.sin(a0)} A${r} ${r} 0 ${f1 - f0 > 0.5 ? 1 : 0} 1 ${cx + r * Math.cos(a1)} ${cy + r * Math.sin(a1)}`; };
const tone = (u) => (u >= 80 ? 'var(--neg)' : u >= 55 ? 'var(--warn)' : 'var(--ac)');
/** Two limit arcs around an account: inner 5-hour, outer weekly; solid is used, dashed the estimate at the reset. */
function limitArcs(g, x, y, a, R = 13) {
  const col = (w) => (w.out ? 'var(--warn)' : a.kind === 'routed' ? 'var(--r2)' : 'var(--ac)');
  if (a.needsLogin) { el('circle', { cx: x, cy: y, r: R, fill: 'none', stroke: 'var(--neg)', 'stroke-dasharray': '2 4', opacity: .8 }, g); return; }
  if (a.cap) { const u = Math.min(1, a.cap.spent / a.cap.cap); el('circle', { cx: x, cy: y, r: R, fill: 'none', stroke: 'var(--line)', 'stroke-width': 2.4 }, g); el('path', { d: arcPath(x, y, R, 0, u), fill: 'none', stroke: 'var(--r2)', 'stroke-width': 2.4, 'stroke-linecap': 'round' }, g); return; }
  if (!a.windows) { el('circle', { cx: x, cy: y, r: R, fill: 'none', stroke: 'var(--line)', 'stroke-dasharray': '1 3' }, g); return; }
  a.windows.forEach((w, i) => {
    const r = i ? R : R - 4, u = w.used / 100, f = Math.min(1, (w.at != null ? w.at : w.used) / 100);
    el('circle', { cx: x, cy: y, r, fill: 'none', stroke: 'var(--line)', 'stroke-width': 2.2 }, g);
    if (f > u) el('path', { d: arcPath(x, y, r, u, f), fill: 'none', stroke: col(w), 'stroke-width': 2.2, 'stroke-dasharray': '1.5 2.5', opacity: .6 }, g);
    if (u > 0) el('path', { d: arcPath(x, y, r, 0, u), fill: 'none', stroke: col(w), 'stroke-width': 2.2, 'stroke-linecap': 'round' }, g);
  });
}
/** A body on the map: limit arcs for an account, a ring for a tool, a gate for the router, a bar for a computer's dock. */
function body(svg, n, x, y, G) {
  const g = el('g', { class: 'n', 'data-node': n.id, tabindex: 0, role: 'button', 'aria-label': `${n.label}${n.sub ? ', ' + n.sub : ''}` }, svg);
  if (n.href) g.dataset.href = n.href;
  n.x = x; n.y = y;
  const col = nodeCol(n, G);
  if (n.kind === 'pc') { el('rect', { x: x - 22, y: y - 3, width: 44, height: 6, rx: 2, fill: col, opacity: .55 }, g); el('rect', { x: x - 6, y: y - 6, width: 12, height: 12, rx: 3, fill: col }, g); }
  else if (n.kind === 'tool') el('circle', { cx: x, cy: y, r: 5.5, fill: 'var(--s)', stroke: 'var(--t2)', 'stroke-width': 1.5 }, g);
  else if (n.kind === 'router') { el('rect', { x: x - 9, y: y - 9, width: 18, height: 18, rx: 4, fill: 'var(--s)', stroke: 'var(--r2)', 'stroke-width': 1.6 }, g); el('circle', { cx: x, cy: y, r: 3, fill: 'var(--r2)' }, g); }
  else if (n.kind === 'account') { el('circle', { cx: x, cy: y, r: 16, fill: 'var(--s)', opacity: .9 }, g); limitArcs(g, x, y, n.acct); el('circle', { cx: x, cy: y, r: 5, fill: col }, g); }
  else { const r = n.rate ? 7 : 5; el('circle', { cx: x, cy: y, r: r + 5, fill: col, opacity: n.rate ? .18 : 0 }, g); el('circle', { cx: x, cy: y, r, fill: n.rate || n.k === 'r' ? col : 'var(--s)', stroke: col, 'stroke-width': 1.5, 'stroke-dasharray': n.k === 'r' ? '2 2' : '' }, g); }
  el('circle', { cx: x, cy: y, r: 18, fill: 'transparent' }, g);
  return g;
}
const route = (svg, e, d, G) => el('path', { class: 'e', d, fill: 'none', stroke: edgeCol(e.k, G), 'stroke-width': e.k === 'r' ? 1.2 : e.rate ? 1.6 : 1, 'stroke-dasharray': e.k === 'r' ? '3 4' : '', opacity: e.rate || e.k === 'r' ? .55 : .25, 'data-a': e.a, 'data-b': e.b }, svg);
/** HTML labels over the map, measured and pushed apart until none overlap. side: l (text ends at x), r (starts at x), c (centred). */
function labelLayer(host, G) {
  const L = [];
  return {
    add(n, x, y, side, cls = '', html) {
      const pc = n.kind === 'account' && G.peers ? `<span class="pc">${n.on.map((id) => `<i style="background:${(G.by[id] || {}).col}"></i>`).join('')}</span>` : '';
      const d = h(`<div class="lb ${cls}" data-for="${esc(n.id)}">${html || `<b>${esc(n.label)}${pc}</b><span class="${n.tone || ''}">${esc(n.sub)}</span>`}</div>`);
      host.append(d); L.push({ d, x, y, side, n }); return d;
    },
    place(gap = 3) {
      for (const l of L) { l.w = l.d.offsetWidth; l.h = l.d.offsetHeight; l.left = l.side === 'r' ? l.x : l.side === 'l' ? l.x - l.w : l.x - l.w / 2; l.top = l.y - l.h / 2; l.top0 = l.top; }
      const s = L.slice().sort((a, b) => a.top - b.top);
      for (let pass = 0; pass < 3; pass++) for (let i = 0; i < s.length; i++) for (let j = 0; j < i; j++) { const a = s[j], b = s[i]; if (b.left < a.left + a.w + 6 && a.left < b.left + b.w + 6 && b.top < a.top + a.h + gap) b.top = a.top + a.h + gap; }
      for (const l of L) { l.d.style.transform = `translate(${l.left.toFixed(1)}px, ${l.top.toFixed(1)}px)`; l.d.style.textAlign = l.side === 'l' ? 'right' : l.side === 'c' ? 'center' : 'left'; }
      return L;
    },
  };
}
/** Beads along routes, as many and as fast as the traffic on them; a still picture when motion is off. */
function beads(svg, items, motion, G) {
  const g = el('g', { class: 'beads' }, svg), B = [];
  for (const it of items) {
    if (!it.e.rate) continue;
    const len = it.path.getTotalLength(), n = Math.max(1, Math.round(Math.log10(it.e.rate + 1) * 1.3)), v = 26 + Math.log10(it.e.rate + 1) * 18;
    for (let i = 0; i < n; i++) B.push({ c: el('circle', { r: 2.6, fill: edgeCol(it.e.k, G) }, g), path: it.path, len, o: i / n, v, e: it.e });
  }
  const place = (t) => { for (const b of B) { const u = ((t * b.v) / b.len + b.o) % 1, p = b.path.getPointAtLength(u * b.len); b.c.setAttribute('cx', p.x.toFixed(1)); b.c.setAttribute('cy', p.y.toFixed(1)); } };
  let raf = 0;
  if (motion && B.length) { const t0 = performance.now(); const loop = (n) => { if (!svg.isConnected) return; if (!document.hidden) place((n - t0) / 1000); raf = requestAnimationFrame(loop); }; raf = requestAnimationFrame(loop); } else place(3.1);
  return { stop: () => cancelAnimationFrame(raf), filter(f) { g.style.opacity = f ? 1 : .55; for (const b of B) b.c.style.display = !f || f(b.e) ? '' : 'none'; } };
}
/** The circular mean; when the angles cancel the first one stands in. */
const meanAng = (as) => { const y = as.reduce((s, a) => s + Math.sin(a), 0), x = as.reduce((s, a) => s + Math.cos(a), 0); return !as.length ? deg(-90) : Math.hypot(x, y) < 1e-6 ? as[0] : Math.atan2(y, x); };
function spread(items, min) {
  for (let pass = 0; pass < 12; pass++) {
    items.sort((a, b) => a.ang - b.ang);
    for (let i = 1; i < items.length; i++) { const d = items[i].ang - items[i - 1].ang; if (d < min) { items[i].ang += (min - d) / 2; items[i - 1].ang -= (min - d) / 2; } }
  }
}
/** A corridor between two rings: radial to a corridor radius, an arc along it, radial in. */
function corridor(cx, cy, rA, tA, rB, tB, rc) {
  const p = (r, t) => `${(cx + r * Math.cos(t)).toFixed(1)} ${(cy + r * Math.sin(t)).toFixed(1)}`;
  let d = tB - tA; while (d > PI) d -= 2 * PI; while (d < -PI) d += 2 * PI;
  if (Math.abs(d) < 0.02) return `M${p(rA, tA)} L${p(rB, tB)}`;
  return `M${p(rA, tA)} L${p(rc, tA)} A${rc} ${rc} 0 0 ${d > 0 ? 1 : 0} ${p(rc, tB)} L${p(rB, tB)}`;
}

// ---------- the Horizon Dial ----------
const RING = { account: 0.42, tool: 0.64, router: 0.82, provider: 1 };
let dialStop = null;
/** The half circle from the docks: accounts, tools, the router gate and providers as hops; the finding's route selected. */
function drawDial(host, G, motion, selected) {
  if (dialStop) { dialStop(); dialStop = null; }
  host.innerHTML = '';
  const scene = host.parentElement, W = host.clientWidth, Hh = host.clientHeight;
  if (!W || !Hh) return;
  const cx = W / 2, cy = Hh - 64;
  // the dial takes the whole width, but shrinks until its label ring passes under the readouts
  const ovl = scene.querySelector('.ov.l'), box = { x0: 0, x1: ovl.offsetWidth + 110, y1: ovl.offsetHeight + 14 };
  const clear = (R) => { const r = R + 22, dx = cx >= box.x0 && cx <= box.x1 ? 0 : Math.min(Math.abs(box.x0 - cx), Math.abs(box.x1 - cx)); return dx >= r || cy - Math.sqrt(r * r - dx * dx) >= box.y1; };
  let R = Math.min(W / 2 - 130, Hh - 96);
  while (R > 140 && !clear(R)) R -= 4;
  const svg = el('svg', { viewBox: `0 0 ${W} ${Hh}` }); host.append(svg);
  const { N, E, by } = G;
  const acc = N.filter((n) => n.kind === 'account');
  acc.forEach((n, i) => { n.ang = deg(-180 + ((i + 1) * 180) / (acc.length + 1)); });
  const of = (pred) => N.filter(pred);
  for (const t of of((n) => n.kind === 'tool')) t.ang = meanAng(acc.filter((a) => a.tool === t.harness).map((a) => a.ang));
  spread(of((n) => n.kind === 'tool'), deg(14));
  if (by.router) by.router.ang = meanAng(of((n) => n.kind === 'tool' && E.some((e) => e.a === n.id && e.b === 'router')).map((t) => t.ang));
  const provs = of((n) => n.kind === 'provider');
  for (const p of provs) { const via = E.filter((e) => e.b === p.id && e.a !== 'router').map((e) => by[e.a].ang); p.ang = via.length ? meanAng(via) : by.router ? by.router.ang : deg(-90); }
  const routedP = provs.filter((p) => p.via === 'router');
  routedP.forEach((p, i) => { p.ang = (by.router ? by.router.ang : deg(-90)) + deg((i - (routedP.length - 1) / 2) * 16); });
  spread(provs, deg(12));
  provs.forEach((p) => { p.ang = Math.max(deg(-168), Math.min(deg(-12), p.ang)); });
  // the dial: baseline and the hop arcs, named at their left ends
  el('line', { x1: cx - R - 20, x2: cx + R + 20, y1: cy, y2: cy, stroke: 'var(--line)' }, svg);
  for (const [k, r] of Object.entries(RING)) {
    if (k === 'router') continue;
    el('path', { d: `M${cx - r * R} ${cy} A${r * R} ${r * R} 0 0 1 ${cx + r * R} ${cy}`, fill: 'none', stroke: 'var(--line)', 'stroke-dasharray': k === 'tool' ? '2 5' : '' }, svg);
    el('text', { x: cx - r * R, y: cy + 13, 'text-anchor': 'middle', fill: 'var(--m)', 'font-size': 10.5, 'font-family': 'var(--mono)' }, svg).textContent = { account: 'accounts', tool: 'tools', provider: 'providers' }[k];
  }
  // the computers' docks along the baseline, this one in the middle
  const pcs = of((n) => n.kind === 'pc'), dockX = {};
  pcs.forEach((c, i) => { dockX[c.id] = cx + (pcs.length === 1 ? 0 : (i - (pcs.length - 1) / 2) * (0.84 * R / Math.max(1, pcs.length - 1))); });
  const P = (n) => (n.kind === 'pc' ? [dockX[n.id], cy] : [cx + RING[n.kind] * R * Math.cos(n.ang), cy + RING[n.kind] * R * Math.sin(n.ang)]);
  const paths = E.map((e, i) => {
    const A = by[e.a], B = by[e.b];
    if (A.kind === 'pc') { const [x1, y1] = P(A), [x2, y2] = P(B); return { e, path: route(svg, e, `M${x1} ${y1} C${x1} ${y1 - 0.2 * R} ${x2} ${y2 + 0.16 * R} ${x2} ${y2}`, G) }; }
    const rA = RING[A.kind] * R, rB = RING[B.kind] * R, rc = (rA + rB) / 2 + ((i % 3) - 1) * 5;
    return { e, path: route(svg, e, corridor(cx, cy, rA, A.ang, rB, B.ang, rc), G) };
  });
  const bd = beads(svg, paths, motion, G);
  const lab = labelLayer(host, G);
  for (const n of N) {
    const [x, y] = P(n); body(svg, n, x, y, G);
    const right = n.kind === 'pc' ? true : Math.cos(n.ang) >= 0;
    if (n.kind === 'pc') lab.add(n, x, cy + 16, 'c', 'sm', `<b>${esc(n.label)}</b>`);
    else if (n.kind === 'account') lab.add(n, x + (right ? 22 : -22), y, right ? 'r' : 'l');
    else if (n.kind === 'tool') lab.add(n, x, y - 14, 'c', 'sm', `<b>${toolIcon(n.harness, 12)}${esc(n.label)}</b><span class="${n.tone || ''}">${esc(n.sub)}</span>`);
    else if (n.kind === 'router') lab.add(n, x + (right ? 14 : -14), y, right ? 'r' : 'l', 'sm');
    else { const r = R + 18; lab.add(n, cx + r * Math.cos(n.ang), cy + r * Math.sin(n.ang), right ? 'r' : 'l'); }
  }
  host.classList.add('measuring'); const placed = lab.place(); host.classList.remove('measuring');
  for (const l of placed) if (l.n.kind === 'provider' && Math.abs(l.top - l.top0) > 4) el('path', { d: `M${l.n.x} ${l.n.y} L${l.side === 'r' ? l.left - 4 : l.left + l.w + 4} ${l.top + l.h / 2}`, stroke: 'var(--m)', 'stroke-width': .8, fill: 'none', opacity: .7 }, svg);
  // the finding's route is selected on open; hover, focus or click another, Escape returns
  const apply = (lit) => {
    host.querySelectorAll('[data-node]').forEach((g) => g.classList.toggle('dim', !!lit && !lit.has(g.dataset.node)));
    host.querySelectorAll('.lb[data-for]').forEach((d) => { const on = !!lit && lit.has(d.dataset.for); d.classList.toggle('dim', !!lit && !on); d.classList.toggle('lit', on); });
    host.querySelectorAll('path.e').forEach((p) => p.classList.toggle('dim', !!lit && !(lit.has(p.dataset.a) && lit.has(p.dataset.b))));
    bd.filter(lit ? (e) => lit.has(e.a) && lit.has(e.b) : null);
  };
  let sticky = by[selected] ? selected : null, hover = null;
  const upd = () => apply(hover ? chain(E, hover) : sticky ? chain(E, sticky) : null);
  host.querySelectorAll('[data-node]').forEach((g) => {
    const id = g.dataset.node;
    g.addEventListener('mouseenter', () => { hover = id; upd(); }); g.addEventListener('mouseleave', () => { hover = null; upd(); });
    g.addEventListener('focus', () => { hover = id; upd(); }); g.addEventListener('blur', () => { hover = null; upd(); });
    g.addEventListener('click', (ev) => { ev.stopPropagation(); if (sticky === id && g.dataset.href) { location.hash = g.dataset.href; return; } sticky = id; upd(); });
    g.addEventListener('keydown', (ev) => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); if (sticky === id && g.dataset.href) location.hash = g.dataset.href; else { sticky = id; upd(); } } });
  });
  host.addEventListener('click', () => { sticky = null; upd(); });
  host.addEventListener('keydown', (ev) => { if (ev.key === 'Escape') { sticky = null; hover = null; upd(); } });
  upd();
  dialStop = bd.stop;
}

// ---------- limits ----------
/** Every limit with a window, tightest first, with its forecast at the reset. */
function burnRows(ps, d) {
  const out = [];
  for (const p of ps) for (const t of (p.quota && p.quota.tiers) || []) {
    if (!t.resets_at || t.resets_at <= d.now) continue;
    const f = d.forecast[p.name + '|' + t.id];
    const left = Math.max(0, Math.round(100 - t.used));
    const o = f && f.runs_out && f.runs_out < t.resets_at ? f.runs_out : null;
    out.push({ name: p.label, acct: p.name, win: t.label, used: Math.min(100, t.used), left, at: f ? Math.min(99.5, f.at_reset) : null, out: o, reset: t.resets_at, read: p.quota.fetched_at });
  }
  const midnight = new Date(d.now); midnight.setHours(24, 0, 0, 0);
  for (const p of ps) {
    const c = d.caps[p.name];
    if (!c) continue;
    const used = Math.min(100, (c.spent / c.cap) * 100);
    out.push({ name: p.label, acct: p.name, win: `$${c.cap.toFixed(2)} daily cap`, used, left: Math.max(0, Math.round(100 - used)), at: null, out: null, reset: midnight.getTime(), cap: c });
  }
  return out.sort((a, b) => a.left - b.left);
}
/** Every account's limits as rings: inner 5-hour, outer weekly; dashed is the estimate at the reset. */
function ringsPanel(ps, G, now) {
  const rings = ps.map((p) => {
    const n = G.by['a:' + p.name], a = n ? n.acct : null, S = 84, c = 42;
    let g = '', mid = '', note = '', cls = '';
    if (!a || a.needsLogin) { g = `<circle cx="${c}" cy="${c}" r="34" fill="none" stroke="var(--neg)" stroke-dasharray="3 5" opacity=".7"/>`; mid = '—'; note = 'sign in to read'; cls = 'neg'; }
    else if (a.cap) { const u = Math.min(1, a.cap.spent / a.cap.cap); g = `<circle cx="${c}" cy="${c}" r="34" fill="none" stroke="var(--line)" stroke-width="6"/>${u > 0 ? `<path d="${arcPath(c, c, 34, 0, u)}" fill="none" stroke="var(--r2)" stroke-width="6" stroke-linecap="round"/>` : ''}`; mid = Math.round((1 - u) * 100) + '%'; note = `$${a.cap.cap.toFixed(2)} daily cap · ${money(a.cap.spent)} today`; }
    else if (!a.windows) { g = `<circle cx="${c}" cy="${c}" r="34" fill="none" stroke="var(--line)" stroke-dasharray="2 4"/>`; mid = p.kind === 'api_key' ? '∞' : '—'; note = p.kind === 'api_key' ? 'API key, no limit' : 'limits not read yet'; }
    else {
      a.windows.forEach((w, i) => { const r = i ? 34 : 25, u = w.used / 100, f = Math.min(1, (w.at != null ? w.at : w.used) / 100);
        g += `<circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="var(--line)" stroke-width="6"/>${f > u ? `<path d="${arcPath(c, c, r, u, f)}" fill="none" stroke="${tone(w.at)}" stroke-width="6" stroke-dasharray="2 3" opacity=".55"/>` : ''}${u > 0 ? `<path d="${arcPath(c, c, r, 0, u)}" fill="none" stroke="${tone(w.used)}" stroke-width="6" stroke-linecap="round"/>` : ''}`; });
      const wk = a.windows[1]; mid = Math.round(100 - wk.used) + '%';
      note = wk.out ? `≈ out ${when(wk.out)}` : wk.reset ? `${wk.label.toLowerCase()} resets ${when(wk.reset)}` : ''; cls = wk.out ? 'warn' : '';
    }
    return `<a class="ring" href="#account?name=${encodeURIComponent(p.name)}" style="text-decoration:none;color:inherit" data-testid="ring-${esc(p.name)}"><svg width="${S}" height="${S}" viewBox="0 0 ${S} ${S}" role="img" aria-label="${esc(p.label)}: ${esc(note)}">${g}<text x="${c}" y="${c + 5}" text-anchor="middle" font-size="14" font-weight="600" fill="var(--ink)" font-family="var(--sans)">${mid}</text></svg><b>${toolIcon(p.harness, 12)}${esc(p.label)}</b><small class="${cls}">${esc(note)}</small></a>`;
  }).join('');
  const read = ps.map((p) => p.quota && p.quota.fetched_at).filter(Boolean);
  return `<section class="panel" data-testid="limits"><div class="ph"><h2>Limits</h2><span class="muted sm">inner 5-hour, outer weekly · dashed is the estimate at the reset${read.length ? ' · read ' + ago(now - Math.max(...read)) : ''}</span></div>
    ${rings ? `<div class="rings">${rings}</div>` : '<p class="empty">Limits show once an account with a plan has run on this computer.</p>'}</section>`;
}

// ---------- the page ----------
/** The Running now panel: every session open on any computer, with the computer's colour and the tool's icon. */
const runningNow = (running, pcs, now) => `<section class="panel" data-testid="running-now"><div class="ph"><h2>Running now</h2><span class="muted mono sm">${running.length} ${running.length === 1 ? 'session' : 'sessions'}</span></div>
      ${running.length ? running.map((r) => `<div class="runrow">${cdot(pcs, r.device)}<span><b>${esc(r.project_name || r.project || (r.cwd || '').split(/[\\/]/).filter(Boolean).pop() || 'no folder')}</b> <span class="muted">${toolIcon(r.harness, 12)}${esc(acctName(r.profile))}, ${esc(nameOf(pcs, r.device))}</span></span><span class="mono">${r.started_at ? dur(now - r.started_at) : ''}</span></div>`).join('') : '<p class="empty">Nothing is running. Start an account from Accounts, or resume a session.</p>'}</section>`;
// presence from a connected computer arrives every 10 s: only the Running now panel is drawn again, in place
document.addEventListener('presence', async (e) => {
  const sec = document.querySelector('[data-testid=running-now]');
  if (S.route.view !== 'home' || !sec || document.getElementById('dialog-root').innerHTML) return;
  const d = await call('live').catch(() => null);
  if (d && S.route.view === 'home' && sec.isConnected) sec.outerHTML = runningNow(d.live.running || [], e.detail, d.now);
});

// from the times the parts of an answer reached the log: close, not exact
const speed = (r) => (r.gen_ms >= 300 && r.output > 0 ? `≈${Math.round(r.output / (r.gen_ms / 1000)).toLocaleString()} <small>tok/s</small>` : '<span class="dash">—</span>');
const firstToken = (r) => (r.ttft_ms != null ? `≈${(r.ttft_ms / 1000).toFixed(1)} s` : '<span class="dash">—</span>');
const TIMING = 'Approximate: worked out from the times each tool wrote the parts of the answer to its log';
function requestTable(rows) {
  return `<table class="grid reqlog" data-testid="request-log"><thead><tr><th>Time</th><th>Tool</th><th>Account</th><th>Model</th><th class="num">Cost</th><th class="num">Fresh input</th><th class="num">Output</th><th class="num">Cache read</th><th class="num">Cache write</th><th class="num" title="${TIMING}">Speed</th><th class="num" title="${TIMING}">First reply</th></tr></thead><tbody>
    ${rows.map((r) => `<tr><td class="mono">${new Date(r.ts).toDateString() === new Date().toDateString() ? hm(r.ts) + ':' + String(new Date(r.ts).getSeconds()).padStart(2, '0') : new Date(r.ts).toLocaleDateString([], { day: 'numeric', month: 'short' }) + ' ' + hm(r.ts)}</td>
      <td>${toolPill(r.harness, TOOL[r.harness])}</td><td>${esc(acctName(r.profile))}</td><td class="mono">${esc(r.model)}</td><td class="num"><b>${money(r.cost, 4)}</b></td>
      <td class="num">${fmt(r.input)}</td><td class="num">${fmt(r.output)}</td><td class="num">${fmt(r.cache_read)}</td><td class="num">${fmt(r.cache_write)}</td>
      <td class="num" data-testid="speed">${speed(r)}</td><td class="num" data-testid="ttft">${firstToken(r)}</td></tr>`).join('') || '<tr><td colspan="11" class="empty">No replies in this range.</td></tr>'}</tbody></table>`;
}
/** The short table: the latest replies with their computer, speed and time to first reply. */
function latestTable(rows, pcs) {
  const peers = syncOn(pcs) && pcs.peers.length > 0;
  return `<table class="grid" data-testid="latest-replies"><thead><tr><th>Time</th><th>Account</th><th>Model</th>${peers ? '<th>Computer</th>' : ''}<th class="num">Tokens</th><th class="num" title="${TIMING}">Speed</th><th class="num" title="${TIMING}">First reply</th></tr></thead><tbody>
    ${rows.slice(0, 8).map((r) => `<tr><td class="mono">${hm(r.ts)}:${String(new Date(r.ts).getSeconds()).padStart(2, '0')}</td><td>${toolIcon(r.harness, 13)}${esc(acctName(r.profile))}</td><td class="mono">${esc(r.model)}</td>${peers ? `<td>${r.device ? cdot(pcs, r.device) + ' ' + esc(nameOf(pcs, r.device)) : ''}</td>` : ''}
      <td class="num">${fmt(r.input + r.output + r.cache_read + r.cache_write)}</td><td class="num">${speed(r)}</td><td class="num ${r.ttft_ms > 4000 ? 'warnT' : ''}">${firstToken(r)}</td></tr>`).join('') || `<tr><td colspan="${peers ? 7 : 6}" class="empty">No replies in this range.</td></tr>`}</tbody></table>`;
}
export function setTodayBadge(cost) {
  const el = document.getElementById('today-cost');
  if (el) el.textContent = cost == null ? '' : 'Today ' + money(cost);
}

/** What needs attention: a limit running out before its reset, a login to renew, a paused provider, router errors. */
function attention(risks, ps, d) {
  const items = [];
  for (const r of risks) items.push({ k: 'warn', t: `${esc(r.name)} ${esc(r.win.toLowerCase())}`, em: `≈ ${when(r.out)}`, p: `Runs out ${dur(r.reset - r.out)} before its reset at today's pace.`, href: `#account?name=${encodeURIComponent(r.acct)}` });
  for (const p of ps) if (p.kind === 'subscription' && (p.identity.needs_login || !p.identity.logged_in)) items.push({ k: 'neg', t: esc(p.label), em: 'login', p: p.identity.needs_login ? 'The login expired, so its limits cannot be read.' : 'Not logged in yet.', btn: `<button class="btn sm" data-act="login" data-name="${esc(p.name)}">Sign in</button>` });
  for (const [id, b] of Object.entries(d.live.breakers || {})) if (b.state === 'open' && (!b.retry_at || b.retry_at > d.now)) items.push({ k: 'neg', t: esc(id), em: b.retry_at ? 'retry ' + dur(b.retry_at - d.now) : 'paused', p: `Paused by the router after ${b.failures || 'repeated'} ${b.failures === 1 ? 'error' : 'errors'} in a row.`, href: '#routing' });
  return `<section class="attn" aria-label="Needs attention" data-testid="attention"><div class="inc hd">Needs attention<span>${items.length ? items.length + ' open' : 'nothing'}</span></div>
    ${items.map((i) => `<div class="inc ${i.k}" ${i.href ? `data-href="${i.href}" style="cursor:pointer"` : ''}><div class="t"><i></i>${i.t}<em>${i.em}</em></div><p>${i.p}</p>${i.btn || ''}</div>`).join('') || '<div class="inc"><p>Every limit lasts until its reset at this pace, every login is valid, no provider is paused.</p></div>'}</section>`;
}

view('home', async (sheet) => {
  sheet.className = 'sheet';
  const wk = 'home:' + JSON.stringify([st.range, st.harness, st.device, st.log, scope()]);
  warmPage(sheet, wk);
  const hours = RANGES.find((r) => r[0] === st.range)[1], days = Math.round(hours / 24);
  // scope: every computer added up, or this one only; the sync core answers the same shape either way
  const [state, d, pcs] = await Promise.all([loadState(), call('home', { days, hours, harness: st.harness, limit: 50, scope: scope() }), computersStatus().catch(() => null)]);
  st.rows = d.requests.rows; st.total = d.requests.total;
  const o = d.overview, k = d.dash, s = state.settings;
  if (!st.harness) setTodayBadge(o.today_cost);
  const shown = shownTools(state), ps = state.profiles.filter((p) => shown.some((h) => h.id === p.harness)), now = k.now;
  const { best, text } = findings(ps, now);
  const burn = burnRows(ps, k), risks = burn.filter((b) => b.out).sort((a, b) => a.out - b.out), risk = risks[0];
  const tools = state.harnesses.filter((h) => shown.some((x) => x.id === h.id));
  const logged = ps.filter((p) => p.identity.logged_in).length, open = ps.reduce((a, p) => a + p.running.length, 0);
  if (!S.appVersions) {
    S.appVersions = {};
    call('apps').then((a) => { S.appVersions = Object.fromEntries(a.apps.filter((x) => x.version).map((x) => [x.id, x.version])); if (S.route.view === 'home') render(); }).catch(() => {});
  }
  const G = graph(state, k, pcs);
  const byDay = Object.fromEntries((((d.activity || {}).days) || []).filter((x) => x.by_device).map((x) => [x.day, x.by_device]));
  const pc = G.peers ? { c: pcs, by: byDay, dev: computers(pcs).some((x) => x.id === st.device) ? st.device : '' } : null;
  const running = G.peers ? (k.live.running || []) : ps.flatMap((p) => p.running.map((r) => ({ ...r, profile: p.name, harness: p.harness, device: pcs ? pcs.this.id : '' })));
  const read = ps.map((p) => p.quota && p.quota.fetched_at).filter(Boolean);
  const basis = risk ? `Measured <b>${Math.round(risk.used)}% used</b>, resets <b>${when(risk.reset)}</b>${risk.read ? ', read ' + ago(now - risk.read) : ''}. The run-out is an estimate from today's pace.`
    : `${ps.length} ${ps.length === 1 ? 'account' : 'accounts'} · ${logged} logged in · ${open ? `<a href="#sessions?live=1" class="acT" data-testid="home-live">${open} ${open === 1 ? 'console' : 'consoles'} open</a>` : 'no console open'} · router ${state.router.running ? ':' + state.router.port + ' running' : 'stopped'}${read.length ? ' · limits read ' + ago(now - Math.max(...read)) : ''} · ${k.cur.requests.toLocaleString()} ${k.cur.requests === 1 ? 'request' : 'requests'} ${RANGE_LABEL[st.range].toLowerCase()}`;
  sheet.innerHTML = `
    <div class="hero">
      <section class="scene dial" aria-label="Live routes"><div class="map" data-testid="routes"></div>
        <div class="ov l"><div>
          <div class="crumb"><i class="dot" style="width:6px;height:6px;border-radius:50%;background:var(--ac)"></i>Overview · ${esc(state.computer || 'this computer')}${G.peers ? ' · ' + G.pcList.length + ' computers' : ''} · ${open} ${open === 1 ? 'session' : 'sessions'} open</div>
          <h1 data-testid="finding">${risk ? `<span class="w">${esc(risk.name)}</span> will likely run out of its ${esc(risk.win.toLowerCase())} ${when(risk.out).replace(/ (\d)/, ' at about $1')}, ${dur(risk.reset - risk.out)} before it resets.` : text}</h1>
          <div class="basis" data-testid="basis">${basis}</div>
          </div>${readouts(k)}</div>
        <div class="ov b"><span><i style="background:var(--ac)"></i>signed-in plan</span><span><i style="background:var(--r2)"></i>through the router</span><span><i style="background:var(--neg)"></i>paused or logged out</span>${G.peers ? G.pcList.map((c) => `<span><i class="sq" style="background:${pcCol(pcs, c.id)}"></i>${esc(c.name)}</span>`).join('') : ''}</div>
        <div class="ov h">${G.empty ? 'add an account to see its route' : 'hover, focus or click a body to trace it · click it again to open it · Esc returns'}</div></section>
      <div class="aside">
        <section class="panel" data-testid="activity"><div class="ph"><h2>Activity, 53 weeks</h2>${pc ? `<span class="seg3" role="group" aria-label="Computer" data-testid="activity-pc">${[['', 'All'], ...computers(pcs).map((x) => [x.id, x.name])].map(([v, l]) => `<a class="${pc.dev === v ? 'on' : ''}" data-act="home-device" data-v="${esc(v)}" data-testid="activity-pc-${esc(v || 'all')}">${esc(l)}</a>`).join('')}</span>` : ''}<span class="muted sm r">Mon inside · daily ${pc && pc.dev ? 'tokens on ' + esc(nameOf(pcs, pc.dev)) : st.metric === 'hit' ? 'tokens' : st.metric}</span></div><div class="hmwrap"></div></section>
        ${attention(risks, ps, k)}
      </div>
    </div>
    <div class="acts" data-testid="home-filters"><div class="toolfilter" role="group" aria-label="Tool"><a class="${!st.harness ? 'on' : ''}" data-act="home-tool" data-v="" data-testid="tool-all">All</a>
        ${tools.map((h) => `<a class="${st.harness === h.id ? 'on' : ''}" data-act="home-tool" data-v="${h.id}" data-testid="tool-${h.id}">${toolIcon(h.id, 12)}${esc(h.label)}</a>`).join('')}</div>
      ${syncOn(pcs) ? scopeSeg() : ''}
      ${best ? `<button class="btn primary" data-act="start" data-name="${esc(best.name)}" data-testid="start-best" style="margin-left:auto">▶ Start ${esc(best.label)}</button>` : ''}</div>
    <div class="row2">${ringsPanel(ps, G, now)}${runningNow(running, pcs, now)}</div>
    <section class="panel"><div class="ph"><h2>${st.log ? 'Request log' : 'Latest replies'}</h2><span class="muted mono sm">${d.requests.total.toLocaleString()} ${RANGE_LABEL[st.range].toLowerCase()} · speed and first reply are approximate</span>
        <a class="linkb r" data-act="home-log" data-testid="request-log-open">${st.log ? 'Latest replies' : 'Request log'}</a></div>
      ${st.log ? `<p class="hint">Costs are what these tokens would cost at API list prices (models.dev). Subscriptions are not billed per token.</p>
      <div class="tscroll" id="home-table">${requestTable(st.rows)}</div>
      ${st.rows.length < st.total ? '<div style="padding:10px 0"><button class="btn outline sm" data-act="home-more" data-testid="home-more">Show 50 more</button></div>' : ''}` : latestTable(st.rows, pcs)}</section>`;
  fit(sheet.querySelector('.hmwrap'), () => yearClock(o.heatmap, st.metric === 'hit' ? 'tokens' : st.metric, pc));
  // the dial is drawn for the width the scene has, and again when the window changes size
  const map = sheet.querySelector('.map');
  const draw = () => drawDial(map, G, !!s.moving_routes && !window.matchMedia('(prefers-reduced-motion: reduce)').matches, risk ? 'a:' + risk.acct : null);
  draw();
  let t; const onResize = () => { if (!map.isConnected) { window.removeEventListener('resize', onResize); return; } clearTimeout(t); t = setTimeout(draw, 150); };
  window.addEventListener('resize', onResize);
  keepPage(sheet, wk);
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
  const days = Math.max(1, Math.round(RANGES.find((r) => r[0] === st.range)[1] / 24));
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
document.addEventListener('click', (e) => { const a = e.target.closest('[data-href]'); if (a && !a.closest('[data-node]')) location.hash = a.dataset.href; });
