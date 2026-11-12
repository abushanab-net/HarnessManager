// Overview, as the reference draws it: the dispatch deck (use next, runs out first, running now), the Reset Horizon
// lanes (ring, capacity, the 5-hour and weekly time tracks, next reset and pace, Start), then the readouts, what
// needs attention and the 53-week year clock, and the latest replies with the request log behind a button.
// Measured numbers are plain; estimates carry ≈; — means not read. No tokens-per-second on this page.

import { S, call, view, render, attempt, changed, toast, loadState, action, acctName, refreshText, shownTools, warmPage, keepPage, patch, openDialog, dlgHead, dialogOpen, mutate } from './core.js';
import { esc, fmt, hm, dur, toolIcon } from './lib.js';
import { accountViews, metricsOf, renderRingSvg, fmtDuration, fmtAbsoluteReset, money, TOOL, eligible, w5of, wkof, showUsed } from './accounts.js';
import { computersStatus, syncOn, computers, nameOf, scope, setScope } from './computers.js';

const RANGES = [['today', 0], ['7d', 168], ['30d', 720], ['all', 24 * 400]], RANGE_LABEL = { today: 'Today', '7d': '7 d', '30d': '30 d', all: 'All' };
// the page's own choices: the tool chips, which computer's sessions, the readout range, the year clock's computer,
// which windows the lanes draw, the hero's mode, the selected lane, and the loaded replies
const st = { harness: '', livePc: 'all', range: 'today', device: '', windowMode: 'both', preferExpiring: false, selected: null, rows: [], total: 0 };
const H = 3600e3, DAY = 24 * H, HORIZON = 7 * DAY;
const ago = (ms) => (ms < 60000 ? 'just now' : dur(ms) + ' ago');
const TIMING = 'Approximate: worked out from the times each tool wrote the parts of the answer to its log';
const firstReply = (r) => (r.ttft_ms != null ? `≈ ${(r.ttft_ms / 1000).toFixed(1)} s` : '<span class="dash">—</span>');
const speed = (r) => (r.gen_ms >= 300 && r.output > 0 ? `≈ ${Math.round(r.output / (r.gen_ms / 1000)).toLocaleString()} tok/s` : '<span class="dash">—</span>');
const timeCell = (ts) => (new Date(ts).toDateString() === new Date().toDateString() ? hm(ts) : new Date(ts).toLocaleDateString([], { day: 'numeric', month: 'short' }) + ' ' + hm(ts));

export function setTodayBadge(cost) {
  const el = document.getElementById('today-cost');
  if (el) { el.textContent = cost == null ? '' : money(cost); el.title = 'Today, at API list prices'; }
}

// ---------- the computers: this one first, each with the colour its dot has everywhere ----------
function pcList(state, pcs) {
  const list = syncOn(pcs) ? computers(pcs) : [{ id: pcs && pcs.this ? pcs.this.id : 'this', name: (pcs && pcs.this && pcs.this.name) || state.computer || 'this computer' }];
  return list.map((c, i) => ({ id: c.id, name: c.name, ci: i % 4 }));
}
const pcDot = (pcs, id, title = '') => { const c = pcs.find((x) => x.id === id); return c ? `<i class="pc-dot c${c.ci}" title="${esc(title || c.name)}"></i>` : ''; };

/** The sessions running now: this computer's consoles and, with sync on, every other computer's. */
function runningSessions(state, d, pcs, me) {
  const by = Object.fromEntries(state.profiles.map((p) => [p.name, p]));
  const here = state.profiles.flatMap((p) => p.running.map((r) => ({ id: r.session_id || '', pid: r.pid, project: (r.cwd || '').split(/[\\/]/).filter(Boolean).pop() || 'folder unknown', computer: me, accountId: p.name, tool: p.harness, since: r.started_at, streaming: !!((d.dash.live.accounts || {})[p.name] || {}).tokens })));
  const far = syncOn(pcs) ? (pcs.peers || []).flatMap((pc) => (pc.running || []).map((r) => ({ id: r.session_id || '', project: (r.cwd || '').split(/[\\/]/).filter(Boolean).pop() || 'folder unknown', computer: pc.id, accountId: r.profile, tool: (by[r.profile] || {}).harness || '', since: r.started_at, streaming: false }))) : [];
  return [...here, ...far];
}

// ---------- the readouts: the number, a dot-matrix level per period, the change against the period before ----------
function dotMatrix(vals) {
  const v = (vals || []).slice(-8).map((x) => x || 0), max = Math.max(...v, 1e-9);
  while (v.length < 8) v.unshift(0);
  const lit = v.map((x) => (x > 0 ? Math.max(1, Math.round((x / max) * 3)) : 0));
  let s = '<div class="dot-matrix" aria-hidden="true">';
  for (let r = 0; r < 3; r++) for (let c = 0; c < 8; c++) s += `<i class="dot-cell ${lit[c] >= 3 - r ? 'lv-4' : 'lv-0'}"></i>`;
  return s + '</div>';
}
function change(cur, prev, kind) {
  if (cur == null || prev == null) return '—';
  if (kind === 'pct') { if (!prev) return cur ? 'new' : '—'; const d = Math.round(((cur - prev) / prev) * 100); return `≈ ${d > 0 ? '+' : d < 0 ? '−' : '±'}${Math.abs(d)}%`; }
  if (kind === 'money') { const d = cur - prev; return `≈ ${d >= 0 ? '+' : '−'}${money(Math.abs(d), 0)}`; }
  const d = Math.round((cur - prev) * 100); return `≈ ${d >= 0 ? '+' : '−'}${Math.abs(d)} pt`;
}
function readouts(k, nPcs) {
  const c = k.cur, p = k.prev, sp = k.spark || {}, fresh = !p.requests || st.range === 'all';
  const vs = (...a) => (fresh ? '—' : change(...a));
  const rd = (key, label, value, delta, vals, title) => `<div class="readout-item" title="${esc(title)}" data-testid="kpi-${key}"><span class="readout-label">${label}</span><span class="readout-val">${value}</span>${dotMatrix(vals)}<span class="readout-delta">${esc(delta)}</span></div>`;
  return `<section class="panel-card" aria-label="Period readouts"><div class="panel-head"><h2 class="panel-title">Period readouts · trend</h2><div class="chip-group" role="group" aria-label="Readout range">${RANGES.map(([r]) => `<button class="filter-chip sm ${st.range === r ? 'active' : ''}" aria-pressed="${st.range === r}" data-act="home-range" data-v="${r}" data-testid="range-${r}">${RANGE_LABEL[r]}</button>`).join('')}</div></div>
    <div class="readout-rows">${rd('tokens', 'Tokens', fmt(c.tokens), vs(c.tokens, p.tokens, 'pct'), sp.tokens, `${c.requests.toLocaleString()} ${c.requests === 1 ? 'request' : 'requests'} ${RANGE_LABEL[st.range].toLowerCase()}`)}${rd('cost', 'Cost ≈', money(c.cost, c.cost >= 100 ? 0 : 2), vs(c.cost, p.cost, 'money'), sp.cost, 'At API list prices; subscriptions are not billed per token')}${rd('hit', 'Cached', `${Math.round(c.hit * 100)}%`, vs(c.hit, p.hit, 'pt'), (sp.hit || []).map((x) => Math.max(0, x - 0.5)), 'The share of input read from the cache')}</div>
    <div class="panel-foot"><span>At API list prices (≈) · subscriptions are not billed per token</span><span>${nPcs} ${nPcs === 1 ? 'computer' : 'computers'} combined</span></div></section>`;
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

// ---------- what needs attention: a window running out, a login to renew, a paused provider, tools with updates ----------
function attention(m, accts, d, now) {
  const items = [];
  for (const r of m.atRisk) items.push({ sev: r.win.left === 0 ? 'neg' : 'warn', t: `${esc(r.acct.name)} ${esc(r.win.label.toLowerCase())}`, when: r.win.left === 0 ? `0% left · ↻ ${fmtDuration(r.win.resetMs)}` : `≈ ${fmtAbsoluteReset(r.win.outAt, now)}`, p: r.win.left === 0 ? `Used up; resets in ${fmtDuration(r.win.resetMs)}.` : `Runs out ${fmtDuration(r.gapMs)} before its ${fmtAbsoluteReset(r.win.resetsAt, now)} reset at today's pace.`, href: `#account?name=${encodeURIComponent(r.acct.id)}` });
  for (const a of accts) if (a.needsLogin) items.push({ sev: 'neg', t: `${esc(a.name)} login`, when: 'Not read (—)', p: a.p.identity.needs_login ? 'The login expired, so its limits cannot be read.' : 'Not logged in yet.', btn: `<button class="btn-lane-start" data-act="login" data-name="${esc(a.id)}">Sign in</button>` });
  for (const [id, b] of Object.entries(d.dash.live.breakers || {})) if (b.state === 'open' && (!b.retry_at || b.retry_at > now)) items.push({ sev: 'neg', t: esc(id), when: b.retry_at ? 'retry ' + fmtDuration(b.retry_at - now) : 'paused', p: `Paused by the router after ${b.failures || 'repeated'} ${b.failures === 1 ? 'error' : 'errors'} in a row.`, href: '#routing' });
  const upd = (S.state && S.state.app_updates) || 0;
  if (upd) items.push({ sev: 'ac', t: `${upd} ${upd === 1 ? 'tool has' : 'tools have'} an update`, when: 'Apps', p: 'Apps installs the newer version.', href: '#apps' });
  return `<section class="panel-card" aria-label="Needs attention" data-testid="attention"><div class="panel-head"><h2 class="panel-title">Needs attention</h2><span class="att-count">${items.length ? items.length + ' open' : 'nothing'}</span></div>
    <div class="attention-list">${items.map((i) => `<div class="att-item" ${i.href ? `tabindex="0" role="link" data-href="${i.href}"` : ''}><div><div class="att-title"><i class="pc-dot ${i.sev}"></i><span>${i.t}</span></div><div class="att-reason">${i.p}</div>${i.btn || ''}</div><span class="att-when">${esc(i.when)}</span></div>`).join('') || '<div class="att-empty">Every limit lasts until its reset at this pace, every login is valid, no provider is paused.</div>'}</div></section>`;
}

// ---------- the Reset Horizon lanes ----------
const axisTicks = () => [0, 2, 4, 6, 7].map((d) => `<span class="axis-tick t${d} ${d === 0 ? 'now' : d === 7 ? 'end' : ''}" data-key="tick-${d}">${d === 0 ? 'NOW' : '+' + d + 'd'}</span>`).join('');
/** A per-model limit's short name: "Fable" of "Week · Fable". */
const modelName = (w) => w.label.split(' · ').pop();
/** Column 2: capacity left (or used), fixed-width bars never multiplied by time. */
function capacityColumn(a, used) {
  if (a.cap) {
    const leftUsd = a.cap.limitUsd - a.cap.spentUsd, left = Math.round((leftUsd / a.cap.limitUsd) * 100), pct = used ? 100 - left : left;
    return `<div class="lane-cap-col" title="Daily router spend cap: ${money(leftUsd)} left of ${money(a.cap.limitUsd)}"><div class="cap-mini-row"><span class="cap-mini-lbl">Cap</span><svg class="cap-mini-track" aria-hidden="true"><rect class="cap-mini-fill r2" width="${pct}%" height="100%" rx="2"/></svg><span class="cap-mini-pct">$${leftUsd.toFixed(0)}</span></div><div class="lane-cap-note">— no vendor window</div></div>`;
  }
  if (!a.measured) return `<div class="lane-cap-col"><div class="lane-cap-note">${a.noPlan ? 'No plan limits' : '— not read'}</div><div class="lane-cap-note ${a.needsLogin ? 'neg' : 'dim'}">${a.needsLogin ? 'Login expired' : a.noPlan ? 'Usage only' : 'Sign in to read'}</div></div>`;
  const row = (w, lbl) => { const v = used ? w.used : w.left, tone = w.left === 0 ? 'neg' : w.kind === '5h' ? (w.left <= 25 ? 'warn' : w.open && w.resetMs <= 2 * H && w.left >= 75 ? 'pos' : '') : (w.outMs != null || w.left <= 30 ? 'warn' : ''); return `<div class="cap-mini-row" title="${esc(w.label)}: ${w.left}% left (${w.used}% used)"><span class="cap-mini-lbl">${lbl}</span><svg class="cap-mini-track" aria-hidden="true"><rect class="cap-mini-fill ${tone}" width="${v}%" height="100%" rx="2"/></svg><span class="cap-mini-pct">${v}%</span></div>`; };
  const w5 = w5of(a), wk = wkof(a);
  // a model's own weekly limit (Claude's Fable) under the shared ones, named by the part after "Week · "
  const models = a.windows.filter((w) => w.kind === 'model' && w !== wk);
  return `<div class="lane-cap-col">${w5 ? row(w5, '5h') : ''}${wk && wk !== w5 ? row(wk, 'Wk') : ''}${models.map((w) => row(w, esc(modelName(w)))).join('')}</div>`;
}
/** Column 3: the time to each reset on a linear scale; the 5-hour window on its own 0–5 h zoom, the week on 0–7 d. */
function runwayTrack(a, now) {
  if (a.cap) return `<div class="lane-runway cap-runway"><span>Daily cap · ↻ ${fmtDuration(a.cap.resetMs)}</span><span>${esc(a.route || 'Local router')}</span></div>`;
  if (!a.measured) return `<div class="lane-runway unread-runway">${a.noPlan ? 'No plan windows' : '— Limits not read'}${a.needsLogin ? ' · Sign in' : ''}</div>`;
  const row = (w, short) => {
    const scale = short ? 5 * H : HORIZON, gate = Math.min(100, (w.resetMs / scale) * 100), out = w.outMs != null ? Math.min(100, (w.outMs / scale) * 100) : null;
    const harvest = short && w.resetMs <= 2 * H && w.left >= 75;
    return `<div class="runway-instrument ${short ? 'short-track' : ''}" data-key="rw-${w.id}" title="${esc(w.label)}: reset ${esc(fmtAbsoluteReset(w.resetsAt, now))}${out !== null ? '; ≈ out ' + esc(fmtAbsoluteReset(w.outAt, now)) : ''}"><div class="runway-caption"><span>${short ? '5h · 0–5h zoom' : 'Week · 0–7d'}${harvest ? ' · Proposed: harvest' : ''}</span><span>↻ ${fmtDuration(w.resetMs)}</span></div>
      <svg class="runway-track-row" aria-hidden="true">${out !== null ? `<line class="runway-forecast-dash" x1="0" x2="${out.toFixed(1)}%" y1="3" y2="3"/><rect class="runway-starve-gap" x="${out.toFixed(1)}%" width="${Math.max(0, gate - out).toFixed(1)}%" height="6"/>` : `<rect class="runway-bar-fill ${harvest ? 'pos' : ''}" width="${gate.toFixed(1)}%" y="1.5" height="3" rx="1.5"/>`}<rect class="reset-gate ${harvest ? 'harvest' : ''}" x="${gate.toFixed(1)}%" y="-1" width="2" height="8" transform="translate(-1 0)"/></svg></div>`;
  };
  const w5 = w5of(a), wk = wkof(a);
  return `<div class="lane-runway">${w5 && w5.open && st.windowMode !== 'week' ? row(w5, true) : ''}${wk && wk !== w5 && wk.open && st.windowMode !== '5h' ? row(wk, false) : ''}</div>`;
}
function lane(a, selected, used, pcs, now) {
  const w5 = w5of(a), wk = wkof(a);
  let top = a.noPlan ? 'No plan limits' : '— not read', sub = a.needsLogin ? 'Login expired' : a.noPlan ? 'Usage only' : 'Sign in or refresh', cls = a.needsLogin ? 'danger' : '';
  if (a.measured && wk) {
    const parts = [w5 && w5.open ? `5h ↻ ${fmtDuration(w5.resetMs)}` : '', wk.open && wk !== w5 ? `Wk ↻ ${fmtDuration(wk.resetMs)}` : ''].filter(Boolean);
    top = parts.map((x) => `<span class="fact">${x}</span>`).join(' · ');
    const spent = a.windows.find((w) => w.kind === 'model' && w.open && w.left === 0);
    if (w5 && w5.open && w5.left === 0) { sub = `Exhausted (5h ↻ ${fmtDuration(w5.resetMs)})`; cls = 'danger'; }
    else if (spent) { sub = `${modelName(spent)} used up (↻ ${fmtDuration(spent.resetMs)})`; cls = 'danger'; }
    else if (wk.outAt) { sub = `≈ Out ${fmtAbsoluteReset(wk.outAt, now)}`; cls = 'warn'; }
    else if (w5 && w5.open && w5.resetMs <= 2 * H && w5.left >= 75) { sub = `Use ${w5.left}% before ${fmtAbsoluteReset(w5.resetsAt, now)}`; cls = 'good'; }
    else if (wk.forecastAtReset != null) { sub = `≈ ${100 - wk.forecastAtReset}% left at reset`; cls = 'good'; }
    else { sub = 'Lasts until the reset'; cls = 'good'; }
  } else if (a.cap) { top = `Cap ↻ ${fmtDuration(a.cap.resetMs)}`; sub = `${money(a.cap.spentUsd)} / ${money(a.cap.limitUsd)} spent`; }
  return `<div class="t-lane ${selected ? 'selected' : ''} ${a.windows.some((w) => w.outAt) ? 'warn-lane' : ''}" tabindex="0" data-act="home-select" data-name="${esc(a.id)}" data-key="lane-${esc(a.id)}" data-testid="lane-${esc(a.id)}">
    <div class="lane-id">${renderRingSvg(a, used)}<div class="lane-name-box"><div class="lane-name-top">${toolIcon(a.tool, 14)}<span class="lane-acct-name">${esc(a.name)}</span><span class="pc-dots">${a.computers.map((id) => pcDot(pcs, id, 'Active on ' + ((pcs.find((c) => c.id === id) || {}).name || id))).join('')}</span></div><div class="lane-sub-meta"><span class="plan-pill">${esc(a.plan)}</span>${a.openSessions ? `<span class="lane-open"><i class="live-pulse-dot"></i>${a.openSessions} open</span>` : '<span>idle</span>'}</div></div></div>
    ${capacityColumn(a, used)}${runwayTrack(a, now)}
    <div class="lane-readout"><span class="lane-readout-primary">${top}</span><span class="lane-readout-sub ${cls}">${sub}</span></div>
    <div class="lane-action">${a.needsLogin ? `<button class="btn-lane-start neg" data-act="login" data-name="${esc(a.id)}">Sign in</button>` : `<span class="split-start"><button class="btn-lane-start" data-act="start" data-name="${esc(a.id)}" data-testid="lane-start-${esc(a.id)}">▶ Start</button><button class="btn-lane-start caret" data-act="start-menu" data-name="${esc(a.id)}" aria-haspopup="menu" aria-label="Start ${esc(a.name)} in a folder" title="Start in a folder" data-testid="lane-start-menu-${esc(a.id)}">▾</button></span>`}</div></div>`;
}

// ---------- the latest replies and the request log ----------
function repliesTable(rows, pcs, peers) {
  return `<table class="replies-table" data-testid="latest-replies"><thead><tr><th>Time</th><th>Account</th><th>Model</th>${peers ? '<th>Computer</th>' : ''}<th class="num">Tokens</th><th class="num" title="${TIMING}">First reply</th></tr></thead><tbody>
    ${rows.slice(0, 5).map((r) => `<tr><td>${timeCell(r.ts)} <span class="ago">(${ago(Date.now() - r.ts)})</span></td><td><span class="cell-inline">${toolIcon(r.harness, 14)}<b>${esc(acctName(r.profile))}</b></span></td><td>${esc(r.model)}</td>${peers ? `<td><span class="cell-inline">${r.device ? pcDot(pcs, r.device) + esc(nameOf(S.cache.computers, r.device)) : ''}</span></td>` : ''}<td class="num">${fmt(r.input + r.output + r.cache_read + r.cache_write)}</td><td class="num ${r.ttft_ms > 4000 ? 'warnT' : ''}" data-testid="ttft">${firstReply(r)}</td></tr>`).join('') || `<tr><td colspan="${peers ? 6 : 5}" class="muted">No replies in this range.</td></tr>`}</tbody></table>`;
}
function requestTable(rows, pcs) {
  return `<table class="replies-table" data-testid="request-log"><thead><tr><th>Time</th><th>Account</th><th>Model</th><th>Computer</th><th class="num">Input</th><th class="num">Output</th><th class="num">Cache read</th><th class="num">Cache write</th><th class="num" title="${TIMING}">Speed ≈</th><th class="num" title="${TIMING}">First reply</th><th class="num">Cost ≈</th></tr></thead><tbody>
    ${rows.map((r) => `<tr><td class="mono">${timeCell(r.ts)}</td><td><span class="cell-inline">${toolIcon(r.harness, 14)}<b>${esc(acctName(r.profile))}</b></span></td><td class="mono">${esc(r.model)}</td><td>${r.device ? esc(nameOf(S.cache.computers, r.device)) : esc((S.state && S.state.computer) || '')}</td><td class="num">${fmt(r.input)}</td><td class="num">${fmt(r.output)}</td><td class="num">${fmt(r.cache_read)}</td><td class="num">${fmt(r.cache_write)}</td><td class="num" data-testid="speed">${speed(r)}</td><td class="num" data-testid="ttft">${firstReply(r)}</td><td class="num"><b>${money(r.cost, 4)}</b></td></tr>`).join('') || '<tr><td colspan="11" class="muted">No replies in this range.</td></tr>'}</tbody></table>`;
}
function openRequestLog() {
  const n = (S.state && S.state.profiles.length) || 0;
  openDialog(`${dlgHead('Request log')}<span class="panel-note">${st.total.toLocaleString()} ${st.total === 1 ? 'request' : 'requests'} ${RANGE_LABEL[st.range].toLowerCase()} · costs are what these tokens would cost at API list prices (models.dev); subscriptions are not billed per token · speed and first reply are approximate</span>
    <div class="table-wrap" id="home-table">${requestTable(st.rows)}</div>${st.rows.length < st.total ? '<div class="more-row"><button class="btn-secondary" data-act="home-more" data-testid="home-more">Show 50 more</button></div>' : ''}`, { size: 'xl', label: 'Request log' });
}

// ---------- the first-run and empty states ----------
function firstRun(state, accts, tools) {
  if (!accts.length) {
    return `<div class="page-body"><section class="panel-card empty-state center"><div class="empty-logo" id="empty-mark"></div><h1>${tools.some((t) => t.installed) ? 'No account yet' : 'No AI coding tool found'} on ${esc(state.computer || 'this computer')}</h1>
      <div class="tool-pick-grid full">${tools.map((t) => `<div class="tool-pick-card"><div class="tool-pick-head"><span class="tool-pick-name">${toolIcon(t.id, 14)} ${esc(t.label)}</span><span class="plan-pill">${t.installed ? 'Installed' : 'Not installed'}</span></div><div class="tool-pick-sup">Supports: ${['Subscription', t.caps.api_key ? 'API key' : '', t.caps.routable ? 'Routed' : ''].filter(Boolean).join(' · ')}</div><button class="btn-secondary" data-act="add-account-for" data-h="${t.id}">Prepare ${esc(t.label)} account</button></div>`).join('')}</div>
      <div class="row-actions"><button class="btn-primary" data-act="add-account" data-testid="add-account">+ Add first account</button></div></section></div>`;
  }
  return `<div class="page-body"><section class="panel-card first-run-card"><div class="eyebrow-row"><span class="eyebrow ac">Profiles on ${esc(state.computer || 'this computer')} · ${accts.length} ${accts.length === 1 ? 'profile' : 'profiles'} ready</span><span class="eyebrow-note">Limits not read (—) until first sign-in or refresh</span></div>
    <div class="first-run-grid"><div class="first-run-col"><h2>Limits have not been read yet.</h2><div class="detected-profiles">${accts.map((a) => `<div class="live-mini-row" data-key="fr-${esc(a.id)}"><div class="live-mini-left">${toolIcon(a.tool, 14)}<b>${esc(a.name)}</b><span class="plan-pill">${a.isMain ? 'main profile' : esc(a.plan)}</span><span class="mono">${esc(a.folder)}</span><span class="note">· ${a.noPlan ? 'Usage only · no plan limits' : '— limits not read'}</span></div>${a.needsLogin ? `<button class="btn-lane-start" data-act="login" data-name="${esc(a.id)}">Log in</button>` : `<button class="btn-lane-start" data-act="start" data-name="${esc(a.id)}">▶ Start</button>`}</div>`).join('')}</div>
      <div class="row-actions"><button class="btn-primary" data-act="add-account" data-testid="add-account">+ Add another account</button><button class="btn-secondary" data-act="refresh-now">Read limits now</button></div></div>
      <div class="panel-card sub"><h2 class="panel-title">Sync limits and live sessions across your computers?</h2><div class="sync-choices">
        <button class="candidate-pill" data-act="firstrun-later" data-testid="firstrun-later"><div><b>Not now — this computer only</b><div class="cand-sub">Turn on multi-computer sync anytime in Settings › Computers</div></div><span>Local</span></button>
        <button class="candidate-pill" data-act="firstrun-folder" data-testid="firstrun-folder"><div><b>Shared folder (OneDrive, Dropbox, iCloud, SMB)</b><div class="cand-sub">Syncs limits and sessions across your computers</div></div><span class="acT">Folder</span></button>
        <button class="candidate-pill" data-act="firstrun-hub" data-testid="firstrun-hub"><div><b>A Harness Hub you host</b><div class="cand-sub">A real-time relay over Tailscale or your LAN</div></div><span>wss://</span></button></div></div></div></section></div>`;
}
action('firstrun-later', () => { try { localStorage.setItem('hm.firstrun', '1'); } catch {} render(); });

// ---------- the page ----------
view('home', async (sheet) => {
  sheet.className = 'sheet';
  const wk = 'home:' + JSON.stringify([st.range, st.harness, st.device, scope()]);
  warmPage(sheet, wk);
  const hours = RANGES.find((r) => r[0] === st.range)[1], days = Math.round(hours / 24);
  // scope: every computer added up, or this one only; the sync core answers the same shape either way
  const [state, d, pcs] = await Promise.all([loadState(), call('home', { days, hours, harness: st.harness, limit: 50, scope: scope() }), computersStatus().catch(() => null)]);
  st.rows = d.requests.rows; st.total = d.requests.total;
  const o = d.overview, k = d.dash, now = k.now, used = showUsed();
  if (!st.harness) setTodayBadge(o.today_cost);
  if (!S.appVersions) { S.appVersions = {}; call('apps').then((a) => { S.appVersions = Object.fromEntries(a.apps.filter((x) => x.version).map((x) => [x.id, x.version])); }).catch(() => {}); }
  const tools = shownTools(state), accts = accountViews(state, k, now, pcs), pcl = pcList(state, pcs), me = pcl[0].id;
  const sessions = runningSessions(state, d, pcs, me);
  const live = sessions.filter((s) => s.computer === me).length, elsewhere = sessions.length - live;
  const isFresh = !accts.some((a) => a.measured) && !d.requests.total && !sessions.length;
  if (isFresh && (!accts.length || !localStorage.getItem('hm.firstrun'))) { patch(sheet, firstRun(state, accts, tools)); const m = sheet.querySelector('#empty-mark'); if (m) m.innerHTML = document.getElementById('brand-mark').innerHTML; keepPage(sheet, wk); return; }
  const m = metricsOf(accts), best = m.bestRoomAcct, exp = m.expiringAcct || best, pick = st.preferExpiring && exp ? exp : best;
  const risk = m.primaryRisk;
  const selected = accts.find((a) => a.id === st.selected) || (risk ? risk.acct : accts[0]);
  const filtered = accts.filter((a) => (!st.harness || a.tool === st.harness) && (scope() !== 'here' || !a.openSessions || a.computers.includes(me)));
  const shownSessions = sessions.filter((s) => st.livePc === 'all' || s.computer === st.livePc).sort((a, b) => (b.streaming - a.streaming) || ((a.since || 0) - (b.since || 0)));
  const yc = yearClock(o.heatmap, pcs, pcl.some((c) => c.id === st.device) ? st.device : '');
  const byDev = pcl.map((c) => ({ ...c, tokens: o.heatmap.reduce((s, dd) => s + ((dd.by_device || {})[c.id] || 0), 0) }));
  const peers = syncOn(pcs) && (pcs.peers || []).length > 0;
  const ttft = (id) => { const t = st.rows.filter((r) => r.profile === id && r.ttft_ms != null).map((r) => r.ttft_ms).sort((a, b) => a - b); return t.length ? `≈ ${(t[Math.floor(t.length / 2)] / 1000).toFixed(1)}s` : '—'; };
  const pick5 = pick && w5of(pick), best5 = best && w5of(best), bestWk = best && wkof(best), exp5 = exp && w5of(exp);
  const fmtAt = (at) => fmtAbsoluteReset(at, now);
  patch(sheet, `<div class="page-body">
    <section class="dispatch-deck" aria-label="Primary operational summary">
      <div class="deck-card hero-lead" data-key="hero" data-testid="use-next"><div class="eyebrow-row"><span class="eyebrow ac">Use next · Reset runway</span><button class="proposed-tag" data-act="home-prefer" aria-pressed="${st.preferExpiring}" title="Proposed: let Start pick an account whose 5-hour window resets within 2 hours and still has more than half its room">Proposed: ${st.preferExpiring ? 'Expiring 5h first ✓' : 'Prefer expiring 5h'}</button></div>
        <div class="lead-main"><div class="lead-text">${pick ? `<div class="lead-headline">Use <span class="lead-name">${esc(pick.name)}</span></div><div class="lead-sub">${toolIcon(pick.tool, 14)} <b>${esc(TOOL[pick.tool] || pick.tool)}</b> · <span class="fact">${esc(pick.plan)}</span>${pick5 && pick5.open ? ` · <span class="fact">5h ↻ ${fmtDuration(pick5.resetMs)} (${fmtAt(pick5.resetsAt)})</span>` : ''} · <span class="fact">read ${pick.readAt ? ago(now - pick.readAt) : 'never'}</span></div>` : `<div class="lead-headline">No measured room to recommend.</div><div class="lead-sub">${accts.length} ${accts.length === 1 ? 'account' : 'accounts'} · ${accts.some((a) => a.needsLogin) ? 'sign in, then ' : ''}refresh to read limits (—)</div>`}</div>
          <div class="lead-actions">${pick ? `<button class="cta-start-lg" data-act="start" data-name="${esc(pick.id)}" data-testid="start-best">▶ Start</button><button class="btn-secondary compact" data-act="start-in-folder" data-name="${esc(pick.id)}" title="Start in a specific folder with flags">Folder…</button>` : '<button class="btn-secondary compact" data-act="refresh-now">Read limits</button>'}</div></div>
        ${best ? `<div class="lead-candidates"><button class="candidate-pill ${!st.preferExpiring ? 'active' : ''}" data-act="home-mode" data-v="room" aria-pressed="${!st.preferExpiring}"><div><b>${esc(best.name)}</b><div class="cand-sub">Most room: ${best5 ? best5.left + '% 5h · ' : ''}${bestWk ? bestWk.left + '% wk' : ''}</div></div><span class="cand-reset">↻ ${best5 && best5.open ? fmtDuration(best5.resetMs) : bestWk && bestWk.open ? fmtDuration(bestWk.resetMs) : '—'}</span></button>
          <button class="candidate-pill ${st.preferExpiring ? 'active' : ''}" data-act="home-mode" data-v="expiring" aria-pressed="${st.preferExpiring}"><div><b>${esc(exp.name)}</b><div class="cand-sub">Resets soonest: ${exp5 ? exp5.left + '% 5h left' : 'no 5-hour window'}</div></div><span class="cand-reset pos">↻ ${exp5 && exp5.open ? fmtDuration(exp5.resetMs) : '—'}</span></button></div>` : ''}</div>
      <div class="deck-card exhaust-card" data-key="exhaust"><div class="eyebrow-row"><span class="eyebrow warn">Runs out first</span><span class="eyebrow-note">Measured + ≈ Pace</span></div>
        ${risk ? `<div><div class="exhaust-Finding" data-testid="finding"><span class="warn-name">${esc(risk.acct.name)}</span> ${risk.win.left === 0 ? `is exhausted now (0% ${esc(risk.win.label.toLowerCase())} left, resets in ${fmtDuration(risk.win.resetMs)}).` : `will likely run out of its ${esc(risk.win.label.toLowerCase())} ≈ ${fmtAt(risk.win.outAt)}, <span class="gap">${fmtDuration(risk.gapMs)}</span> before reset.`}</div>
          <div class="exhaust-basis" data-testid="basis">Measured <b>${risk.win.used}% used</b> (${risk.win.left}% left), resets <b>${fmtAt(risk.win.resetsAt)}</b>${risk.acct.readAt ? ', read ' + ago(now - risk.acct.readAt) : ''} · pace forecast ≈.</div></div>` : `<div class="exhaust-Finding" data-testid="finding">No account is on pace to run out before its reset.</div><div class="exhaust-basis" data-testid="basis">${accts.filter((a) => a.measured).length} of ${accts.length} ${accts.length === 1 ? 'account' : 'accounts'} measured · ${live} ${live === 1 ? 'console' : 'consoles'} open here · router ${state.router.running ? ':' + state.router.port + ' running' : 'stopped'}</div>`}
        <div class="deficit-chips">${m.atRisk.slice(0, 3).map((r) => `<button class="mini-chip ${selected && selected.id === r.acct.id ? 'active' : ''}" data-act="home-select" data-name="${esc(r.acct.id)}" data-key="risk-${esc(r.acct.id)}-${esc(r.win.id)}">${toolIcon(r.acct.tool, 14)} <b>${esc(r.acct.name)} · ${esc(r.win.label)}</b> <span class="chip-warn">${r.win.left === 0 ? '0% left now' : `${r.win.left}% left · ≈ ${fmtAt(r.win.outAt)}`}</span></button>`).join('')}${accts.filter((a) => a.needsLogin).map((a) => `<button class="mini-chip danger ${selected && selected.id === a.id ? 'active' : ''}" data-act="home-select" data-name="${esc(a.id)}" data-key="login-${esc(a.id)}">${toolIcon(a.tool, 14)} <b>${esc(a.name)}</b> <span>— login expired</span></button>`).join('')}</div></div>
      <div class="deck-card" data-key="running" data-testid="running-now"><div class="eyebrow-row"><span class="eyebrow secondary">Running now</span><span class="eyebrow-note">${live} here · ${elsewhere} elsewhere</span></div>
        <div class="chip-group" role="group" aria-label="Filter running sessions by computer"><button class="filter-chip sm ${st.livePc === 'all' ? 'active' : ''}" data-act="home-live-pc" data-v="all">All (${sessions.length})</button>${pcl.map((c) => `<button class="filter-chip sm ${st.livePc === c.id ? 'active' : ''}" data-act="home-live-pc" data-v="${esc(c.id)}"><i class="pc-dot c${c.ci}"></i>${esc(c.name)} ${sessions.filter((s) => s.computer === c.id).length}</button>`).join('')}</div>
        <div class="live-mini-list" tabindex="0" aria-label="All running sessions; scroll for more">${shownSessions.map((s) => `<div class="live-mini-row" data-key="live-${esc(s.computer)}-${esc(s.id || s.pid)}"><div class="live-mini-left">${pcDot(pcl, s.computer)}${toolIcon(s.tool, 14)}<b>${esc(s.project)}</b><span class="live-mini-meta">${esc(nameOf(pcs, s.computer) || pcl[0].name)}</span><span class="live-account">${esc(acctName(s.accountId))} · ${s.streaming ? 'streaming' : 'open'}${s.since ? ' · ' + fmtDuration(now - s.since) : ''}</span></div>${s.id ? `<button class="btn-follow" data-act="open-session" data-profile="${esc(s.accountId)}" data-id="${esc(s.id)}" ${s.computer !== me ? `data-device="${esc(s.computer)}"` : ''}>Follow</button>` : `<button class="btn-follow" data-act="show-window" data-pid="${s.pid}">Show</button>`}</div>`).join('') || '<div class="att-empty">No session is running.</div>'}</div></div>
    </section>
    <section class="horizon-panel" aria-label="Reset Horizon timeline" data-testid="horizon"><div class="horizon-header"><div class="horizon-title-group"><div class="horizon-title"><span>Reset Horizon</span></div><div class="honesty-legend"><span class="legend-item"><i class="legend-swatch solid"></i>Time until reset</span><span class="legend-item"><i class="legend-swatch dashed"></i>≈ Pace forecast</span><span class="legend-item"><i class="legend-swatch starve"></i>Runs out before reset</span><span class="legend-item"><b>—</b> Not read</span></div></div>
        <div class="horizon-controls"><div class="chip-group" role="group" aria-label="Window filter">${[['both', '5h + Week'], ['5h', '5-hour only'], ['week', 'Weekly only']].map(([v, l]) => `<button class="filter-chip ${st.windowMode === v ? 'active' : ''}" aria-pressed="${st.windowMode === v}" data-act="home-window" data-v="${v}">${l}</button>`).join('')}</div><button class="filter-chip active" data-act="home-ring-mode" title="Switch limit rings and capacity bars between % left and % used">Rings: ${used ? '% used' : '% left'}</button></div></div>
      <div class="horizon-filterbar" data-testid="home-filters"><div class="chip-group" role="group" aria-label="Filter by tool"><button class="filter-chip ${!st.harness ? 'active' : ''}" data-act="home-tool" data-v="" data-testid="tool-all">All (${accts.length})</button>${tools.map((h) => `<button class="filter-chip ${st.harness === h.id ? 'active' : ''}" data-act="home-tool" data-v="${h.id}" data-testid="tool-${h.id}">${toolIcon(h.id, 14)}<span>${esc(h.label)}</span><span class="count">${accts.filter((a) => a.tool === h.id).length}</span></button>`).join('')}</div>
        ${peers ? `<div class="chip-group" role="group" aria-label="Computers" data-testid="scope"><button class="filter-chip ${scope() === 'all' ? 'active' : ''}" data-act="scope" data-v="all" data-testid="scope-all">All computers (${pcl.length})</button><button class="filter-chip ${scope() === 'here' ? 'active' : ''}" data-act="scope" data-v="here" data-testid="scope-here"><i class="pc-dot c0"></i>This computer (${esc(pcl[0].name)})</button></div>` : ''}</div>
      <div class="timeline-grid-head"><div>ACCOUNT &amp; WEEK RING (${used ? 'USED' : 'LEFT'})</div><div>CAPACITY (% ${used ? 'USED' : 'LEFT'})</div><div class="axis-ruler">${axisTicks()}</div><div class="col-readout">NEXT RESET &amp; PACE</div><div class="col-action">ACTION</div></div>
      <div class="timeline-lanes">${filtered.map((a) => lane(a, selected && a.id === selected.id, used, pcl, now)).join('') || '<div class="group-empty">No account of that tool.</div>'}</div>
      ${selected ? `<div class="route-inspector-bar"><div class="route-chain"><span class="route-label">Selected route:</span><span class="route-node"><span class="pc-dots">${(selected.computers.length ? selected.computers : [me]).map((id) => `${pcDot(pcl, id)}<span>${esc(nameOf(pcs, id) || pcl[0].name)}</span>`).join(' + ')}</span></span><span class="route-arrow">→</span><span class="route-node">${toolIcon(selected.tool, 14)}<b>${esc(selected.name)}</b><span class="route-muted">(${selected.measured ? `${wkof(selected).used}% ${esc(wkof(selected).label.toLowerCase())} used` : '— not read'})</span></span><span class="route-arrow">→</span><span class="route-node"><b>${esc(TOOL[selected.tool] || selected.tool)}</b>${(S.appVersions || {})[selected.tool] ? `<span class="route-muted">v${esc(S.appVersions[selected.tool])}</span>` : ''}<span class="route-live">· first reply ${ttft(selected.id)} · ${selected.openSessions} open</span></span></div>
        <div class="route-right"><span class="esc-hint"><kbd class="kbd">Esc</kbd> resets selection</span><a class="filter-chip active" href="#account?name=${encodeURIComponent(selected.id)}">Inspect ${esc(selected.name)} ↗</a></div></div>` : ''}</section>
    <section class="lower-grid" aria-label="Readouts, attention items and the 53-week activity">${readouts(k, pcl.length)}${attention(m, accts, d, now)}
      <section class="panel-card" data-testid="activity"><div class="panel-head"><h2 class="panel-title">Activity, 53 weeks</h2>${peers ? `<div class="chip-group" role="group" aria-label="Year clock computer" data-testid="activity-pc"><button class="filter-chip sm ${!st.device ? 'active' : ''}" data-act="home-device" data-v="" data-testid="activity-pc-all">All</button>${pcl.map((c) => `<button class="filter-chip sm ${st.device === c.id ? 'active' : ''}" data-act="home-device" data-v="${esc(c.id)}" title="${esc(c.name)}" data-testid="activity-pc-${esc(c.id)}"><i class="pc-dot c${c.ci}"></i><span class="pc-name">${esc(c.name)}</span></button>`).join('')}</div>` : ''}</div>
        <div class="year-clock-wrap">${yc.svg}<div class="year-clock-facts"><div><b class="ink">Monday inside</b> · daily tokens</div><div class="muted">Busiest: <b class="ink">${yc.bv ? new Date(yc.best.day + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short' }) + ' (' + fmt(yc.bv) + ')' : '—'}</b></div><div class="muted">53-wk total: <b class="ac">${fmt(yc.total)}</b></div>${peers ? `<div class="year-clock-pcs">${byDev.map((c) => `<span><i class="pc-dot c${c.ci} mr"></i>${esc(c.name)}: ${fmt(c.tokens)} (53w)</span>`).join('')}</div>` : ''}</div></div></section></section>
    <section class="panel-card" aria-label="Latest replies" data-key="replies"><div class="panel-head"><div class="head-group"><h2 class="panel-title">Latest replies</h2><span class="panel-note">${st.total.toLocaleString()} ${RANGE_LABEL[st.range].toLowerCase()} · first reply is approximate (≈)</span></div><button class="filter-chip link" data-act="home-log" data-testid="request-log-open">Request log (full table) ↗</button></div>${repliesTable(st.rows, pcl, peers)}</section>
  </div>`);
  keepPage(sheet, wk);
});

action('home-range', (el) => { st.range = el.dataset.v; render(); });
action('home-tool', (el) => { st.harness = el.dataset.v; render(); });
action('home-device', (el) => { st.device = el.dataset.v; render(); });
action('home-live-pc', (el) => { st.livePc = el.dataset.v; render(); });
action('home-window', (el) => { st.windowMode = el.dataset.v; render(); });
action('home-prefer', () => { st.preferExpiring = !st.preferExpiring; render(); });
action('home-mode', (el) => { st.preferExpiring = el.dataset.v === 'expiring'; render(); });
action('home-select', (el) => { st.selected = el.dataset.name; render(); });
action('home-ring-mode', () => mutate(() => call('settings_set', { key: 'quota_display', value: showUsed() ? 'left' : 'used' })));
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
action('home-sync', async () => {
  const r = await attempt(() => call('refresh', { live: S.state && S.state.live_quota }));
  if (r) { S.lastSync = Date.now(); toast(refreshText(r)); changed(); }
});
// Escape with nothing open returns the selection to the finding's account
document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && S.route.view === 'home' && st.selected && !dialogOpen() && !document.querySelector('.menu')) { st.selected = null; render(); } });
// cards that open another page
document.addEventListener('click', (e) => { const a = e.target.closest('[data-href]'); if (a && !e.target.closest('button, a')) location.hash = a.dataset.href; });
