// Overview, as the 1.10 drawing draws it (docs/project-review-2026-11/mockups/a-overview.html): the head with the range
// and three readouts; a deck of three cards (use next, per tool; needs attention, one entry per account; running now);
// the accounts runway, one 48 px row per account (its limits in one line, one lane from now to +7 d, one verdict, one
// action); then the selected route, the 53-week year clock and the latest replies with the request log behind a button.
// Measured numbers are plain; estimates carry ≈; — means not read. No tokens-per-second on this page.

import { S, call, view, render, attempt, loadState, action, acctName, shownTools, warmPage, keepPage, patch, openDialog, dlgHead, dialogOpen, mutate, openMenu, needsMark } from './core.js';
import { esc, fmt, hm, dur, toolIcon } from './lib.js';
import { accountViews, metricsOf, fmtDuration, fmtAbsoluteReset, money, TOOL, w5of, wkof, showUsed } from './accounts.js';
import { creditText, freshCredits, freshReset, resetHint, limitSlots, roomCls, roomText } from './limits.js';
import { computersStatus, syncOn, computers, nameOf, scope, setScope } from './computers.js';
import { pageHead, plural, seg } from './parts.js';

const RANGES = [['today', 0], ['7d', 168], ['30d', 720], ['all', 24 * 400]], RANGE_LABEL = { today: 'Today', '7d': '7 d', '30d': '30 d', all: 'All' };
const WINDOWS = [['both', '5h + Week'], ['5h', '5-hour only'], ['week', 'Weekly only']];
const USE_KEY = 'hm.usenext';
// the page's own choices: the table's tool, the tool Use next recommends for (remembered), which computer's sessions,
// the readout range, the year clock's computer, the window the lanes draw, the 5-hour preference, the selected row,
// and the loaded replies
const st = { harness: '', useTool: (() => { try { return localStorage.getItem(USE_KEY) || ''; } catch { return ''; } })(), livePc: 'all', range: 'today', device: '', windowMode: 'both', preferExpiring: false, selected: null, rows: [], total: 0 };
const H = 3600e3, DAY = 24 * H, HORIZON = 7 * DAY;
const ago = (ms) => (ms < 60000 ? 'just now' : dur(ms) + ' ago');
const TIMING = 'Approximate: worked out from the times each tool wrote the parts of the answer to its log';
const firstReply = (r) => (r.ttft_ms != null ? `≈ ${(r.ttft_ms / 1000).toFixed(1)} s` : '<span class="dash">—</span>');
const timeCell = (ts) => (new Date(ts).toDateString() === new Date().toDateString() ? hm(ts) : new Date(ts).toLocaleDateString([], { day: 'numeric', month: 'short' }) + ' ' + hm(ts));
/** A segmented choice whose labels are markup (an icon, a count); parts.js seg() takes plain words.
 * `opts` is [[value, label markup, the name said on hover and to a screen reader, testid]…]. */
const segOf = (label, opts, cur, act, cls = '') => `<div class="seg ${cls}" role="group" aria-label="${esc(label)}">${opts.map(([v, html, name, tid]) => `<button type="button" class="${v === cur ? 'on' : ''}" aria-pressed="${v === cur}" data-act="${act}" data-v="${esc(v)}" ${name ? `title="${esc(name)}" aria-label="${esc(name)}"` : ''} data-testid="${esc(tid)}">${html}</button>`).join('')}</div>`;
/** A button that opens a short menu of choices and names the one chosen. */
const pick = (act, label, text, testid) => `<button type="button" class="btn sm" data-act="${act}" aria-haspopup="menu" aria-label="${esc(label)}: ${esc(text)}" title="${esc(label)}" data-testid="${testid}">${esc(text)}<span aria-hidden="true">▾</span></button>`;

// ---------- the computers: this one first, each with the colour its dot has everywhere ----------
function pcList(state, pcs) {
  const list = syncOn(pcs) ? computers(pcs) : [{ id: pcs && pcs.this ? pcs.this.id : 'this', name: (pcs && pcs.this && pcs.this.name) || state.computer || 'this computer' }];
  return list.map((c, i) => ({ id: c.id, name: c.name, ci: i % 4 }));
}
const pcDot = (pcs, id, title = '') => { const c = pcs.find((x) => x.id === id); return c ? `<i class="pc-dot c${c.ci}" title="${esc(title || c.name)}"></i>` : ''; };

/** The sessions running now: this computer's consoles and, with sync on, every other computer's; `titles` names them. */
function runningSessions(state, d, pcs, me, titles) {
  const by = Object.fromEntries(state.profiles.map((p) => [p.name, p]));
  const folder = (r) => (r.cwd || '').split(/[\\/]/).filter(Boolean).pop() || 'folder unknown';
  const named = (s) => { const t = titles[s.accountId + '/' + s.id] || {}; return { ...s, title: t.title || (s.id ? 'Untitled session' : 'New console'), project: t.project_name || s.project }; };
  const here = state.profiles.flatMap((p) => p.running.map((r) => ({ id: r.session_id || '', pid: r.pid, project: folder(r), computer: me, accountId: p.name, tool: p.harness, since: r.started_at, streaming: !!((d.dash.live.accounts || {})[p.name] || {}).tokens })));
  const far = syncOn(pcs) ? (pcs.peers || []).flatMap((pc) => (pc.running || []).map((r) => ({ id: r.session_id || '', project: folder(r), computer: pc.id, accountId: r.profile, tool: (by[r.profile] || {}).harness || '', since: r.started_at, streaming: false }))) : [];
  return [...here, ...far].map(named);
}

// ---------- the readouts: the number for the range; its change against the period before is said on hover ----------
function change(cur, prev, kind) {
  if (cur == null || prev == null) return '—';
  if (kind === 'pct') { if (!prev) return cur ? 'new' : '—'; const d = Math.round(((cur - prev) / prev) * 100); return `≈ ${d > 0 ? '+' : d < 0 ? '−' : '±'}${Math.abs(d)}%`; }
  if (kind === 'money') { const d = cur - prev; return `≈ ${d >= 0 ? '+' : '−'}${money(Math.abs(d), 0)}`; }
  const d = Math.round((cur - prev) * 100); return `≈ ${d >= 0 ? '+' : '−'}${Math.abs(d)} pt`;
}
function readouts(k, nPcs) {
  const c = k.cur, p = k.prev, fresh = !p.requests || st.range === 'all';
  const vs = (...a) => (fresh ? '' : ` · ${change(...a)} against the period before`);
  const rd = (key, value, label, title) => `<div title="${esc(title)}" data-testid="kpi-${key}"><b>${value}</b><span>${label}</span></div>`;
  return `<div class="ov-readouts">${rd('tokens', fmt(c.tokens), 'tokens', `${plural(c.requests, 'request')} ${RANGE_LABEL[st.range].toLowerCase()}${nPcs > 1 ? `, ${nPcs} computers combined` : ''}${vs(c.tokens, p.tokens, 'pct')}`)}${rd('cost', c.cost == null ? '—' : '≈ ' + money(c.cost, c.cost >= 100 ? 0 : 2), 'at list prices', `What these tokens would cost at API list prices; subscriptions are not billed per token${vs(c.cost, p.cost, 'money')}`)}${rd('hit', `${Math.round(c.hit * 100)}%`, 'cached', `The share of input read from the cache${vs(c.hit, p.hit, 'pt')}`)}</div>`;
}

// ---------- the year clock: 53 weeks around, Monday inside to Sunday outside; crisp wedge cells ----------
function yearClock(days, pcs, dev) {
  const val = (d) => (dev ? ((d.by_device || {})[dev] || 0) : d.tokens);
  const max = Math.max(1e-9, ...days.map(val)), best = days.reduce((a, d) => (val(d) > val(a) ? d : a), days[0]);
  const cx = 78, cy = 78, rIn = 30, rOut = 67, stepA = (2 * Math.PI) / 54, stepR = (rOut - rIn) / 7;
  const first = new Date(days[0].day + 'T12:00'), off = (first.getDay() + 6) % 7;
  const parts = [];
  let lastM = -1;
  days.forEach((d, i) => {
    const date = new Date(d.day + 'T12:00'), week = Math.floor((i + off) / 7), day = (i + off) % 7, v = val(d) / max;
    const lv = v >= 0.85 ? 4 : v >= 0.55 ? 3 : v >= 0.3 ? 2 : v >= 0.08 ? 1 : v > 0 ? 1 : 0;
    const a0 = week * stepA - Math.PI / 2 + 0.03, a1 = a0 + stepA * 0.78, r0 = rIn + day * stepR + 0.4, r1 = r0 + stepR - 0.9;
    const P = (r, a) => `${(cx + Math.cos(a) * r).toFixed(1)},${(cy + Math.sin(a) * r).toFixed(1)}`;
    const label = `${date.toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' })}: ${dev ? `${fmt(val(d))} tokens on ${esc(nameOf(pcs, dev))}` : `${fmt(d.tokens)} tokens · ${d.requests} requests · ${money(d.cost)}`}`;
    parts.push(`<path class="yc-cell lv-${lv} ${d === best && val(best) ? 'best' : ''}" d="M${P(r0, a0)} L${P(r0, a1)} L${P(r1, a1)} L${P(r1, a0)} Z"><title>${esc(label)}</title></path>`);
    if (day === 0 && date.getMonth() !== lastM) { lastM = date.getMonth(); const am = week * stepA - Math.PI / 2 + stepA * 0.4; parts.push(`<text x="${(cx + Math.cos(am) * 73).toFixed(1)}" y="${(cy + Math.sin(am) * 73).toFixed(1)}" text-anchor="middle" dominant-baseline="central">${date.toLocaleDateString([], { month: 'short' })[0]}</text>`); }
  });
  const bv = val(best), total = days.reduce((s, d) => s + val(d), 0);
  const centre = dev ? `<small>${esc(nameOf(pcs, dev))} (53w)</small><b>53-wk total</b><span>${fmt(total)}</span>` : `<small>busiest day</small><b>${bv ? new Date(best.day + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short' }) : '—'}</b><span>${bv ? fmt(bv) : 'no usage yet'}</span>`;
  return { svg: `<div class="year-clock-svg-box"><svg width="156" height="156" viewBox="0 0 156 156" role="img" aria-label="Daily usage over the last 53 weeks, as a year clock" data-testid="heatmap">${parts.join('')}</svg><div class="year-clock-center">${centre}</div></div>`, best, bv, total };
}

// ---------- what a window says, in the few words a row has room for ----------
/** A per-model limit's short name: "Fable" of "Week · Fable". */
const modelName = (w) => w.label.split(' · ').pop();
const shortName = (w) => (w.kind === '5h' ? '5h' : w.kind === 'week' ? 'Wk' : modelName(w));
const longName = (w) => (w.kind === '5h' ? '5-hour' : w.kind === 'week' ? 'Week' : modelName(w));
/** "15:07" today, "Fri 09:00" another day, "next Tue" on today's weekday a week away. */
const clock = (at, now) => (at - now > DAY && new Date(at).getDay() === new Date(now).getDay() ? 'next ' + new Date(at).toLocaleDateString('en-US', { weekday: 'short' }) : fmtAbsoluteReset(at, now).replace(/^Today /, ''));
const dayOf = (at, now) => fmtAbsoluteReset(at, now).replace(/^Today /, 'today ');
/** A stretch of time in one unit, for a line with little room: "40 m", "21 h", "3 d". */
const about = (ms) => (ms < H ? `${Math.max(1, Math.round(ms / 60000))} m` : ms < 2 * DAY ? `${Math.round(ms / H)} h` : `${Math.round(ms / DAY)} d`);
/** The shared windows (5-hour, week) that are used up: the account cannot start until the last of them is back. */
const lockOf = (a) => [w5of(a), wkof(a)].filter((w, i, l) => w && l.indexOf(w) === i && w.kind !== 'model' && w.open && w.left === 0).sort((x, y) => y.resetMs - x.resetMs)[0] || null;
/** The window a row's lane and verdict speak for: the 5-hour or the weekly one when that is the choice; else a window
 * that is used up, else the one on pace to run out first, else the week. */
function governing(a) {
  const w5 = w5of(a), wk = wkof(a);
  if (st.windowMode === '5h') return w5 || null;
  if (st.windowMode === 'week') return wk && wk !== w5 ? wk : null;
  return lockOf(a) || a.windows.find((w) => w.open && w.left === 0) || a.windows.filter((w) => w.outAt).sort((x, y) => x.outAt - y.outAt)[0] || wk || w5 || null;
}
const toneOf = (w) => (w.left === 0 ? 'bad' : w.kind === '5h' ? (w.left <= 25 ? 'warn' : w.open && w.resetMs <= 2 * H && w.left >= 75 ? 'spare' : '') : (w.outMs != null || w.left <= 30 ? 'warn' : ''));
/** A limit bar: what is left (or used) filled, as an SVG whose width is an attribute. */
const bar = (pct, tone) => `<svg class="ov-bar" aria-hidden="true"><rect class="${tone}" width="${Math.max(0, Math.min(100, pct))}%" height="100%" rx="3"/></svg>`;

/** Column 2: the limits in one line, in three fixed places (week, a model's own, 5-hour), each coloured by what is
 * left of it; a second model's limit and further sit behind "+N". */
function limitsCell(a, used, now) {
  const lim = (name, title, pct, tone, num) => `<div class="ov-lim" title="${esc(title)}"><div class="t"><span>${esc(name)}</span><b class="${num}">${pct}%</b></div>${bar(pct, tone)}</div>`;
  if (a.cap) {
    const leftUsd = a.cap.limitUsd - a.cap.spentUsd, left = Math.max(0, Math.round((leftUsd / a.cap.limitUsd) * 100));
    return `<div class="ov-lims">${lim('Cap', `Daily router spend cap: ${money(leftUsd)} left of ${money(a.cap.limitUsd)}`, used ? 100 - left : left, 'r2', left === 0 ? 'negT' : '')}</div>`;
  }
  if (!a.measured) return `<span class="ov-unread" title="${a.needsLogin ? 'Not read: log in to read its limits' : a.noPlan ? 'No plan limits to read' : 'Not read yet: its limits show after it runs on this computer, or after a refresh'}">—</span>`;
  // three fixed places (week, a model's own limit, 5-hour), so each limit is in the same column on every row
  const { slots, rest: more } = limitSlots(a);
  const tip = (w) => `${w.label}: ${w.left}% left (${w.used}% used)${w.open ? ' · resets ' + fmtAbsoluteReset(w.resetsAt, now) : ''}${toneOf(w) === 'spare' ? ' · spare room before the reset' : ''}`;
  return `<div class="ov-lims ${more.length ? 'more' : ''}">${slots.map((w) => (w ? lim(shortName(w), tip(w), used ? w.used : w.left, roomCls(w.left), roomText(w.left)) : '<div class="ov-lim gap" aria-hidden="true"></div>')).join('')}${more.length ? `<span class="ov-more" title="${esc(more.map(tip).join('\n'))}" data-testid="more-${esc(a.id)}">+${more.length}</span>` : ''}</div>`;
}
/** Column 3: one lane from now to +7 d (+5 h for the 5-hour window alone): filled to the reset's tick; from the
 * estimated run-out to the reset it is striped. */
function laneCell(a, w, now) {
  const short = st.windowMode === '5h', scale = short ? 5 * H : HORIZON, parts = short ? 5 : 7;
  const cap = !short && a.cap ? { open: true, resetMs: a.cap.resetMs, left: 1, outMs: null } : null;
  w = w || cap;
  if (!w || !w.open) return '<svg class="ov-lane none" aria-hidden="true"><line x1="0" x2="100%" y1="4" y2="4"/></svg>';
  const at = (ms) => Math.min(100, (ms / scale) * 100), gate = at(w.resetMs), out = w.left === 0 ? 0 : w.outMs != null ? at(w.outMs) : null;
  // a stretch too short to see is drawn 2.5% wide, and the tick stands at its end
  const end = out === null ? gate : Math.max(gate, Math.min(100, out + 2.5));
  const grid = Array.from({ length: parts - 1 }, (_, i) => `<line class="g" x1="${(((i + 1) / parts) * 100).toFixed(2)}%" x2="${(((i + 1) / parts) * 100).toFixed(2)}%" y1="0" y2="8"/>`).join('');
  return `<svg class="ov-lane" aria-hidden="true">${grid}${out === null ? `<rect class="fill" width="${gate.toFixed(1)}%" height="8" rx="2"/>` : `<rect class="fill warn" width="${out.toFixed(1)}%" height="8" rx="2"/><rect class="starve" x="${out.toFixed(1)}%" width="${(end - out).toFixed(1)}%" height="8"/>`}<rect class="tick" x="${end.toFixed(1)}%" y="-5" width="2" height="18" rx="1" transform="translate(-1 0)"/></svg>`;
}
/** Column 4: ONE short phrase, with its tone and what a hover adds; `today` is the list price of today's tokens. */
function verdictOf(a, w, now, today) {
  if (a.needsLogin) return ['negT', a.p.identity.needs_login ? 'Login expired' : 'Not logged in', 'Its limits cannot be read until it is logged in'];
  if (a.cap) return ['plain', `≈ ${money(a.cap.spentUsd)} of ${money(a.cap.limitUsd)} today`, `Routed through ${a.route || 'the local router'}: the daily cap resets in ${fmtDuration(a.cap.resetMs)}`];
  if (a.kind !== 'subscription') return ['plain', `Pay per use${today != null ? ` · ≈ ${money(today)} today` : ''}`, a.kind === 'routed' ? `Routed through ${a.route || 'the local router'}; no daily cap` : 'An API key: billed per token, no plan limits'];
  if (!a.measured) return ['plain', a.noPlan ? 'No plan limits' : 'Limits not read', a.noPlan ? `${TOOL[a.tool] || a.tool} has no plan limits to read` : 'Its limits show after it runs on this computer, or after a refresh'];
  if (!w) return ['plain', '—', st.windowMode === '5h' ? 'No 5-hour window' : 'No weekly window'];
  const model = w.kind === 'model' ? modelName(w) + ' ' : '';
  if (w.open && w.left === 0) return ['negT', `${model ? model + 'used up' : 'Used up'} · back ${clock(w.resetsAt, now)}`, `${w.label}: nothing left until its reset ${fmtAbsoluteReset(w.resetsAt, now)}`];
  if (w.outAt) return ['warnT', `≈ ${model ? model + 'out' : 'Out'} ${dayOf(w.outAt, now)}`, `${w.label}: at this pace it runs out ${fmtDuration(w.resetMs - w.outMs)} before its reset ${fmtAbsoluteReset(w.resetsAt, now)} (an estimate)`];
  if (!w.open) return ['', 'Lasts until its reset', `${w.label}: no window is open; the next one starts with the next request`];
  return ['', `Lasts until ${clock(w.resetsAt, now)}`, `${w.label} resets ${fmtAbsoluteReset(w.resetsAt, now)}${w.forecastAtReset != null ? `; ≈ ${100 - w.forecastAtReset}% left then at this pace` : ''}`];
}
function quotaChips(a) {
  if (a.needsLogin || a.mismatch || a.kind !== 'subscription') return '';
  const { credits, unused_reset: reset } = a.p.quota || {}, now = Date.now();
  const value = freshCredits(credits, now) ? creditText(credits, true) : '';
  const creditChip = value ? `<span class="pill quiet" data-testid="quota-chip">${esc(a.tool === 'claude' ? 'Usage credits ' + value.toLowerCase() : value)}</span>` : '';
  const resetChip = reset?.holds_unused_reset && freshReset(reset, now) ? '<span class="pill quiet" title="Unused reset reported" data-testid="quota-chip">Unused reset</span>' : '';
  return creditChip || resetChip ? `<div class="row ov-quota-chips">${creditChip}${resetChip}</div>` : '';
}

/** One row of the runway: account | limits | lane | verdict | the one action that works in its state. */
function runwayRow(a, sel, used, pcs, now, today) {
  const w = governing(a), [tone, text, tip] = verdictOf(a, w, now, today), lock = lockOf(a);
  const hint = resetHint(a, w);
  const plan = a.needsLogin ? '' : a.kind === 'api_key' ? 'API key' : a.kind === 'routed' ? 'Routed' : a.p.identity.plan || '';
  const on = a.computers.map((id) => (pcs.find((c) => c.id === id) || {}).name || id);
  const where = a.openSessions ? `${plural(a.openSessions, 'session')} open${on.length ? ' on ' + on.join(', ') : ''}` : 'No session open';
  return `<tr class="${sel ? 'sel' : ''}" tabindex="0" data-act="home-select" data-name="${esc(a.id)}" data-key="lane-${esc(a.id)}" data-testid="lane-${esc(a.id)}" title="${esc(where)}">
    <td><div class="row">${toolIcon(a.tool, 16)}<b class="trunc ov-name">${esc(a.name)}</b>${plan ? `<span class="pill ov-plan" title="${esc(a.kind === 'routed' ? a.plan : plan)}">${esc(plan)}</span>` : ''}${a.isMain ? `<span class="pill quiet" title="The tool's own profile (${esc(a.folder)})">main</span>` : ''}</div>${quotaChips(a)}</td>
    <td>${limitsCell(a, used, now)}</td><td>${laneCell(a, w, now)}</td>
    <td><span class="ov-verdict trunc ${tone}" title="${esc(tip)}" data-testid="verdict-${esc(a.id)}">${esc(text)}</span>${hint ? `<span class="small muted trunc ov-verdict-note" title="${esc(hint)}" data-testid="reset-hint">${esc(hint)}</span>` : ''}</td>
    <td>${a.needsLogin ? `<button class="btn sm" data-act="login" data-name="${esc(a.id)}" data-testid="lane-login-${esc(a.id)}">Log in</button>` : `<button class="btn sm" data-act="start" data-name="${esc(a.id)}" ${lock ? `disabled title="Used up · back ${esc(clock(lock.resetsAt, now))}"` : `title="Start ${esc(a.name)} in a terminal"`} data-testid="lane-start-${esc(a.id)}">Start</button>`}</td></tr>`;
}

/** The account Use next names for one tool, among `accts` (accountViews), by the rule its card draws with: the most
 * room (or, with "5h first", the 5-hour window that resets soonest), else one with no plan limits to run out; null
 * when the tool has none. The command palette (palette.js) offers it as "Start …". */
export function recommendedFor(accts, tool) {
  const pool = accts.filter((a) => a.tool === tool), pm = metricsOf(pool);
  return (st.preferExpiring && pm.expiringAcct) || pm.bestRoomAcct || pool.find((a) => a.noPlan && !a.needsLogin) || null;
}

// ---------- what needs attention: ONE entry per account, the most severe first, each with its one action ----------
// the tools with a newer version, with both versions: read when the count the state gives changes ("2.10.0" is newer than "2.9.3")
const newer = (a, b) => { const x = a.split('.').map(Number), y = b.split('.').map(Number); for (let i = 0; i < Math.max(x.length, y.length); i++) { if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) > (y[i] || 0); } return false; };
let appsFor = null, updates = [];
function readApps(n) {
  if (appsFor === n) return;
  appsFor = n;
  call('apps').then((r) => {
    S.appVersions = Object.fromEntries(r.apps.filter((x) => x.version).map((x) => [x.id, x.version]));
    const latest = (r.latest && r.latest.versions) || {};
    updates = r.apps.filter((a) => a.path && a.version && latest[a.id] && newer(latest[a.id], a.version)).map((a) => ({ id: a.id, label: a.label, from: a.version, to: latest[a.id] }));
    show();
  }).catch(() => {});
}
function attention(m, accts, d, now) {
  const items = [], open = (href) => `<a class="btn sm" href="${href}">Open</a>`;
  // a session started here that waits for you (its hooks said so): first, oldest first, one line each; Open shows
  // the session with its request, answered there (sessions.js)
  for (const r of (S.needs || []).filter((x) => x.state === 'waiting')) {
    const what = r.ask ? (r.ask.kind === 'question' ? 'Question · ' + r.ask.what : r.ask.tool + (r.ask.what ? ' · ' + r.ask.what : '')) : r.reason || 'waiting';
    items.push({ key: 'run-' + r.run, rank: -1, at: r.ts, icon: toolIcon(r.harness, 16), name: acctName(r.profile), tone: 'warnT', title: [what, r.cwd].filter(Boolean).join('\n'), text: 'needs you: ' + what,
      btn: `<button class="btn sm" data-act="open-run" data-run="${esc(r.run)}" title="Open the session at its request" data-testid="att-open">Open</button>` });
  }
  for (const a of accts) {
    const risks = m.atRisk.filter((r) => r.acct === a), spent = risks.find((r) => r.win.left === 0), out = risks.find((r) => r.win.left > 0);
    if (!spent && !out && !a.needsLogin) continue;
    const all = [...risks.map((r) => (r.win.left === 0 ? `${r.win.label} used up, back ${fmtAbsoluteReset(r.win.resetsAt, now)}` : `${r.win.label} ≈ runs out ${fmtAbsoluteReset(r.win.outAt, now)}, ${fmtDuration(r.gapMs)} before its reset`)), ...(a.needsLogin ? [a.p.identity.needs_login ? 'The login expired, so its limits cannot be read' : 'Not logged in yet'] : [])];
    const hint = resetHint(a, (spent || out)?.win);
    items.push({ key: a.id, rank: a.needsLogin ? 2 : spent ? 0 : 1, at: spent ? spent.win.resetMs : out ? out.outMs : 0, icon: toolIcon(a.tool, 16), name: a.name, title: [...all, hint].filter(Boolean).join('\n'),
      tone: a.needsLogin || spent ? 'negT' : 'warnT', text: a.needsLogin ? (a.p.identity.needs_login ? 'Login expired' : 'Not logged in') : spent ? `${longName(spent.win)} used up · back ${clock(spent.win.resetsAt, now)}` : `≈ Out ${about(out.gapMs)} before reset`,
      btn: a.needsLogin ? `<button class="btn sm" data-act="login" data-name="${esc(a.id)}">Log in</button>` : open(`#account?name=${encodeURIComponent(a.id)}`) });
  }
  for (const [id, b] of Object.entries(d.dash.live.breakers || {})) if (b.state === 'open' && (!b.retry_at || b.retry_at > now)) items.push({ key: 'provider-' + id, rank: 3, at: 0, icon: '<span class="dot bad"></span>', name: id, tone: 'negT', title: 'A provider the router paused',
    text: `Paused after ${b.failures || 'repeated'} ${b.failures === 1 ? 'error' : 'errors'}${b.retry_at ? ' · retry in ' + fmtDuration(b.retry_at - now) : ''}`, btn: open('#routing') });
  const upd = (S.state && S.state.app_updates) || 0;
  for (const u of updates) items.push({ key: 'update-' + u.id, rank: 4, at: 0, icon: toolIcon(u.id, 16), name: u.label, tone: 'muted', title: `${u.label} ${u.to} is out; ${u.from} is installed`, text: `Update ${u.from} → ${u.to}`,
    btn: `<button class="btn sm" data-act="app-update" data-id="${esc(u.id)}" data-label="${esc(u.label)}" data-to="${esc(u.to)}">Update</button>` });
  // the count is known before the versions are read
  if (upd && !updates.length) items.push({ key: 'updates', rank: 4, at: 0, icon: '<span class="dot"></span>', name: 'Tool updates', tone: 'muted', title: 'Apps installs the newer version', text: `${plural(upd, 'tool has', 'tools have')} a newer version`, btn: open('#apps') });
  items.sort((x, y) => x.rank - y.rank || x.at - y.at);
  return `<section class="card" aria-label="Needs attention" data-key="attention" data-testid="attention"><div class="card-head"><h2 class="section">Needs attention</h2>${items.length ? `<span class="pill warn" title="${plural(items.length, 'entry', 'entries')} to act on" data-testid="attention-count">${items.length}</span>` : '<span class="small muted">Nothing</span>'}</div>
    <div class="ov-list" ${items.length > 4 ? 'tabindex="0" aria-label="Everything that needs attention; scroll for more"' : ''}>${items.map((i) => `<div class="ov-item" data-key="att-${esc(i.key)}" data-testid="att-${esc(i.key)}" title="${esc(i.title)}">${i.icon}<div class="two"><b class="trunc">${esc(i.name)}</b><span class="small trunc ${i.tone}">${esc(i.text)}</span></div>${i.btn}</div>`).join('') || '<div class="ov-empty">Every limit lasts until its reset at this pace, every login is valid, no provider is paused.</div>'}</div></section>`;
}

// ---------- the latest replies and the request log ----------
function repliesTable(rows, pcs, peers) {
  return `<table class="replies-table" data-testid="latest-replies"><thead><tr><th>Time</th><th>Account</th><th>Model</th>${peers ? '<th>Computer</th>' : ''}<th class="num">Tokens</th><th class="num" title="${TIMING}">First reply</th></tr></thead><tbody>
    ${rows.slice(0, 5).map((r) => `<tr><td>${timeCell(r.ts)} <span class="ago">(${ago(Date.now() - r.ts)})</span></td><td><span class="cell-inline">${toolIcon(r.harness, 14)}<b>${esc(acctName(r.profile))}</b></span></td><td>${esc(r.model)}</td>${peers ? `<td><span class="cell-inline">${r.device ? pcDot(pcs, r.device) + esc(nameOf(S.cache.computers, r.device)) : ''}</span></td>` : ''}<td class="num">${fmt(r.input + r.output + r.cache_read + r.cache_write)}</td><td class="num ${r.ttft_ms > 4000 ? 'warnT' : ''}" data-testid="ttft">${firstReply(r)}</td></tr>`).join('') || `<tr><td colspan="${peers ? 6 : 5}" class="muted">No replies in this range.</td></tr>`}</tbody></table>`;
}
function requestTable(rows) {
  return `<table class="replies-table" data-testid="request-log"><thead><tr><th>Time</th><th>Account</th><th>Model</th><th>Computer</th><th class="num">Input</th><th class="num">Output</th><th class="num">Cache read</th><th class="num">Cache write</th><th class="num" title="${TIMING}">First reply</th><th class="num">Cost ≈</th></tr></thead><tbody>
    ${rows.map((r) => `<tr><td class="mono">${timeCell(r.ts)}</td><td><span class="cell-inline">${toolIcon(r.harness, 14)}<b>${esc(acctName(r.profile))}</b></span></td><td class="mono">${esc(r.model)}</td><td>${r.device ? esc(nameOf(S.cache.computers, r.device)) : esc((S.state && S.state.computer) || '')}</td><td class="num">${fmt(r.input)}</td><td class="num">${fmt(r.output)}</td><td class="num">${fmt(r.cache_read)}</td><td class="num">${fmt(r.cache_write)}</td><td class="num" data-testid="ttft">${firstReply(r)}</td><td class="num"><b>${money(r.cost, 4)}</b></td></tr>`).join('') || '<tr><td colspan="10" class="muted">No replies in this range.</td></tr>'}</tbody></table>`;
}
function openRequestLog() {
  openDialog(`${dlgHead('Request log')}<span class="panel-note">${st.total.toLocaleString()} ${st.total === 1 ? 'request' : 'requests'} ${RANGE_LABEL[st.range].toLowerCase()} · costs are what these tokens would cost at API list prices (models.dev); subscriptions are not billed per token · first reply is approximate</span>
    <div class="table-wrap" id="home-table">${requestTable(st.rows)}</div>${st.rows.length < st.total ? '<div class="more-row"><button class="btn-secondary" data-act="home-more" data-testid="home-more">Show 50 more</button></div>' : ''}`, { size: 'xl', label: 'Request log' });
}

// ---------- the first-run and empty states ----------
function firstRun(state, accts, tools) {
  if (!accts.length) {
    const none = `${tools.some((t) => t.installed) ? 'No account yet' : 'No AI coding tool found'} on ${esc(state.computer || 'this computer')}`;
    return `<div class="page-body">${pageHead('home', none + '.')}<section class="panel-card empty-state center"><div class="empty-logo" id="empty-mark"></div><h2>${none}</h2>
      <div class="tool-pick-grid full">${tools.map((t) => `<div class="tool-pick-card"><div class="tool-pick-head"><span class="tool-pick-name">${toolIcon(t.id, 14)} ${esc(t.label)}</span><span class="plan-pill">${t.installed ? 'Installed' : 'Not installed'}</span></div><div class="tool-pick-sup">Supports: ${['Subscription', t.caps.api_key ? 'API key' : '', t.caps.routable ? 'Routed' : ''].filter(Boolean).join(' · ')}</div><button class="btn-secondary" data-act="add-account-for" data-h="${t.id}">Prepare ${esc(t.label)} account</button></div>`).join('')}</div>
      <div class="row-actions"><button class="btn-primary" data-act="add-account" data-testid="add-account">+ Add first account</button></div></section></div>`;
  }
  return `<div class="page-body">${pageHead('home', `${plural(accts.length, 'account')} found on ${esc(state.computer || 'this computer')}; limits have not been read yet.`)}<section class="panel-card first-run-card"><div class="eyebrow-row"><span class="eyebrow ac">Profiles on ${esc(state.computer || 'this computer')} · ${accts.length} ${accts.length === 1 ? 'profile' : 'profiles'} ready</span><span class="eyebrow-note">Limits not read (—) until first sign-in or refresh</span></div>
    <div class="first-run-grid"><div class="first-run-col"><h2>Limits have not been read yet.</h2><div class="detected-profiles">${accts.map((a) => `<div class="live-mini-row" data-key="fr-${esc(a.id)}"><div class="live-mini-left">${toolIcon(a.tool, 14)}<b>${esc(a.name)}</b><span class="plan-pill">${a.isMain ? 'main profile' : esc(a.plan)}</span><span class="mono">${esc(a.folder)}</span><span class="note">· ${a.noPlan ? 'Usage only · no plan limits' : '— limits not read'}</span></div>${a.needsLogin ? `<button class="btn-lane-start" data-act="login" data-name="${esc(a.id)}">Log in</button>` : `<button class="btn-lane-start" data-act="start" data-name="${esc(a.id)}">▶ Start</button>`}</div>`).join('')}</div>
      <div class="row-actions"><button class="btn-primary" data-act="add-account" data-testid="add-account">+ Add another account</button><button class="btn-secondary" data-act="refresh-now">Read limits now</button></div></div>
      <div class="panel-card sub"><h2 class="panel-title">Sync limits and live sessions across your computers?</h2><div class="sync-choices">
        <button class="candidate-pill" data-act="firstrun-later" data-testid="firstrun-later"><div><b>Not now — this computer only</b><div class="cand-sub">Turn on multi-computer sync anytime on the Computers page</div></div><span>Local</span></button>
        <button class="candidate-pill" data-act="firstrun-folder" data-testid="firstrun-folder"><div><b>Shared folder (OneDrive, Dropbox, iCloud, SMB)</b><div class="cand-sub">Syncs limits and sessions across your computers</div></div><span class="acT">Folder</span></button>
        <button class="candidate-pill" data-act="firstrun-hub" data-testid="firstrun-hub"><div><b>A Harness Hub you host</b><div class="cand-sub">A real-time relay over Tailscale or your LAN</div></div><span>wss://</span></button></div></div></div></section></div>`;
}
action('firstrun-later', () => { try { localStorage.setItem('hm.firstrun', '1'); } catch {} render(); });

// ---------- the page ----------
// what the page last read: a choice that only changes what is shown draws from it again, without asking the backend
let last = null;
view('home', async (sheet) => {
  sheet.className = 'sheet';
  const wk = 'home:' + JSON.stringify([st.range, st.harness, scope()]);
  warmPage(sheet, wk);
  const hours = RANGES.find((r) => r[0] === st.range)[1], days = Math.round(hours / 24);
  const statep = loadState(), pcsp = computersStatus().catch(() => null);
  // scope: every computer added up, or this one only; the sync core answers the same shape either way. Beside it, only
  // when the page has a use for them: today's list price per account (pay-per-use accounts), and the running sessions' titles
  const [state, d, pcs, cost, open] = await Promise.all([statep, call('home', { days, hours, harness: st.harness, limit: 50, scope: scope() }), pcsp,
    statep.then((s) => (s.profiles.some((p) => p.kind !== 'subscription') ? call('usage', { days: 1 }).catch(() => null) : null)),
    Promise.all([statep, pcsp]).then(([s, c]) => (s.profiles.some((p) => p.running.length) || (syncOn(c) && (c.peers || []).some((p) => (p.running || []).length)) ? call('sessions', { filter: { open: true, per: 60 }, group: 'project', scope: 'all' }).catch(() => null) : null))]);
  st.rows = d.requests.rows; st.total = d.requests.total;
  last = { state, d, pcs, wk, today: Object.fromEntries(((cost && cost.accounts) || []).map((b) => [b.key, b.cost])), titles: Object.fromEntries(((open && open.groups) || []).flatMap((g) => g.sessions).map((s) => [s.profile + '/' + s.id, s])) };
  readApps(state.app_updates || 0);
  draw(sheet);
});
/** Draw the page again from what was last read: after a choice that only changes what is shown. */
function show() {
  if (S.drawnView !== 'home') return;
  if (last && S.drawn === location.hash) draw(document.getElementById('sheet')); else render();
}

function draw(sheet) {
  const { state, d, pcs, wk, today, titles } = last;
  const o = d.overview, k = d.dash, now = k.now, used = showUsed();
  const tools = shownTools(state), accts = accountViews(state, k, now, pcs), pcl = pcList(state, pcs), me = pcl[0].id;
  const sessions = runningSessions(state, d, pcs, me, titles);
  const isFresh = !accts.some((a) => a.measured) && !d.requests.total && !sessions.length;
  if (isFresh && (!accts.length || !localStorage.getItem('hm.firstrun'))) { patch(sheet, firstRun(state, accts, tools)); const m = sheet.querySelector('#empty-mark'); if (m) m.innerHTML = document.getElementById('brand-mark').innerHTML; keepPage(sheet, wk); return; }
  const m = metricsOf(accts), risk = m.primaryRisk, peers = syncOn(pcs) && (pcs.peers || []).length > 0;
  const fmtAt = (at) => fmtAbsoluteReset(at, now);
  // the status: the accounts used up or on pace to run out before their reset (two at most), else what runs now
  const seen = new Set(), risks = m.atRisk.filter((r) => !seen.has(r.acct.id) && seen.add(r.acct.id)).slice(0, 2);
  const status = risks.length
    ? risks.map((r) => `${esc(r.acct.name)} ${r.win.left === 0 ? 'is used up until ' + fmtAt(r.win.resetsAt).replace(/^Today /, '') : 'will run out ' + fmtAt(r.win.outAt).replace(/^Today /, 'today at ') + ', before its reset'}`).join('; ') + '.'
    : `${sessions.length ? plural(sessions.length, 'session is', 'sessions are') + ' running now' : 'No session is running'}; ${accts.some((a) => a.measured) ? 'no account is on pace to run out before its reset' : 'limits have not been read yet'}.`;

  // ---- Use next: the same rule as ever (most room; or the 5-hour window that resets soonest), among one tool's accounts when one is chosen
  const useTools = tools.filter((h) => accts.some((a) => a.tool === h.id)), useTool = useTools.some((h) => h.id === st.useTool) ? st.useTool : '';
  const pool = useTool ? accts.filter((a) => a.tool === useTool) : accts, pm = metricsOf(pool);
  const share = (w) => `${used ? w.used : w.left}% of ${w.kind === 'week' ? 'the week' : w.kind === '5h' ? 'the 5-hour window' : esc(w.label)} ${used ? 'used' : 'left'}`;
  let next = st.preferExpiring && pm.expiringAcct ? pm.expiringAcct : pm.bestRoomAcct, why = '';
  if (next) {
    const w5 = w5of(next), w = wkof(next), soon = st.preferExpiring && w5 && w5.open && w5.left >= 50;
    why = soon ? `${share(w5)} · resets in ${fmtDuration(w5.resetMs)}` : `${share(w)} · ${w.outAt ? '≈ out ' + dayOf(w.outAt, now) : 'lasts until its reset'}`;
  } else if (useTool && (next = pool.find((a) => a.noPlan && !a.needsLogin))) why = next.kind === 'subscription' ? 'No plan limits to run out' : 'Pay per use · no plan limits';
  const runner = next && next.measured ? metricsOf(pool.filter((a) => a !== next)).bestRoomAcct : null, rw = runner && wkof(runner);
  const toolName = useTool ? (useTools.find((h) => h.id === useTool) || {}).label : '';
  // each tool by name while two fit beside the heading; more of them by their icon, named on hover
  const wide = useTools.length <= 2;
  const useSeg = segOf('Recommend for', [['', 'Any', '', 'use-any'], ...useTools.map((h) => [h.id, toolIcon(h.id, 14) + (wide ? esc(h.label) : ''), h.label, 'use-' + h.id])], useTool, 'home-use-tool', 'tight');
  const useCard = `<section class="card" data-key="hero" data-testid="use-next" aria-label="Use next"><div class="card-head"><h2 class="section">Use next</h2>${useSeg}</div>
    <div class="ov-use">${next ? `<div class="row ov-who">${toolIcon(next.tool, 28)}<b class="trunc" data-testid="use-name">${esc(next.name)}</b>${next.measured && next.p.identity.plan ? `<span class="pill ov-plan" title="${esc(next.p.identity.plan)}">${esc(next.p.identity.plan)}</span>` : ''}${next.isMain ? '<span class="pill quiet">main</span>' : ''}</div>
        <p class="ov-why trunc" title="${next.readAt ? 'Limits read ' + ago(now - next.readAt) : ''}" data-testid="use-why">${why}</p>${quotaChips(next)}`
      : `<div class="row ov-who"><b class="trunc">No ${esc(toolName ? toolName + ' ' : '')}account with measured room</b></div><p class="ov-why trunc">${plural(pool.length, 'account')} · ${pool.some((a) => a.needsLogin) ? 'log in, then ' : ''}refresh to read limits (—)</p>`}
      <div class="row">${next ? `<button class="btn primary" data-act="start" data-name="${esc(next.id)}" data-testid="start-best"><svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M2 1l7 4-7 4z" fill="currentColor"/></svg>Start</button><button class="btn" data-act="start-in-folder" data-name="${esc(next.id)}" title="Pick the folder, a recent one or any, and the flags" data-testid="start-folder">Start in folder…</button>` : '<button class="btn primary" data-act="refresh-now">Read limits</button>'}
        <button class="btn sm quiet ov-prefer ${st.preferExpiring ? 'on' : ''}" data-act="home-prefer" aria-pressed="${st.preferExpiring}" title="Prefer the account whose 5-hour window resets soonest and still has at least half its room" data-testid="use-prefer">${st.preferExpiring ? '✓ ' : ''}5h first</button></div>
      <div class="row small ov-runner" data-testid="use-runner"><span class="muted">Runner-up</span>${runner ? `${toolIcon(runner.tool, 14)}<b>${esc(runner.name)}</b><span class="muted trunc">${used ? rw.used + '% used' : rw.left + '% left'} · ${rw.outAt ? '≈ out ' + dayOf(rw.outAt, now).split(' ')[0] : 'lasts'}</span>` : '<span class="muted">none</span>'}</div></div></section>`;

  // ---- Running now: a row per session; the header counts them per computer, and a count shows that computer's alone
  const live = sessions.filter((s) => s.computer === me).length, others = pcl.slice(1).map((c) => ({ ...c, n: sessions.filter((s) => s.computer === c.id).length })).filter((c) => c.n);
  const livePc = st.livePc !== 'all' && sessions.some((s) => s.computer === st.livePc) ? st.livePc : 'all';
  const shownSessions = sessions.filter((s) => livePc === 'all' || s.computer === livePc).sort((a, b) => (b.streaming - a.streaming) || ((a.since || 0) - (b.since || 0)));
  const count = (id, text) => (others.length && sessions.some((s) => s.computer === id) ? `<button type="button" class="ov-count ${livePc === id ? 'on' : ''}" aria-pressed="${livePc === id}" data-act="home-live-pc" data-v="${esc(id)}" title="Show only these; again for all">${text}</button>` : `<span>${text}</span>`);
  const runCard = `<section class="card" data-key="running" data-testid="running-now" aria-label="Running now"><div class="card-head"><h2 class="section">Running now</h2><span class="small muted ov-counts" title="The local router is ${state.router.running ? 'running on :' + esc(state.router.port) : 'stopped'}" data-testid="running-count">${count(me, `${live} here`)}${others.map((c) => ` · ${count(c.id, `${c.n} on ${esc(c.name)}`)}`).join('')}</span></div>
    <div class="ov-list" ${shownSessions.length > 4 ? 'tabindex="0" aria-label="All running sessions; scroll for more"' : ''}>${shownSessions.map((s) => `<div class="ov-item" data-key="live-${esc(s.computer)}-${esc(s.id || s.pid)}"><div class="two"><div><b class="trunc grow" title="${esc(s.title)}">${esc(s.title)}</b>${s.computer === me && s.id ? needsMark(s.accountId, s.id) : ''}<span class="meta" title="${s.streaming ? 'Its account sent tokens in the last minutes' : 'Open'}">${s.since ? fmtDuration(now - s.since) : ''}</span></div>
        <div class="meta"><span class="trunc">${esc(s.project)}</span><span class="g">${s.tool ? toolIcon(s.tool, 14) : ''}${esc(acctName(s.accountId))}</span><span class="g">${pcDot(pcl, s.computer)}${esc(nameOf(pcs, s.computer) || pcl[0].name)}</span></div></div>
        ${s.id ? `<button class="btn sm" data-act="open-session" data-profile="${esc(s.accountId)}" data-id="${esc(s.id)}" ${s.computer !== me ? `data-device="${esc(s.computer)}"` : ''}>Follow</button>` : `<button class="btn sm" data-act="show-window" data-pid="${s.pid}" title="Bring its console window to the front">Show</button>`}</div>`).join('') || '<div class="ov-empty">No session is running.</div>'}</div></section>`;

  // ---- the accounts runway
  const selected = accts.find((a) => a.id === st.selected) || (risk ? risk.acct : accts[0]);
  const filtered = accts.filter((a) => (!st.harness || a.tool === st.harness) && (scope() !== 'here' || !a.openSessions || a.computers.includes(me)));
  // a tool is named while the head line has room at the usual window width (about 590 px for the tools and the
  // computers' choice: 55 px a tool, 6.6 px a letter); in a fuller line, and a tool without accounts, by icon and count
  const nOf = (h) => accts.filter((a) => a.tool === h.id).length;
  const named = tools.reduce((n, h) => n + 55 + (nOf(h) ? 6.6 * h.label.length : 0), 0) + (peers ? 127 : 0) <= 590;
  const toolSeg = segOf('Filter by tool', [['', `All <span class="dim">${accts.length}</span>`, '', 'tool-all'], ...tools.map((h) => [h.id, `${toolIcon(h.id, 14)}${named && nOf(h) ? esc(h.label) + ' ' : ''}<span class="dim">${nOf(h)}</span>`, h.label, 'tool-' + h.id])], st.harness, 'home-tool');
  const ttft = (id) => { const t = st.rows.filter((r) => r.profile === id && r.ttft_ms != null).map((r) => r.ttft_ms).sort((a, b) => a - b); return t.length ? `≈ ${(t[Math.floor(t.length / 2)] / 1000).toFixed(1)}s` : '—'; };
  const runway = `<section class="card ov-runway" aria-label="Accounts runway" data-testid="horizon"><div class="ov-tbl-head" data-testid="home-filters"><h2 class="section">Accounts runway</h2>${toolSeg}
      <span class="ov-picks">${pick('home-window-menu', 'The window the lanes draw', WINDOWS.find((x) => x[0] === st.windowMode)[1], 'window-pick')}${pick('home-used-menu', 'Limits shown as', used ? '% used' : '% left', 'used-pick')}${peers ? pick('home-scope-menu', 'Computers', scope() === 'here' ? 'This computer' : 'All computers', 'scope') : ''}
      <button type="button" class="btn sm icon" data-act="home-legend" aria-haspopup="menu" aria-label="Legend" title="Legend: a bar shows what is ${used ? 'used' : 'left'}. The lane runs from now to ${st.windowMode === '5h' ? '+5 h' : '+7 d'}; the tick is the reset; a striped stretch is an estimated time with nothing left before the reset." data-testid="legend">i</button></span></div>
    <table class="ov-table"><colgroup><col class="c-acct"><col class="c-lims"><col><col class="c-verdict"><col class="c-act"></colgroup>
      <thead><tr><th>Account</th><th>Limits ${used ? 'used' : 'left'}</th><th><div class="ov-axis"><span>Now</span><span>${st.windowMode === '5h' ? '+5 h' : '+7 d'}</span></div></th><th>Verdict</th><th aria-label="Action"></th></tr></thead>
      <tbody>${filtered.map((a) => runwayRow(a, a.id === st.selected, used, pcl, now, today[a.id])).join('') || '<tr class="none"><td colspan="5" class="muted">No account of that tool.</td></tr>'}</tbody></table>
    ${selected ? `<div class="route-inspector-bar" data-testid="route"><div class="route-chain"><span class="route-label">Selected route:</span><span class="route-node"><span class="pc-dots">${(selected.computers.length ? selected.computers : [me]).map((id) => `${pcDot(pcl, id)}<span>${esc(nameOf(pcs, id) || pcl[0].name)}</span>`).join(' + ')}</span></span><span class="route-arrow">→</span><span class="route-node">${toolIcon(selected.tool, 14)}<b>${esc(selected.name)}</b><span class="route-muted">(${selected.measured ? `${wkof(selected).used}% ${esc(wkof(selected).label.toLowerCase())} used` : '— not read'})</span></span><span class="route-arrow">→</span><span class="route-node"><b>${esc(TOOL[selected.tool] || selected.tool)}</b>${(S.appVersions || {})[selected.tool] ? `<span class="route-muted">v${esc(S.appVersions[selected.tool])}</span>` : ''}<span class="route-live">· first reply ${ttft(selected.id)} · ${selected.openSessions} open</span></span></div>
      <div class="route-right">${st.selected ? '<span class="esc-hint"><kbd class="kbd">Esc</kbd> resets selection</span>' : ''}<a class="btn sm" href="#account?name=${encodeURIComponent(selected.id)}" data-testid="route-open">Open ${esc(selected.name)}</a></div></div>` : ''}</section>`;

  // ---- below the table, as before: the year clock and the latest replies
  const yc = yearClock(o.heatmap, pcs, pcl.some((c) => c.id === st.device) ? st.device : '');
  const byDev = pcl.map((c) => ({ ...c, tokens: o.heatmap.reduce((s, dd) => s + ((dd.by_device || {})[c.id] || 0), 0) }));
  patch(sheet, `<div class="page-body ov">${pageHead('home', status, seg('Readout range', RANGES.map(([r]) => [r, RANGE_LABEL[r]]), st.range, (v) => `data-act="home-range" data-v="${v}" data-testid="range-${v}"`) + readouts(k, pcl.length))}
    <div class="ov-deck">${useCard}${attention(m, accts, d, now)}${runCard}</div>
    ${runway}
    <div class="ov-lower"><section class="panel-card" data-testid="activity"><div class="panel-head"><h2 class="panel-title">Activity, 53 weeks</h2>${peers ? `<span data-testid="activity-pc">${segOf('Year clock computer', [['', 'All', '', 'activity-pc-all'], ...pcl.map((c) => [c.id, `<i class="pc-dot c${c.ci}"></i><span class="pc-name">${esc(c.name)}</span>`, c.name, 'activity-pc-' + c.id])], pcl.some((c) => c.id === st.device) ? st.device : '', 'home-device')}</span>` : ''}</div>
        <div class="year-clock-wrap">${yc.svg}<div class="year-clock-facts"><div><b class="ink">Monday inside</b> · daily tokens</div><div class="muted">Busiest: <b class="ink">${yc.bv ? new Date(yc.best.day + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short' }) + ' (' + fmt(yc.bv) + ')' : '—'}</b></div><div class="muted">53-wk total: <b class="ac">${fmt(yc.total)}</b></div>${peers ? `<div class="year-clock-pcs">${byDev.map((c) => `<span><i class="pc-dot c${c.ci} mr"></i>${esc(c.name)}: ${fmt(c.tokens)} (53w)</span>`).join('')}</div>` : ''}</div></div></section>
      <section class="panel-card" aria-label="Latest replies" data-key="replies"><div class="panel-head"><div class="head-group"><h2 class="panel-title">Latest replies</h2><span class="panel-note">${st.total.toLocaleString()} ${RANGE_LABEL[st.range].toLowerCase()} · first reply is approximate (≈)</span></div><button class="btn sm" data-act="home-log" title="Every request of the range, with its tokens and cost" data-testid="request-log-open">Request log</button></div>${repliesTable(st.rows, pcl, peers)}</section></div>
  </div>`);
  keepPage(sheet, wk);
}

// a choice that changes what is read (the range, the tool, the computers) reads again; the others only draw again
action('home-range', (el) => { st.range = el.dataset.v; render(); });
action('home-tool', (el) => { st.harness = el.dataset.v; render(); });
action('home-use-tool', (el) => { st.useTool = el.dataset.v; try { localStorage.setItem(USE_KEY, st.useTool); } catch {} show(); });
action('home-device', (el) => { st.device = el.dataset.v; show(); });
action('home-live-pc', (el) => { st.livePc = st.livePc === el.dataset.v ? 'all' : el.dataset.v; show(); });
action('home-prefer', () => { st.preferExpiring = !st.preferExpiring; show(); });
action('home-select', (el) => { st.selected = el.dataset.name; show(); });
action('home-window-menu', (el) => openMenu(el, WINDOWS.map(([v, label]) => ({ label, on: st.windowMode === v, testid: 'window-' + v, run: () => { st.windowMode = v; show(); } }))));
action('home-used-menu', (el) => openMenu(el, [['left', '% left'], ['used', '% used']].map(([v, label]) => ({ label, on: showUsed() === (v === 'used'), testid: 'used-' + v, run: () => mutate(() => call('settings_set', { key: 'quota_display', value: v })) }))));
action('home-scope-menu', (el) => openMenu(el, [['all', `All computers (${computers(S.cache.computers).length})`], ['here', `This computer (${(S.state && S.state.computer) || 'this computer'})`]].map(([v, label]) => ({ label, on: scope() === v, testid: 'scope-' + v, run: () => { setScope(v); render(); } }))));
// the legend, drawn with the marks it explains
action('home-legend', (el) => openMenu(el, [
  ['<svg class="ov-bar"><rect width="64%" height="100%" rx="3"/></svg>', `A bar: what is ${showUsed() ? 'used' : 'left'} of a limit (measured)`],
  ['<svg class="ov-lane"><rect class="fill" width="70%" height="8" rx="2"/><rect class="tick" x="70%" y="-5" width="2" height="18" rx="1"/></svg>', `The lane: now to ${st.windowMode === '5h' ? '+5 h' : '+7 d'}; the tick is the reset`],
  ['<svg class="ov-lane"><rect class="fill warn" width="35%" height="8" rx="2"/><rect class="starve" x="35%" width="35%" height="8"/><rect class="tick" x="70%" y="-5" width="2" height="18" rx="1"/></svg>', 'Striped: ≈ nothing left before the reset (an estimate)'],
  ['<svg class="ov-lane none"><line x1="0" x2="100%" y1="4" y2="4"/></svg>', '— Not read, or no plan limits'],
].map(([mark, text]) => ({ html: `<span class="ov-legend">${mark}<span>${esc(text)}</span></span>`, run: () => {} }))));
action('home-log', () => openRequestLog());
action('home-more', async (el) => {
  if (el.disabled) return;
  el.disabled = true;
  const base = st.rows, hours = RANGES.find((x) => x[0] === st.range)[1];
  const r = await attempt(() => call('requests', { days: Math.max(1, Math.round(hours / 24)), hours, harness: st.harness, limit: 50, offset: base.length }));
  el.disabled = false;
  if (!r || st.rows !== base) return; // the list was redrawn (another filter) while this page loaded
  st.rows = base.concat(r.rows);
  const t = document.getElementById('home-table');
  if (t) t.innerHTML = requestTable(st.rows);
  if (st.rows.length >= r.total) el.remove();
});
// Escape with nothing open returns the selection to the account most at risk
document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && S.route.view === 'home' && st.selected && !dialogOpen() && !document.querySelector('.menu')) { st.selected = null; show(); } });
