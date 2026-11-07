// Accounts: the finding, the daily chart, one card per tool with a row per account; Account detail with its four tabs;
// Add account with sign-in; Start in a folder; the account menu; the tray popup. The account adapter, the one
// eligibility rule and the ring renderer live here and feed the Overview and the popup too.

import { S, call, view, go, toast, attempt, mutate, changed, acctName, refreshText, shownTools, openDialog, closeDialog, dlgHead, openMenu, action, loadState, render, pickFolder, saveFile, patch, warmPage, keepPage } from './core.js';
import { esc, fmt, hm, when, dur, toolIcon } from './lib.js';
import { tagsHtml } from './sessions.js';
import { syncOn, computers, nameOf, cdot } from './computers.js';
import { resumeData, reasonShort } from './resume.js';

export const TOOL = { claude: 'Claude Code', codex: 'Codex', opencode: 'OpenCode', agy: 'Antigravity', pi: 'Pi', omp: 'Oh My Pi' };
/** The tools whose plans have windows to read; the others show usage only. */
export const PLAN_TOOLS = ['claude', 'codex'];
export const money = (c, d = 2) => (c == null ? '—' : '$' + c.toLocaleString(undefined, { minimumFractionDigits: d, maximumFractionDigits: d }));
/** "4 h 08 m", "2 d 14 h": the reference's duration. */
export function fmtDuration(ms) {
  if (ms == null || isNaN(ms)) return '—';
  const m = Math.max(1, Math.round(ms / 60000));
  if (m < 60) return `${m} m`;
  const h = Math.floor(m / 60), rm = m % 60;
  if (h < 24) return rm ? `${h} h ${String(rm).padStart(2, '0')} m` : `${h} h`;
  const d = Math.floor(h / 24), rh = h % 24;
  return rh ? `${d} d ${rh} h` : `${d} d`;
}
/** "Today 19:50" within 18 hours, else "Sun 00:56". */
export function fmtAbsoluteReset(at, now = Date.now()) {
  if (at == null) return '—';
  const t = new Date(at), hhmm = `${String(t.getHours()).padStart(2, '0')}:${String(t.getMinutes()).padStart(2, '0')}`;
  return at - now < 18 * 3600e3 && at >= now ? `Today ${hhmm}` : `${t.toLocaleDateString('en-US', { weekday: 'short' })} ${hhmm}`;
}
const agoText = (t, now) => (t == null ? 'never' : now - t < 60000 ? 'just now' : dur(now - t) + ' ago');
/** The row's "read … ago" in short units (11 h ago), on its own line under the buttons, so they keep their size. */
const readAgo = (t, now) => { const d = now - t; return d < 60000 ? 'just now' : (d < 3600e3 ? `${Math.floor(d / 60000)} m` : d < 86400e3 ? `${Math.floor(d / 3600e3)} h` : `${Math.floor(d / 86400e3)} d`) + ' ago'; };
const tier = (p, id) => p.quota && p.quota.tiers.find((t) => t.id === id);
const weekTier = (p) => tier(p, 'week') || (p.quota && p.quota.tiers.find((t) => t.id.startsWith('week')));
const emailOf = (p) => p.identity.email || p.expected_email || '';
const planOf = (p) => (p.kind === 'api_key' ? ['API key', 'warn'] : p.kind === 'routed' ? [`Routed · ${p.route || ''}`, 'r2'] : !p.identity.logged_in ? ['Not logged in', 'neg'] : p.identity.needs_login ? ['Login expired', 'neg'] : [p.identity.plan || 'Logged in', 'ac']);
/** The setting: limits as what is left (the default) or what is used. */
export const showUsed = () => !!(S.state && S.state.settings && S.state.settings.quota_display === 'used');

/** An account as every page draws it: the profile with its windows (left, used, reset, the forecast), its cap, and
 * where it runs; x is limits_extra (forecast, caps), pcs the computers (for sessions elsewhere). */
export function acctView(p, x = {}, now = Date.now(), pcs = null) {
  const tiers = (p.quota && p.quota.tiers) || [];
  const needsLogin = p.kind === 'subscription' && (p.identity.needs_login || !p.identity.logged_in);
  const windows = tiers.map((t) => {
    const f = x.forecast && x.forecast[p.name + '|' + t.id];
    const used = Math.min(100, Math.round(t.used)), left = Math.max(0, 100 - used), open = !!(t.resets_at && t.resets_at > now);
    const out = f && f.runs_out && open && f.runs_out < t.resets_at ? f.runs_out : null;
    return { id: t.id, label: t.label, kind: t.id === 'session' ? '5h' : t.id === 'week' ? 'week' : 'model', used, left, open, resetsAt: t.resets_at || null, resetMs: open ? t.resets_at - now : 0, outAt: out, outMs: out ? out - now : null, forecastAtReset: f ? Math.min(100, Math.round(f.at_reset)) : null };
  });
  const c = x.caps && x.caps[p.name], mid = new Date(now); mid.setHours(24, 0, 0, 0);
  const me = pcs && pcs.this ? pcs.this.id : 'this';
  const far = pcs && syncOn(pcs) ? (pcs.peers || []).flatMap((d) => (d.running || []).filter((r) => r.profile === p.name).map((r) => ({ ...r, device: d.id }))) : [];
  const on = [...new Set([...(p.running.length ? [me] : []), ...far.map((r) => r.device)])];
  const [plan, planTone] = planOf(p);
  return { p, id: p.name, name: p.label, tool: p.harness, kind: p.kind, plan, planTone, email: emailOf(p), folder: p.home, isMain: !!p.main, needsLogin, measured: windows.length > 0,
    added: p.created_at ? new Date(p.created_at).toLocaleDateString([], { day: 'numeric', month: 'short' }) : '', readAt: (p.quota && p.quota.fetched_at) || null, windows, far,
    cap: c ? { limitUsd: c.cap, spentUsd: c.spent, resetMs: mid - now } : null, route: p.route, openSessions: p.running.length + far.length, computers: on,
    noPlan: !PLAN_TOOLS.includes(p.harness) || p.kind !== 'subscription', mismatch: !!p.email_mismatch };
}
/** Every shown account, as acctView draws it. */
export const accountViews = (st, x, now, pcs) => { const ids = shownTools(st).map((h) => h.id); return st.profiles.filter((p) => ids.includes(p.harness)).map((p) => acctView(p, x, now, pcs)); };
/** The one rule for a recommendation: a subscription, signed in, with measured windows none of which is used up. */
export const eligible = (a) => a.kind === 'subscription' && !a.needsLogin && a.measured && a.windows.every((w) => !w.open || w.left > 0);
export const w5of = (a) => a.windows.find((w) => w.kind === '5h');
export const wkof = (a) => a.windows.find((w) => w.kind === 'week') || a.windows[0];
/** Most room, resets soonest, and every window running out: the Overview's deck, the Accounts finding and the tray share these. */
export function metricsOf(accts) {
  const measured = accts.filter((a) => a.measured);
  const byRoom = measured.filter(eligible).sort((a, b) => (wkof(b).left - wkof(a).left) || ((w5of(b) || {}).left || 0) - ((w5of(a) || {}).left || 0));
  const bestRoomAcct = byRoom[0] || null;
  const expiringAcct = byRoom.filter((a) => w5of(a) && w5of(a).open && w5of(a).left >= 50).sort((a, b) => w5of(a).resetMs - w5of(b).resetMs)[0] || bestRoomAcct;
  const atRisk = [];
  for (const a of measured) for (const w of a.windows) if (w.open && (w.left === 0 || w.outMs != null)) atRisk.push({ acct: a, win: w, outMs: w.left === 0 ? 0 : w.outMs, gapMs: w.resetMs - (w.left === 0 ? 0 : w.outMs) });
  atRisk.sort((x, y) => x.outMs - y.outMs);
  return { bestRoomAcct, expiringAcct, atRisk, primaryRisk: atRisk[0] || null };
}

/** The week ring (36 px, the label an SVG text that fits inside): what is left (or used, by the setting); ∞ for an API key, a dotted — when not read, red dashes for a login to renew. */
const R = 15.5, C = 2 * Math.PI * R;
const ring = (title, circles, label) => `<div class="ring-wrap" title="${title}"><svg viewBox="0 0 36 36" aria-hidden="true"><g transform="rotate(-90 18 18)">${circles}</g><text class="ring-val ${label.length > 3 ? 'long' : ''}" x="18" y="18" text-anchor="middle" dominant-baseline="central">${label}</text></svg></div>`;
const arc = (pct, cls) => `<circle class="track" cx="18" cy="18" r="${R}" fill="none" stroke-width="3"/><circle class="fill ${cls}" cx="18" cy="18" r="${R}" fill="none" stroke-width="3" stroke-linecap="round" stroke-dasharray="${((pct / 100) * C).toFixed(1)} ${C.toFixed(1)}"/>`;
export function renderRingSvg(a, used = showUsed()) {
  if (a.kind === 'api_key') return ring('API key · no plan limit', '', '∞');
  if (a.cap) {
    const left = Math.max(0, Math.round(100 - (a.cap.spentUsd / a.cap.limitUsd) * 100)), pct = used ? 100 - left : left;
    return ring(`${esc(a.name)}: daily cap ${money(a.cap.limitUsd - a.cap.spentUsd)} left of ${money(a.cap.limitUsd)}`, arc(pct, 'r2'), `${pct}%`);
  }
  if (a.needsLogin || !a.measured) return ring(a.needsLogin ? 'Sign in to read its limits' : a.noPlan ? 'No plan limits' : 'Limits not read (—)', `<circle class="unread ${a.needsLogin ? 'neg' : ''}" cx="18" cy="18" r="${R}" fill="none" stroke-width="2.8" stroke-dasharray="3.4 3.4"/>`, '—');
  const wk = wkof(a), pct = used ? wk.used : wk.left;
  const tone = wk.left === 0 ? 'neg' : wk.left <= 30 || wk.outMs != null ? 'warn' : '';
  return ring(`${esc(a.name)}: ${esc(wk.label.toLowerCase())} ${wk.left}% left (${wk.used}% used)${wk.open ? ', resets ' + esc(fmtAbsoluteReset(wk.resetsAt)) : ''}`, arc(pct, tone), `${pct}%`);
}
/** A room bar: the track with what is left (or used) filled, as an SVG whose width is an attribute. */
export const winTrack = (pct, tone = '', forecastAt = null) => `<svg class="win-track" aria-hidden="true"><rect class="win-fill ${tone}" width="${Math.max(0, Math.min(100, pct))}%" height="100%" rx="3"/>${forecastAt != null ? `<line class="limit-forecast" x1="${forecastAt}%" x2="${forecastAt}%" y1="-3" y2="9"/>` : ''}</svg>`;

// ---------- the tray popup: every account's ring, in the window beside the rail and in the tray's own window ----------
export function trayHtml(accts, inWindow) {
  const { bestRoomAcct: best } = metricsOf(accts), b5 = best && w5of(best);
  const line = (a) => { const wk = a.measured && wkof(a); return wk && wk.open ? `↻ ${fmtDuration(wk.resetMs)}` : a.cap ? `Daily cap · ${money(a.cap.limitUsd - a.cap.spentUsd)} left` : a.kind === 'api_key' ? 'Pay per use' : a.needsLogin ? '— sign in to read' : a.noPlan ? 'No plan limits' : '— unread'; };
  return `<aside class="tray-popup" role="dialog" aria-label="Limits of every account" tabindex="-1">
    <div class="tray-head"><span class="tray-title"><span class="brand-logo" id="tray-mark"></span>Limits · ${accts.length} ${accts.length === 1 ? 'account' : 'accounts'}</span>${inWindow ? '<button class="icon-btn xs" data-act="toggle-tray" aria-label="Close the limits popup">✕</button>' : ''}</div>
    <div class="tray-next">${best ? `Use next: <b>${esc(best.name)}</b>${b5 && b5.open ? ` (${b5.left}% 5h · ↻ ${fmtDuration(b5.resetMs)})` : ''}` : accts.length ? '— Limits not read; no recommendation' : 'No accounts configured'}</div>
    ${accts.length ? `<div class="tray-rings-grid" data-testid="nav-limits">${accts.map((a) => `<button class="tray-ring-card" data-act="tray-open" data-name="${esc(a.id)}" data-key="${esc(a.id)}" title="Inspect ${esc(a.name)}" data-testid="nav-limit-${esc(a.id)}">${renderRingSvg(a)}<span class="who"><span class="name">${toolIcon(a.tool, 14)}<b>${esc(a.name)}</b></span><span class="sub">${esc(line(a))}</span></span></button>`).join('')}</div>` : '<div class="tray-empty">No accounts configured. Add one on Accounts.</div>'}
  </aside>`;
}
/** Draw the popup window's content (limits.html) from the state read; the mark is drawn by the caller. */
export async function drawTray(root) {
  const x = await call('limits_extra').catch(() => ({}));
  root.innerHTML = trayHtml(accountViews(S.state, x, Date.now(), S.cache.computers), false);
}
action('toggle-tray', async (el) => {
  const root = document.getElementById('tray-root'), trigger = document.querySelector('[data-act=toggle-tray].tray-trigger');
  if (root.innerHTML) { root.innerHTML = ''; trigger.setAttribute('aria-expanded', 'false'); trigger.focus({ preventScroll: true }); return; }
  const x = await call('limits_extra').catch(() => ({}));
  root.innerHTML = trayHtml(accountViews(S.state, x, Date.now(), S.cache.computers), true);
  document.dispatchEvent(new CustomEvent('hm:tray'));
  trigger.setAttribute('aria-expanded', 'true');
  (root.querySelector('.tray-ring-card') || root.querySelector('.tray-popup')).focus({ preventScroll: true });
});
action('tray-open', (el) => { document.getElementById('tray-root').innerHTML = ''; document.querySelector('.tray-trigger')?.setAttribute('aria-expanded', 'false'); go('account', { name: el.dataset.name }); });
// a click outside the open popup closes it
document.addEventListener('click', (e) => { const root = document.getElementById('tray-root'); if (root && root.innerHTML && !root.contains(e.target) && !e.target.closest('.tray-trigger')) { root.innerHTML = ''; document.querySelector('.tray-trigger')?.setAttribute('aria-expanded', 'false'); } });

// ---------- the Accounts page ----------
// usage drawn into the rows: the range, each account's breakdown and day series, and the colour it has in the chart
const au = { days: 7, by: {}, series: {}, color: {}, models: [], days_list: null, fold: new Set(['chart', 'models']) };
let acctTool = '';
const folded = new Set();
const sparkSvg = (vals) => { const v = vals.slice(-7), m = Math.max(1, ...v); return `<svg class="spark-bars" viewBox="0 0 67 26" aria-label="7-day token sparkline">${v.map((x, i) => (x > 0 ? `<rect x="${i * 10}" y="${26 - Math.max(2, Math.round((x / m) * 24))}" width="7" height="${Math.max(2, Math.round((x / m) * 24))}" rx="1.5"/>` : '')).join('')}</svg>`; };
const SERIES = 8;

function windowsCol(a, best, used, now) {
  const lines = [];
  if (a.cap) {
    const left = Math.max(0, Math.round(100 - (a.cap.spentUsd / a.cap.limitUsd) * 100)), pct = used ? 100 - left : left;
    lines.push(`<div class="win-bar-line"><span class="lbl">${money(a.cap.limitUsd)} daily cap</span>${winTrack(pct, left <= 28 ? 'warn' : 'r2')}<span class="val">${pct}% ${used ? 'used' : 'left'}</span><span class="rs">↻ ${fmtDuration(a.cap.resetMs)}</span></div>`);
  }
  for (const w of a.windows) {
    const pct = used ? w.used : w.left;
    lines.push(`<div class="win-bar-line"><span class="lbl">${esc(w.label)}</span>${winTrack(pct, w.left === 0 ? 'neg' : w.left <= 28 ? 'warn' : '')}<span class="val ${w.left === 0 ? 'neg' : ''}">${w.left === 0 ? 'Used up' : `${pct}% ${used ? 'used' : 'left'}`}</span><span class="rs" title="${w.open ? 'Resets ' + esc(fmtAbsoluteReset(w.resetsAt, now)) : ''}">${w.open ? '↻ ' + fmtDuration(w.resetMs) : w.resetsAt ? 'window has reset' : ''}</span></div>`);
  }
  if (!lines.length) lines.push(`<div class="acct-nolimits">${a.kind === 'api_key' ? 'Pay per use · no plan limits' : a.kind === 'routed' ? `Through route ${esc(a.route || '')}` : a.needsLogin ? 'Log in to see its plan and limits.' : a.noPlan ? 'No plan limits · usage only' : '— plan limits not read'}</div>`);
  // the line under the limits: the best place to start, or a window that runs out before it resets
  const out = a.windows.find((w) => w.outAt);
  if (best && a.id === best.id) lines.push('<div class="acct-advice" data-testid="best-note">Most room of any account: start here next.</div>');
  else if (out) lines.push(`<div class="acct-advice warn">${esc(out.label)} runs out ≈ ${esc(fmtAbsoluteReset(out.outAt, now))} at this pace.</div>`);
  return lines.join('');
}

function accountRow(a, best, now, used, pcs) {
  const p = a.p, b = au.by[a.id], vals = au.series[a.id] || [];
  const live = a.tool === 'claude' && a.kind === 'subscription' && p.identity.logged_in;
  return `<div class="acct-row" tabindex="0" role="button" data-act="open-account" data-name="${esc(a.id)}" data-key="${esc(a.id)}" data-testid="row-${esc(a.id)}">
    <div class="acct-who">${renderRingSvg(a, used)}<div>
      <div class="acct-name-row"><b class="acct-name">${esc(a.name)}</b>${a.isMain ? `<span class="plan-pill" title="Main tool profile (${esc(a.folder)}): can be hidden, not deleted">main</span>` : ''}${a.mismatch ? '<span class="plan-pill neg" title="Logged in as a different email than you entered">email differs</span>' : ''}</div>
      <div class="acct-email">${esc(a.email || p.identity.name || p.base_url || (p.identity.logged_in ? 'logged in' : 'not logged in'))}</div>
      <div class="acct-plan-row"><span class="plan-pill ${a.planTone}" title="${esc(p.identity.plan || '')}">${esc(a.plan)}</span>${a.added ? `<span class="acct-added">Added ${esc(a.added)}</span>` : ''}</div></div></div>
    <div class="acct-windows-col">${windowsCol(a, best, used, now)}</div>
    <div title="${esc(au.days + ' days: ' + (vals.length ? vals.map((v) => fmt(v)).join(' · ') : 'no usage'))}">${sparkSvg(vals.length ? vals : [0])}<div class="spark-note">${au.days}-day pace</div></div>
    <div class="acct-totals" title="${b ? esc(`input ${fmt(b.input)} · output ${fmt(b.output)} · cache read ${fmt(b.cache_read)} · ${b.requests.toLocaleString()} requests · ${Math.round(b.cache_hit_rate * 100)}% from cache`) : 'no usage in this range'}"><div class="acct-tokens">${fmt(b ? b.tokens : 0)} <small>tokens</small></div>
      <div class="acct-cost" data-testid="cost-${esc(a.id)}">${a.cap ? `${money(a.cap.spentUsd)} <small>today, cap ${money(a.cap.limitUsd)}</small>` : `${money(b ? b.cost : null)} <small>list ≈</small>`}</div>${a.openSessions ? `<div class="acct-open">● ${a.openSessions} open${a.far.length ? ' · ' + [...new Set(a.far.map((r) => nameOf(pcs, r.device)))].map(esc).join(', ') : ''}</div>` : ''}</div>
    <div class="acct-actions"><span class="acct-read" title="Limits read ${a.readAt ? agoText(a.readAt, now) : 'never'}">${a.readAt ? readAgo(a.readAt, now) : ''}</span>${live ? `<button class="icon-btn" data-act="quota-live" data-name="${esc(a.id)}" title="Read this account's limits from Anthropic now, with the login Claude Code saved" aria-label="Read limits of ${esc(a.name)} now" data-testid="live-${esc(a.id)}">↻</button>` : ''}
      ${p.identity.logged_in || a.kind !== 'subscription' ? `<button class="btn-lane-start" data-act="start" data-name="${esc(a.id)}" data-testid="start-${esc(a.id)}">Start</button>` : `<button class="btn-lane-start" data-act="login" data-name="${esc(a.id)}" data-testid="login-${esc(a.id)}">Log in</button>`}
      <button class="icon-btn" data-act="account-menu" data-name="${esc(a.id)}" data-home="${esc(a.folder)}" data-main="${a.isMain ? 1 : ''}" title="Account actions (folder, settings, sync, remove)" aria-label="More for ${esc(a.name)}" data-testid="menu-${esc(a.id)}">⋯</button></div></div>`;
}

/** The finding: the account with the most room, then what else needs saying. */
export function findings(accts, now) {
  const { bestRoomAcct: best } = metricsOf(accts);
  let text = '';
  if (best) { const s = w5of(best), w = wkof(best); text = `<span class="lead-name">${esc(best.name)}</span> has the most room: ${s ? s.used + '% of its 5-hour limit and ' : ''}${w ? w.used + '% of its week used.' : 'ready to start.'}`; }
  else text = accts.length ? 'Limits have not been read yet.' : 'No accounts configured yet.';
  const extra = [];
  for (const a of accts.filter(eligible)) for (const w of a.windows) if (w.open && w.used >= 75) extra.push(`<b>${esc(a.name)}</b> is at ${w.used}% of its ${esc(w.label.toLowerCase())} until ${esc(fmtAbsoluteReset(w.resetsAt, now))}.`);
  const mm = accts.find((a) => a.mismatch); if (mm) extra.push(`<b>${esc(mm.name)}</b> is logged in as a different email than expected.`);
  const notIn = accts.filter((a) => a.kind === 'subscription' && !a.p.identity.logged_in).length; if (notIn) extra.push(`${notIn} ${notIn === 1 ? 'account is' : 'accounts are'} not logged in yet.`);
  return { best, text, extra: extra.slice(0, 3).join(' ') };
}

function stackedChart(days, series, short) {
  const totals = days.map((d, i) => series.reduce((s, x) => s + (x.vals[i] || 0), 0)), max = Math.max(1, ...totals), H = short ? 58 : 90;
  return `<div class="stacked-chart-box ${short ? 'short' : ''}">${days.map((d, i) => {
    const total = totals[i], h = total ? Math.max(8, Math.round((total / max) * H)) : 0; let y = h;
    const segs = series.map((x) => { const v = x.vals[i] || 0; if (!v || !total) return ''; const sh = Math.max(1, Math.round((v / total) * h)); y -= sh; return `<rect class="c${x.ci}" y="${Math.max(0, y)}" width="100%" height="${sh}"><title>${esc(x.name)}: ${fmt(v)}</title></rect>`; }).join('');
    const label = new Date(d + 'T12:00').toLocaleDateString([], { month: 'short', day: 'numeric' });
    return `<div class="stack-col" title="${esc(label)}: ${fmt(total)} tokens"><span class="stack-tot">${total ? fmt(total) : ''}</span><svg class="stack-bars" height="${h}" aria-hidden="true">${segs}</svg><span class="stack-lbl">${days.length > 14 && i % 2 ? '' : esc(label)}</span></div>`;
  }).join('')}</div>`;
}

view('accounts', async (sheet) => {
  sheet.className = 'sheet';
  const [st, u, x] = await Promise.all([loadState(), call('usage', { days: au.days }).catch(() => null), call('limits_extra').catch(() => ({}))]);
  if (u) {
    au.by = Object.fromEntries(u.accounts.map((b) => [b.key, b]));
    au.series = u.summary.by_profile || {};
    au.models = u.models || [];
    au.color = Object.fromEntries(u.accounts.map((b, i) => [b.key, i % SERIES]));
    au.days_list = u.summary.days;
  }
  const now = st.now, pcs = S.cache.computers, used = showUsed();
  const accts = accountViews(st, x, now, pcs);
  if (st.first_run && (st.first_run.main_profiles.length || st.first_run.accounts.length)) toast(`Found ${st.first_run.main_profiles.length} installed tools and imported ${st.first_run.accounts.length} accounts from your PowerShell launcher.`);
  const { best, text, extra } = findings(accts, now);
  const q = (document.getElementById('find').value || '').toLowerCase();
  const shown = accts.filter((a) => !q || a.id.toLowerCase().includes(q) || a.name.toLowerCase().includes(q) || a.email.toLowerCase().includes(q));
  const read = accts.map((a) => a.readAt).filter(Boolean), readAgo = read.length ? 'limits read ' + agoText(Math.max(...read), now) : 'limits not read';
  const tools = shownTools(st), groups = tools.filter((h) => !acctTool || h.id === acctTool);
  const series = accts.filter((a) => au.series[a.id] && au.series[a.id].some((v) => v)).map((a) => ({ name: a.name, ci: au.color[a.id] || 0, vals: au.series[a.id] }));
  const chartOpen = !au.fold.has('chart'), modelsOpen = !au.fold.has('models'), topModel = au.models[0] ? au.models[0].tokens : 1;
  const nTools = new Set(accts.map((a) => a.tool)).size;
  patch(sheet, `<div class="page-body">
    <section class="accounts-top-bar"><div class="accounts-finding"><h1 data-testid="finding">${text}</h1><p>${accts.length} ${accts.length === 1 ? 'account' : 'accounts'} across ${nTools} ${nTools === 1 ? 'tool' : 'tools'}, each in its own folder with its own login · sorted by room left · usage at API list prices (≈).${extra ? ' ' + extra : ''}</p></div>
      <div class="accounts-actions"><div class="chip-group" role="group" aria-label="Accounts date range">${[7, 14, 30].map((d) => `<button class="filter-chip ${d === au.days ? 'active' : ''}" aria-pressed="${d === au.days}" data-act="acct-days" data-v="${d}" data-testid="u-days-${d}">${d}d</button>`).join('')}</div>
        <button class="btn-secondary" data-act="acct-csv" data-testid="u-csv">Export CSV</button>${best ? `<button class="btn-secondary" data-act="start-in-folder" data-name="${esc(best.id)}" data-testid="start-best">Start in a folder…</button>` : ''}<button class="btn-primary" data-act="add-account" data-testid="add-account">+ Add account</button></div></section>
    <div class="accounts-grid ${modelsOpen ? 'with-models' : ''}">
      <section class="panel-card" data-testid="usage-panel"><div class="panel-head"><div class="head-group"><button class="panel-title fold" data-act="acct-fold" data-h="chart" aria-expanded="${chartOpen}">Tokens per day, by account (${au.days}d)</button>
        ${chartOpen ? `<div class="chart-legend">${series.map((s) => `<span><i class="pc-dot c${s.ci}"></i>${esc(s.name)}</span>`).join('')}</div>` : ''}</div>
        <button class="filter-chip ${modelsOpen ? 'active' : ''}" data-act="acct-fold" data-h="models" aria-expanded="${modelsOpen}">${modelsOpen ? '▾ Hide By model' : '▸ By model'}</button></div>
        ${chartOpen ? (series.length && au.days_list ? `<div data-testid="usage-ledger">${stackedChart(au.days_list, series, true)}</div>` : '<span class="muted">No usage in this range.</span>') : ''}</section>
      ${modelsOpen ? `<section class="panel-card" data-testid="models-panel"><div class="panel-head"><h2 class="panel-title">By model (${au.days}d)</h2><span class="panel-note">API list prices ≈</span></div>
        <div class="model-rows">${au.models.map((m) => `<div class="win-bar-line"><span class="mono">${esc(m.key)}</span>${winTrack(Math.max(4, Math.round((m.tokens / topModel) * 100)))}<span>${fmt(m.tokens)}</span><span class="r">${m.cost == null ? '—' : money(m.cost)}</span></div>`).join('') || '<span class="muted">No usage yet.</span>'}</div></section>` : ''}
    </div>
    <div class="horizon-filterbar boxed"><div class="chip-group" role="group" aria-label="Tool"><button class="filter-chip ${!acctTool ? 'active' : ''}" data-act="acct-tool" data-v="" data-testid="chip-all">All (${accts.length})</button>${tools.map((h) => `<button class="filter-chip ${acctTool === h.id ? 'active' : ''}" data-act="acct-tool" data-v="${h.id}" data-testid="chip-${h.id}">${toolIcon(h.id, 14)}<span>${esc(h.label)}</span></button>`).join('')}</div>
      <span class="panel-note">Sorted by room left · ${readAgo} · click or press Enter on any row for account detail</span></div>
    <div data-testid="accounts-table">${groups.map((h) => {
      const list = shown.filter((a) => a.tool === h.id).sort((a, b) => (eligible(b) - eligible(a)) || (b.measured - a.measured) || ((b.measured && a.measured) ? (wkof(b).left - wkof(a).left) : 0));
      const open = !folded.has(h.id), on = list.filter((a) => a.p.identity.logged_in).length, consoles = list.reduce((s, a) => s + a.openSessions, 0);
      const next = Math.min(...list.flatMap((a) => a.windows.filter((w) => w.open).map((w) => w.resetMs)).concat([Infinity]));
      const tok = list.reduce((s, a) => s + (au.by[a.id] ? au.by[a.id].tokens : 0), 0), cst = list.some((a) => au.by[a.id] && au.by[a.id].cost != null) ? list.reduce((s, a) => s + ((au.by[a.id] || {}).cost || 0), 0) : null;
      return `<section class="tool-group-card" data-key="g-${h.id}" data-testid="group-${h.id}"><button class="tool-group-header" data-act="acct-fold" data-h="${h.id}" aria-expanded="${open}">
        <div class="group-title"><span class="chev" aria-hidden="true"></span>${toolIcon(h.id, 14)}<h2 class="panel-title">${esc(h.label)}</h2>${list.length ? `<span class="n">${list.length}</span>` : ''}</div>
        ${list.length ? `<div class="group-meta"><span>${on} logged in</span><span class="ac">${consoles} ${consoles === 1 ? 'console' : 'consoles'} open</span><span>next reset ${isFinite(next) ? fmtDuration(next) : '—'}</span>${tok ? `<span>${fmt(tok)} tokens</span><span>${money(cst)}</span>` : ''}</div>` : ''}</button>
        ${!open ? '' : list.length ? list.map((a) => accountRow(a, best, now, used, pcs)).join('') : `<div class="group-empty">${h.installed ? (q ? 'No account matches the search.' : `No ${esc(h.label)} account yet.`) : `${esc(h.label)} is not installed on this computer.`}</div>`}</section>`;
    }).join('')}</div></div>`);
});
action('acct-days', (el) => { au.days = +el.dataset.v; render(); });
action('acct-csv', async () => {
  const to = await saveFile(`Harness Manager usage ${new Date().toISOString().slice(0, 10)}.csv`, { name: 'CSV', extensions: ['csv'] });
  if (to) attempt(() => call('usage_csv', { days: au.days, path: to }), 'Exported');
});
action('acct-tool', (el) => { acctTool = el.dataset.v; render(); });
action('acct-fold', (el) => { const h = el.dataset.h, set = h === 'chart' || h === 'models' ? au.fold : folded; if (set.has(h)) set.delete(h); else set.add(h); render(); });

// ---------- account detail ----------
const STARTED_WITH = { claude: 'CLAUDE_CONFIG_DIR', codex: 'CODEX_HOME', opencode: 'XDG_CONFIG_HOME', agy: 'its own login', pi: 'PI_CODING_AGENT_DIR', omp: 'PI_CODING_AGENT_DIR' };
const sessionLine = (s, pcs) => `<div class="account-session" data-key="s-${esc(s.id)}" data-testid="session-${esc(s.id)}"><div><b><i class="session-dot ${s.open ? 'live' : s.failed ? 'bad' : ''}" ${s.failed ? `title="${s.failed} failed ${s.failed === 1 ? 'command' : 'commands'}"` : ''}></i>${esc(s.title || 'Untitled session')}</b><div class="session-meta"><span>${esc(s.project_name || (s.cwd || '').split(/[\\/]/).pop() || '')}</span>${syncOn(pcs) && s.device ? `<span>${esc(s.device_name || nameOf(pcs, s.device))}</span>` : ''}<span class="when ${s.open ? 'live' : ''}">${s.open ? 'live' : esc(when(s.last_active_at))}</span>${s.archived ? '<span class="plan-pill">archived</span>' : ''}</div></div>
  ${s.open ? `<button class="btn-follow" data-act="open-session" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}">Follow</button>` : `<button class="filter-chip" data-act="open-session" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}">Open</button>`}</div>`;

view('account', async (sheet, params) => {
  const name = params.name, tab = params.tab || 'overview';
  const d = await call('detail', { name });
  const p = d.view, now = Date.now(), x = await call('limits_extra').catch(() => ({})), pcs = S.cache.computers;
  const a = acctView(p, x, now, pcs), used = showUsed();
  sheet.className = 'sheet';
  const tabs = [['overview', 'Overview'], ['sessions', 'Sessions'], ['projects', 'Projects'], ['settings', 'Settings']];
  const tool = (S.state && S.state.harnesses.find((h) => h.id === p.harness)) || { installed: true };
  const running = d.sessions.filter((s) => s.open).length;
  let body = '';
  if (tab === 'overview') {
    const [k, u] = await Promise.all([call('account_stats', { name }).catch(() => null), call('usage', { days: 30, profile: name }).catch(() => null)]);
    const models = (u && u.models) || [], topModel = models[0] ? models[0].tokens : 1, w = k ? k.week : null;
    const secs = (ms) => (ms == null ? '—' : '≈ ' + (ms / 1000).toFixed(1) + ' s');
    const week = d.sessions.filter((s) => now - s.last_active_at < 7 * 86400000);
    const stat = (title, value, note) => `<div class="account-stat"><span>${title}</span><b>${value}</b><span>${note}</span></div>`;
    // the bars mean % left like every other bar of the app; the dashed mark is ≈ what is left at the reset
    const lim = (t) => {
      const pct = used ? t.used : t.left, at = t.forecastAtReset != null ? (used ? t.forecastAtReset : 100 - t.forecastAtReset) : null;
      const reset = !t.resetsAt ? 'no window open' : !t.open ? 'window has reset' : 'resets ' + (t.resetMs < 86400000 ? 'in ' + fmtDuration(t.resetMs) : fmtAbsoluteReset(t.resetsAt, now));
      const note = t.outAt ? ['≈ Runs out ' + fmtAbsoluteReset(t.outAt, now) + ' at this pace.', 'warn'] : t.forecastAtReset != null ? ['≈ Lasts until the reset at this pace.', ''] : t.left >= 60 ? ['Plenty left.', ''] : null;
      return `<div class="account-limit" data-key="w-${esc(t.id)}"><div class="account-limit-head"><b>${esc(t.label)}</b><span class="${t.left === 0 ? 'neg' : t.left < 20 ? 'warn' : ''}">${t.left === 0 ? 'Used up' : `${pct}% ${used ? 'used' : 'left'}`} · ${esc(reset)}</span></div>${winTrack(pct, t.left === 0 ? 'neg' : t.left < 20 ? 'warn' : '', at)}${note ? `<span class="note ${note[1]}">${esc(note[0])}</span>` : ''}</div>`;
    };
    const facts = [
      ['Login', p.kind !== 'subscription' ? (p.identity.logged_in ? 'Key saved' : '<span class="negT">No key</span>') : !p.identity.logged_in ? '<span class="negT">Not logged in</span>' : p.identity.needs_login ? '<span class="negT">Expired</span>' : '<span class="acT">Valid</span>'],
      ['Folder', `<button class="btn-secondary" data-act="open-folder" data-path="${esc(p.home)}" title="Open the folder">${esc(p.home)}</button>`],
      ['Started with', `<span class="mono">${esc(STARTED_WITH[p.harness] || '—')}</span>`],
      ...(p.harness === 'claude' && !p.main ? [['Follows main', 'CLAUDE.md, rules, skills, MCP'], ['Own', 'Login, history, usage']] : []),
      ...(p.harness === 'claude' && p.kind === 'subscription' ? [['Live limits', S.state && S.state.live_quota ? 'On (reads the saved login)' : 'Off']] : []),
      ['CLI', `<span class="mono">${esc(d.cli)}</span>`],
    ];
    const per = k ? k.per_day : [], max = Math.max(1, ...per);
    const dayLabel = (i) => new Date(k.days[i] + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short' });
    body = `<section class="account-stats" aria-label="Account readouts">${stat('Tokens · 7 days', w ? fmt(w.tokens) : '—', w ? Math.round(w.hit * 100) + '% read from cache' : 'Cache share not read')}${stat('At list prices', w ? money(w.cost) : '—', p.kind === 'subscription' ? 'Subscription: not billed per token' : 'Billed per token')}${stat('First reply p95', w && w.p95 != null ? secs(w.p95) : '—', w && w.replies ? `p50 ${secs(w.p50)} · ${w.replies.toLocaleString()} replies` : 'No timed replies')}${stat('Sessions · 7 days', week.length, running ? `${running} running now` : 'None running now')}${stat('Busiest project', w && w.top_project ? Math.round(w.top_project.share * 100) + '%' : '—', w && w.top_project ? esc(w.top_project.name) : 'No usage this week')}</section>
      <div class="account-columns"><section class="panel-card"><div class="panel-head"><h2 class="panel-title">Limits</h2><span class="muted">${a.readAt ? 'read ' + agoText(a.readAt, now) : p.caps.quota ? 'not read yet' : esc(p.harness_label) + ' has no limits to read'}</span></div>
          ${a.windows.length ? a.windows.map(lim).join('') + `<span class="muted">Bars: % ${used ? 'used' : 'left'} · dashed mark: ≈ level at reset.</span>` : `<span class="muted">${p.kind !== 'subscription' ? 'API keys and routes have no plan limits.' : !PLAN_TOOLS.includes(p.harness) ? esc(p.harness_label) + ' has no limits to read.' : '— Limits show up after this account runs on this computer.'}</span>`}</section>
        <section class="panel-card"><h2 class="panel-title">Login and folder</h2><dl class="facts-grid">${facts.map(([t, v]) => `<dt>${t}</dt><dd>${v}</dd>`).join('')}</dl>${p.kind === 'subscription' ? `<button class="btn-secondary" data-act="login" data-name="${esc(name)}">Log in again</button>` : ''}</section></div>
      <div class="account-columns"><section class="panel-card"><div class="panel-head"><h2 class="panel-title">Tokens per day · 30 days</h2><span class="muted">${k && per.some(Boolean) ? fmt(per.reduce((s, v) => s + v, 0)) + ' · ' + money(k.cost30) + ' at list prices' : '—'}</span></div>
          ${per.some(Boolean) ? `<div class="account-chart">${per.map((v, i) => `<div class="stack-col ${i === per.length - 1 ? 'today' : ''}" title="${esc(dayLabel(i))}: ${fmt(v)} tokens"><svg class="stack-bars" height="${Math.round((v / max) * 76)}" aria-hidden="true"><rect width="100%" height="100%" rx="2"/></svg><span class="stack-lbl">${i === 0 || i === 14 ? esc(dayLabel(i)) : i === per.length - 1 ? 'Today' : ''}</span></div>`).join('')}</div>` : '<span class="muted">No usage yet.</span>'}</section>
        <section class="panel-card" data-testid="account-models"><h2 class="panel-title">By model · 30 days</h2>${models.length ? `<div class="model-rows">${models.map((m) => `<div class="win-bar-line"><span class="mono">${esc(m.key)}</span>${winTrack(Math.round((m.tokens / topModel) * 100))}<span>${fmt(m.tokens)}</span><span class="r">${m.cost == null ? '—' : money(m.cost)}</span></div>`).join('')}</div>` : '<span class="muted">No usage yet.</span>'}</section></div>
      ${p.running.length ? `<section class="panel-card"><h2 class="panel-title">Open now</h2>${p.running.map((r) => `<div class="account-session"><div><span class="mono">${esc(r.cwd || '— Folder not read')}</span><div class="muted">PID ${r.pid}${r.started_at ? ' · since ' + hm(r.started_at) : ''}</div></div><button class="btn-secondary" data-act="show-window" data-pid="${r.pid}">Show window</button></div>`).join('')}</section>` : ''}
      <section class="panel-card"><div class="panel-head"><h2 class="panel-title">Recent sessions</h2><a class="filter-chip" href="#account?name=${encodeURIComponent(name)}&tab=sessions">All ${d.sessions.length}</a></div>${d.sessions.slice(0, 5).map((s) => sessionLine(s, pcs)).join('') || '<span class="muted">No sessions in the last 30 days.</span>'}</section>`;
  } else if (tab === 'sessions') {
    body = `<section class="panel-card"><h2 class="panel-title">Sessions · ${d.sessions.length}</h2>${d.sessions.map((s) => sessionLine(s, pcs)).join('') || '<span class="muted">No sessions in the last 30 days.</span>'}</section>`;
  } else if (tab === 'projects') {
    body = `<section class="panel-card"><h2 class="panel-title">Projects · 7 days</h2>${d.projects.length ? `<table class="replies-table"><thead><tr><th>Project</th><th>Key</th><th class="num">Tokens</th></tr></thead><tbody>${d.projects.map(([k, n, t]) => `<tr><td><a class="filter-chip" href="#sessions?project=${encodeURIComponent(k)}">${esc(n)}</a></td><td class="mono muted">${esc(k)}</td><td class="num">${fmt(t)}</td></tr>`).join('')}</tbody></table>` : '<span class="muted">No usage this week.</span>'}</section>`;
  } else {
    const field = (id, label, value, type = 'text', extra = '') => `<div class="form-field"><label for="${id}">${label}</label><input id="${id}" type="${type}" value="${esc(value ?? '')}" ${extra}></div>`;
    const row = (fields, act, title = 'Save', testid = '') => `<div class="settings-row">${fields}<button class="btn-secondary" data-act="${act}" data-name="${esc(name)}" ${testid ? `data-testid="${testid}"` : ''}>${title}</button></div>`;
    body = `<div class="account-settings"><section class="panel-card"><h2 class="panel-title">Profile settings</h2>${row(field('pf-label', 'Name', p.label, 'text', `placeholder="${esc(name)}" maxlength="60" data-testid="rename-input"`), 'save-label', 'Save', 'rename-save')}<span class="muted">How this account is shown. Its folder and the command line keep the id <span class="mono">${esc(name)}</span>.</span>
        ${row(field('pf-email', 'Expected email', p.expected_email || '', 'email'), 'save-email')}<span class="muted">Checked after each login, so you notice a wrong account.</span></section>
      ${p.kind === 'api_key' ? `<section class="panel-card"><h2 class="panel-title">API key</h2>${row(field('pf-key', 'New API key', '', 'password', 'placeholder="Paste a new key"'), 'save-key', 'Replace key')}<span class="muted">Kept in Windows Credential Manager, never in a file.</span>${row(field('pf-url', 'Base URL', p.base_url || '') + field('pf-model', 'Model', p.model || ''), 'save-endpoint')}<span class="muted">Leave empty to use the vendor's own API and default model.</span></section>` : ''}
      ${p.kind === 'routed' ? `<section class="panel-card"><h2 class="panel-title">Daily spend cap</h2>${row(field('pf-cap', 'Cap in USD', p.daily_cap_usd ?? '', 'number', 'min="0" step="0.5" placeholder="no cap" data-testid="cap-input"'), 'save-cap', 'Save', 'cap-save')}<span class="muted">The most the router spends for this account in a day, at API list prices. At the cap it stops answering for it until midnight. Empty: no cap.</span></section>` : ''}
      ${p.harness === 'claude' && !p.main ? `<section class="panel-card"><h2 class="panel-title">Shared with the main profile</h2><span>CLAUDE.md, rules, skills, plugins and MCP servers follow the main profile. Login, history and usage stay with this account.</span><div><button class="btn-secondary" data-act="sync-now">Sync now</button></div></section>` : ''}
      <section class="panel-card"><h2 class="panel-title">${p.main ? 'Hide main profile' : 'Remove account'}</h2><span class="muted">${p.main ? "Stops managing the tool's own profile. Its folder and login are never deleted." : 'Removes it from Harness Manager. Its folder stays unless you choose to delete it.'}</span><div><button class="btn-secondary negT" data-act="remove-account" data-name="${esc(name)}" data-main="${p.main ? 1 : ''}" data-testid="remove-account">${p.main ? 'Hide main profile…' : 'Remove account…'}</button></div></section></div>`;
  }
  const open = p.running.length ? `${p.running.length} ${p.running.length === 1 ? 'console' : 'consoles'} open${p.running[0].cwd ? ' in ' + esc(p.running[0].cwd.split(/[\\/]/).pop()) : ''}` : '';
  patch(sheet, `<div class="page-body"><a class="filter-chip back-chip" href="#accounts">← Accounts</a>
    <header class="page-heading"><div class="account-identity"><div class="account-avatar" aria-hidden="true">${esc(p.label.split(/[-_ ]/).filter(Boolean).slice(0, 2).map((s) => s[0].toUpperCase()).join(''))}</div><div><h1 data-testid="detail-name">${toolIcon(p.harness, 20)}<span>${esc(p.label)}</span></h1>
      <div class="heading-facts"><span>${esc(p.harness_label)}</span>${a.email ? `<span>${esc(a.email)}</span>` : ''}<span class="plan-pill ${a.planTone}">${esc(a.plan)}</span><span>${esc([p.identity.org, p.identity.role].filter(Boolean).join(' · '))}${p.main ? (p.identity.org ? ' · ' : '') + 'main profile' : ''}</span></div>${open ? `<div class="muted">${open}</div>` : ''}</div></div>
      <div class="page-actions"><button class="btn-secondary" data-act="start-in-folder" data-name="${esc(name)}">Start in a folder…</button><button class="btn-primary" data-act="start" data-name="${esc(name)}" ${(!p.identity.logged_in && p.kind === 'subscription') || !tool.installed ? 'disabled' : ''}>Start ${esc(p.label)}</button><button class="icon-btn" data-act="account-menu" data-name="${esc(name)}" data-home="${esc(p.home)}" data-main="${p.main ? 1 : ''}" aria-label="More" title="Account actions">⋯</button></div></header>
    <nav class="page-tabs" aria-label="Account tabs">${tabs.map(([k, l]) => `<a class="filter-chip ${k === tab ? 'active' : ''}" ${k === tab ? 'aria-current="page"' : ''} href="#account?name=${encodeURIComponent(name)}&tab=${k}" data-testid="tab-${k}">${l}</a>`).join('')}</nav>${body}</div>`);
});

// ---------- actions ----------
action('open-account', (el, e) => { if (!e.target.closest('button, a')) go('account', { name: el.dataset.name }); });
action('start', (el) => attempt(() => call('start', { name: el.dataset.name }), `Opened a terminal for ${acctName(el.dataset.name)}`));
// the tool's own flag for running without asking before each step, where it has one
const SKIP = { claude: '--dangerously-skip-permissions', codex: '--dangerously-bypass-approvals-and-sandbox' };
const startable = () => (S.state.profiles || []).filter((p) => shownTools(S.state).some((h) => h.id === p.harness) && (p.kind !== 'subscription' || p.identity.logged_in));

/** Start an account in a folder: a recent project of this account or any folder, optionally without
 * permission prompts, with extra arguments for this terminal only. */
async function startDialog(name) {
  const d = await attempt(() => call('detail', { name }));
  if (!d) return;
  const p = d.view, label = p.label;
  const recent = [...new Set(d.sessions.map((s) => s.cwd).filter(Boolean))].slice(0, 6);
  let dir = recent[0] || '';
  const others = startable().filter((x) => x.name !== name);
  openDialog(`${dlgHead('Start ' + label + ' in a folder…')}
    <div class="form-field"><label for="sd-acct">Account</label><select id="sd-acct" data-act="sd-acct"><option value="${esc(name)}" selected>${esc(label)}${p.identity.plan ? ' (' + esc(p.identity.plan) + ')' : ''}</option>${others.map((x) => `<option value="${esc(x.name)}">${esc(x.label)}${x.identity.plan ? ' (' + esc(x.identity.plan) + ')' : ''}</option>`).join('')}</select></div>
    <span class="dlg-text">Runs as <b>${esc(label)}</b>, with its own login, history and usage. Other accounts, and the consoles they have open, are not affected.</span>
    <div class="form-field"><label for="sd-recent">Folder</label><div class="field-row">${recent.length ? `<select id="sd-recent" class="mono" data-act="sd-recent" aria-label="Recent folders of this account">${recent.map((r) => `<option value="${esc(r)}">${esc(r)}</option>`).join('')}</select>` : ''}<button class="btn-secondary" id="sd-choose">Choose folder…</button></div><span class="mono muted" id="sd-dir" data-testid="sd-dir">${esc(dir || 'No folder chosen')}</span></div>
    ${SKIP[p.harness] ? `<label class="check-row"><input type="checkbox" id="sd-skip" data-testid="sd-skip"><span>Skip permission prompts (<code class="mono">${SKIP[p.harness]}</code>)</span></label><span class="muted">${esc(p.harness_label)} then runs commands and edits files without asking first. Only for folders you trust.</span>` : ''}
    <div class="form-field"><label for="sd-args">Extra arguments <span class="opt">(optional)</span></label><input class="mono" id="sd-args" placeholder='e.g. --model opus --append-system-prompt "Be brief"' data-testid="sd-args"><span class="muted">Added after the start command, for this terminal only. Separate with spaces; quote values that contain spaces.</span></div>
    <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" id="sd-ok" data-testid="sd-ok" ${dir ? '' : 'disabled'}>▶ Open terminal</button></span></div>`, { size: 'sm', label: 'Start in a folder' });
  const set = (v) => { dir = v; document.getElementById('sd-dir').textContent = v || 'No folder chosen'; document.getElementById('sd-ok').disabled = !v; };
  document.getElementById('sd-choose').onclick = async () => { const v = await pickFolder(); if (v) set(v); };
  const rec = document.getElementById('sd-recent'); if (rec) rec.onchange = () => set(rec.value);
  document.getElementById('sd-ok').onclick = async () => {
    const extra = (document.getElementById('sd-args').value.match(/"[^"]*"|'[^']*'|\S+/g) || []).map((a) => a.replace(/^(["'])(.*)\1$/, '$2'));
    const skip = document.getElementById('sd-skip'), who = document.getElementById('sd-acct').value;
    const args = [...(skip && skip.checked ? [SKIP[p.harness]] : []), ...extra];
    if (await attempt(() => call('start', { name: who, cwd: dir, args }), `Opened ${acctName(who)} in ${dir}`)) closeDialog();
  };
}
action('start-in-folder', (el) => startDialog(el.dataset.name));
action('sd-acct', (el) => { closeDialog(); startDialog(el.value); });

/** A new session in a project's folder, or in a folder picked here when no project is given, as any account
 * of any tool; the project's latest account comes first. */
function newInProject(dir, project, preferred) {
  const ps = startable();
  let name = (ps.find((p) => p.name === preferred) || ps[0] || {}).name;
  openDialog(`${dlgHead(project ? 'New session in ' + project : 'New session')}
    <div class="form-field"><label>Folder</label><div class="field-row"><span class="mono" id="np-dir" data-testid="np-dir">${esc(dir || 'No folder chosen')}</span>${project ? '' : `<button class="btn-secondary" id="np-choose" data-testid="np-choose">Choose folder…</button>`}</div></div>
    <div class="form-field"><label>Account</label>${ps.length ? `<div class="chip-group np-accts">${ps.map((p) => `<button class="filter-chip" data-name="${esc(p.name)}" data-testid="np-${esc(p.name)}">${toolIcon(p.harness, 14)} ${esc(p.label)}</button>`).join('')}</div>` : '<span class="muted">No account can start yet: add one, or log one in, on Accounts.</span>'}</div>
    <div id="np-skipw" hidden><label class="check-row"><input type="checkbox" id="np-skip" data-testid="np-skip"><span>Skip permission prompts <code class="mono" id="np-flag"></code></span></label><span class="muted">The tool then runs commands and edits files without asking first. Only for folders you trust.</span></div>
    <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" id="np-ok" data-testid="np-ok" ${name && dir ? '' : 'disabled'}>▶ Start session</button></span></div>`, { size: 'sm', label: 'New session' });
  const pick = (n) => {
    name = n;
    const flag = SKIP[ps.find((x) => x.name === n).harness];
    document.querySelectorAll('.np-accts .filter-chip').forEach((b) => b.classList.toggle('active', b.dataset.name === n));
    document.getElementById('np-skipw').hidden = !flag;
    document.getElementById('np-flag').textContent = flag || '';
    document.getElementById('np-skip').checked = false;
  };
  document.querySelectorAll('.np-accts .filter-chip').forEach((b) => (b.onclick = () => pick(b.dataset.name)));
  if (name) pick(name);
  const choose = document.getElementById('np-choose');
  if (choose) choose.onclick = async () => { const v = await pickFolder(); if (!v) return; document.getElementById('np-dir').textContent = dir = v; document.getElementById('np-ok').disabled = !name; };
  document.getElementById('np-ok').onclick = async () => {
    const p = ps.find((x) => x.name === name), skip = document.getElementById('np-skip').checked;
    if (await attempt(() => call('start', { name, cwd: dir, args: skip ? [SKIP[p.harness]] : [] }), `Opened ${p.label} in ${project || dir}`)) closeDialog();
  };
}
action('project-new', (el) => newInProject(el.dataset.dir, el.dataset.name, el.dataset.profile));
action('session-new', () => newInProject('', ''));
action('show-window', async (el) => { const ok = await attempt(() => call('show_window', { pid: +el.dataset.pid })); if (ok === false) toast('That console is in another window; switch to it from the taskbar.'); });
action('open-folder', (el) => attempt(() => call('open_folder', { path: el.dataset.path })));
action('open-session', (el) => go('session', { profile: el.dataset.profile, id: el.dataset.id, ...(el.dataset.device ? { device: el.dataset.device } : {}) }));
action('resume', (el) => attempt(() => call('resume', { profile: el.dataset.profile, id: el.dataset.id, cwd: el.dataset.cwd }), 'Resumed in a new terminal'));
action('sync-now', () => mutate(() => call('sync', {}), 'Accounts are in step'));
action('save-cap', (el) => { const v = document.getElementById('pf-cap').value.trim(); mutate(() => call('set_cap', { name: el.dataset.name, cap: v === '' ? null : +v }), 'Saved'); });
action('save-email', (el) => mutate(() => call('update', { name: el.dataset.name, email: document.getElementById('pf-email').value }), 'Saved'));
action('save-label', (el) => mutate(() => call('set_label', { name: el.dataset.name, label: document.getElementById('pf-label').value }), 'Renamed'));
action('save-key', (el) => {
  const key = document.getElementById('pf-key').value;
  if (!key.trim()) return toast('Paste the new key first', true);
  attempt(() => call('set_key', { name: el.dataset.name, key }), 'Key replaced').then(() => (document.getElementById('pf-key').value = ''));
});
action('save-endpoint', (el) => mutate(() => call('update', { name: el.dataset.name, base_url: document.getElementById('pf-url').value, model: document.getElementById('pf-model').value }), 'Saved'));
/** Remove an account: from the list's or the account's ⋯ menu, or its Settings tab. */
function removeAccount(name, main) {
  const shown = acctName(name);
  const body = main
    ? `Harness Manager stops managing <b>${esc(shown)}</b>. It is the tool's own profile: its folder, login and sessions are never deleted, and <i>Find tools</i> in Settings adds it back.`
    : `Harness Manager forgets <b>${esc(shown)}</b> and its saved key. Its sessions stay on disk unless you also delete its folder.`;
  openDialog(`${dlgHead((main ? 'Hide ' : 'Remove ') + shown)}<div class="dlg-text">${body}</div>${main ? '' : '<label class="check-row"><input type="checkbox" id="rm-home" data-testid="remove-home"><span>Also delete its folder</span></label>'}
    <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-secondary negT" id="rm-ok" data-testid="remove-confirm">${main ? 'Hide profile' : 'Remove'}</button></span></div>`, { size: 'sm', label: 'Remove account' });
  document.getElementById('rm-ok').onclick = async () => {
    const del = !main && document.getElementById('rm-home').checked;
    closeDialog();
    if (await attempt(() => call('remove', { name, delete_home: del }), `Removed ${shown}`)) { go('accounts'); changed(); }
  };
}
action('remove-account', (el) => removeAccount(el.dataset.name, el.dataset.main === '1'));
action('account-menu', (el) => {
  const p = (S.state.profiles || []).find((x) => x.name === el.dataset.name) || {};
  openMenu(el, [
    { label: 'Start in a folder…', run: () => startDialog(el.dataset.name) },
    ...(p.kind === 'subscription' ? [{ label: 'Log in again', run: () => startLogin(el.dataset.name) }] : []),
    { label: 'Open folder', run: () => call('open_folder', { path: el.dataset.home }) },
    { label: 'Profile settings', run: () => go('account', { name: el.dataset.name, tab: 'settings' }) },
    ...(p.harness === 'claude' && !p.main ? [{ label: 'Sync skills and MCP servers from the main profile now', run: () => mutate(() => call('sync', {}), 'Accounts are in step') }] : []),
    ...((p.running || []).length ? [{ label: 'Show the console window', run: () => call('show_window', { pid: p.running[0].pid }) }] : []),
    { label: p.main ? 'Hide main profile (keep folder and login)' : 'Remove account…', danger: true, run: () => removeAccount(el.dataset.name, el.dataset.main === '1') },
  ]);
});
action('login', (el) => startLogin(el.dataset.name));
// a new dialog starts from a Claude Code subscription, not from the last one's choices
action('add-account', () => { add.harness = 'claude'; add.kind = 'subscription'; addDialog(); });
action('add-account-for', (el) => { add.harness = el.dataset.h; add.kind = 'subscription'; addDialog(); });
action('quota-live', (el) => mutate(() => call('quota_live', { name: el.dataset.name }), 'Limits read'));
action('refresh-now', async () => {
  const r = await attempt(() => call('refresh', { live: S.state && S.state.live_quota }));
  if (r) { toast(refreshText(r)); changed(); }
});

// ---------- add account ----------
const add = { harness: 'claude', kind: 'subscription' };
const PREFIX = { claude: '.claude-', codex: '.codex-', opencode: '.opencode-', agy: '.agy-', pi: '.pi-', omp: '.omp-' };
const KIND_TEXT = { subscription: { claude: 'Pro, Max, Team or Enterprise. You sign in in the browser.', codex: 'ChatGPT Plus, Pro or Team. You sign in in the browser.', opencode: 'OpenCode signs in to its providers in a terminal.', agy: 'Google sign-in. Antigravity keeps one login at a time; Start swaps it.', pi: 'Pi signs in to a provider with /login inside the tool.', omp: 'Oh My Pi signs in to a provider with /login inside the tool.' }, api_key: 'Pay per use. The key is kept in Windows Credential Manager.', routed: 'Sent through the local router to a route of providers.' };
const supports = (h) => [['subscription', 'Subscription', true], ['api_key', 'API key', !!h.caps.api_key], ['routed', 'Routed', !!h.caps.routable]];

function addDialog() {
  const hs = shownTools(S.state), h = hs.find((x) => x.id === add.harness) || hs[0];
  const kinds = supports(h);
  if (!kinds.find((k) => k[0] === add.kind && k[2])) add.kind = 'subscription';
  const keep = (id) => { const e = document.getElementById(id); return e ? e.value : ''; };
  const v = { name: keep('ad-name'), email: keep('ad-email'), key: keep('ad-key'), url: keep('ad-url'), model: keep('ad-model'), route: keep('ad-route') };
  const routes = S.state.routes || [], prefix = PREFIX[h.id] || '.' + h.id + '-';
  const cliOf = (v) => `hrm add ${v.name || '<name>'}${h.id !== 'claude' ? ' --harness ' + h.id : ''}${add.kind === 'api_key' ? ' --api-key' : ''}${add.kind === 'routed' ? ' --route ' + (v.route || '<route>') : ''}${v.email ? ' --email ' + v.email : ''}`;
  const version = (id) => (S.appVersions || {})[id];
  openDialog(`${dlgHead('Add account')}
    <div class="form-field"><label>Tool</label><div class="tool-pick-grid" role="group" aria-label="Tool">${hs.map((t) => `<button type="button" class="tool-pick-card ${t.id === h.id ? 'active' : ''}" aria-pressed="${t.id === h.id}" data-act="add-tool" data-h="${t.id}" data-testid="add-tool-${t.id}"><div class="tool-pick-head"><span class="tool-pick-name">${toolIcon(t.id, 14)} ${esc(t.label)}</span><span class="plan-pill">${t.installed ? (version(t.id) ? 'v' + esc(version(t.id)) : 'Installed') : 'Prepare'}</span></div><div class="tool-pick-sup">Supports: <b>${supports(t).filter((k) => k[2]).map((k) => k[1]).join(' · ')}</b></div></button>`).join('')}</div></div>
    <div class="form-field"><label>Account kind for ${esc(h.label)}</label><div class="kind-pick-grid" role="group" aria-label="Account kind">${kinds.map(([k, l, ok]) => `<button type="button" class="kind-pick-card ${k === add.kind ? 'active' : ''} ${ok ? '' : 'disabled'}" aria-pressed="${k === add.kind}" data-act="add-kind" data-kind="${k}" ${ok ? '' : 'disabled'} data-testid="kind-${k}"><div class="kind-pick-head"><b>${l}</b><span class="plan-pill ${ok ? 'ac' : 'neg'}">${ok ? 'Supported ✓' : 'Unsupported'}</span></div><div class="tool-pick-sup">${ok ? esc(k === 'subscription' ? KIND_TEXT.subscription[h.id] || 'Sign in with the tool.' : KIND_TEXT[k]) : `${esc(h.label)} takes ${k === 'api_key' ? 'no API key' : 'no route'}.`}</div></button>`).join('')}</div></div>
    <div class="form-2col"><div class="form-field"><label for="ad-name">Name <span class="opt">(creates folder <span class="mono folder-preview" id="ad-folder">~\\${esc(prefix)}${esc(v.name || '<name>')}</span>)</span></label><input id="ad-name" value="${esc(v.name)}" placeholder="work-design" data-testid="add-name"></div>
      ${add.kind === 'subscription' ? `<div class="form-field"><label for="ad-email">Expected email <span class="opt">(optional · checked after sign-in)</span></label><input id="ad-email" type="email" value="${esc(v.email)}" placeholder="you@company.com" data-testid="add-email"></div>` : add.kind === 'api_key' ? `<div class="form-field"><label for="ad-key">API key</label><input id="ad-key" type="password" value="${esc(v.key)}" placeholder="sk-…" data-testid="add-key"></div>`
        : `<div class="form-field"><label for="ad-route">Route</label>${routes.length ? `<select id="ad-route" data-testid="add-route">${routes.map((r) => `<option ${r.id === v.route ? 'selected' : ''}>${esc(r.id)}</option>`).join('')}</select>` : '<span class="muted">No routes yet. Create one under API keys &amp; routing.</span>'}</div>`}</div>
    ${add.kind !== 'subscription' ? `<div class="form-2col">${add.kind === 'api_key' ? `<div class="form-field"><label for="ad-url">Endpoint <span class="opt">(optional)</span></label><input id="ad-url" value="${esc(v.url)}" placeholder="The vendor's own API when empty"></div>` : ''}<div class="form-field"><label for="ad-model">Model <span class="opt">(optional)</span></label><input id="ad-model" value="${esc(v.model)}" placeholder="${add.kind === 'routed' ? 'e.g. hm-openrouter--z-ai/glm-5' : 'The vendor default when empty'}"></div></div>` : ''}
    ${h.id === 'claude' && add.kind === 'subscription' ? `<div class="sync-facts-grid"><div><b class="ac">Kept in sync from the main profile</b><span>CLAUDE.md, rules, skills, plugins and MCP servers</span></div><div><b>Its own</b><span>Login, history and usage, so each account's numbers stay separate</span></div></div>` : ''}
    <div class="cli-box"><span class="lbl">CLI equivalent</span><code class="mono clibox" id="ad-cli" data-testid="add-cli">${esc(cliOf(v))}</code></div>
    <div class="modal-foot"><span><span class="kbd">Tab</span> next field · <span class="kbd">Esc</span> cancel</span><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" id="ad-create" data-testid="add-create">${add.kind === 'subscription' ? 'Create and log in' : 'Create'}</button></span></div>`, { label: 'Add account' });
  const card = document.querySelector('.modal-card');
  card.addEventListener('input', () => {
    document.getElementById('ad-folder').textContent = '~\\' + prefix + (keep('ad-name') || '<name>');
    document.getElementById('ad-cli').textContent = cliOf({ name: keep('ad-name'), email: keep('ad-email'), route: keep('ad-route') });
  });
  document.getElementById('ad-name').focus();
  // validation stays inside the dialog, in its own strip; the page's notice bar is not for this
  const fail = (msg, id) => { card.querySelector('.notice-bar')?.remove(); card.querySelector('.modal-head').insertAdjacentHTML('afterend', `<div class="notice-bar warn" role="alert">${esc(msg)}</div>`); document.getElementById(id).focus(); };
  document.getElementById('ad-create').onclick = async () => {
    const name = document.getElementById('ad-name').value.trim();
    if (!/^[a-z0-9_-]+$/i.test(name)) return fail('Use a name of letters, numbers, hyphens or underscores.', 'ad-name');
    if (add.kind === 'api_key' && !keep('ad-key').trim()) return fail('Paste the API key first.', 'ad-key');
    const req = { name, harness: add.harness, kind: add.kind, email: keep('ad-email') || null, api_key: keep('ad-key') || null, base_url: keep('ad-url') || null, model: keep('ad-model') || null, route: keep('ad-route') || null };
    const created = await attempt(() => call('add', req));
    if (!created) return;
    closeDialog();
    await changed();
    if (add.kind !== 'subscription') return toast(`Added ${req.name}`);
    startLogin(req.name);
  };
}
action('add-tool', (el) => { add.harness = el.dataset.h; addDialog(); });
action('add-kind', (el) => { if (!el.disabled) { add.kind = el.dataset.kind; addDialog(); } });

/** Sign in: Claude in the dialog (link plus pasted code); other tools in a terminal window. */
async function startLogin(name) {
  const p = (S.state.profiles || []).find((x) => x.name === name) || { harness: add.harness };
  if (p.harness !== 'claude') {
    if (!(await attempt(() => call('login_terminal', { name })))) return;
    // the dialog watches the account's login every 2 s and says when the terminal's sign-in has landed
    const before = JSON.stringify(p.identity || null);
    const body = (msg, ok) => `${dlgHead('Sign in ' + acctName(name))}<div class="dlg-text ${ok ? 'ok' : ''}" data-testid="login-status">${msg}</div>
      <div class="modal-foot"><span class="acts"><button class="${ok ? 'btn-primary' : 'btn-secondary'}" data-act="login-done" data-testid="login-done">${ok ? 'Done' : 'Close'}</button></span></div>`;
    let timer;
    openDialog(body('A terminal window opened with the sign-in. Finish it there; this dialog follows along.', false), { onClose: () => clearInterval(timer), size: 'sm', label: 'Sign in' });
    timer = setInterval(async () => {
      const st = await call('state').catch(() => null);
      const now = st && st.profiles.find((x) => x.name === name);
      if (!now || JSON.stringify(now.identity) === before || !now.identity.logged_in) return;
      clearInterval(timer);
      S.state = st;
      const dlg = document.querySelector('.modal-card');
      if (dlg) dlg.innerHTML = body(`Signed in${now.identity.email ? ' as ' + esc(now.identity.email) : ''}.`, true);
    }, 2000);
    return;
  }
  const flow = await attempt(() => call('login_start', { name }));
  if (!flow) return;
  let timer;
  const draw = (st) => {
    const dlg = document.querySelector('.modal-card');
    if (!dlg) return;
    dlg.innerHTML = `${dlgHead('Sign in ' + acctName(name))}
      <div class="notice-bar ${st.state === 'failed' || (st.state === 'done' && st.email_mismatch) ? 'warn' : ''}" role="status" data-testid="login-status">${esc(st.message)}</div>
      ${st.url && st.state === 'waiting' ? `<div class="form-field"><label>Link</label><div><button class="btn-secondary" id="lg-open">Open the sign-in page</button></div><span class="url mono">${esc(st.url)}</span></div>
        <div class="form-field"><label for="lg-code">Code or callback URL</label><div class="field-row"><input id="lg-code" placeholder="Paste the code or callback URL" data-testid="login-code"><button class="btn-secondary" id="lg-send" data-testid="login-send">Send</button></div></div>` : ''}
      <div class="modal-foot"><span class="acts">${st.state === 'waiting' ? `<button class="btn-secondary" id="lg-cancel">Cancel sign-in</button>` : `<button class="btn-primary" data-act="login-done" data-testid="login-done">Done</button>`}</span></div>`;
    const o = document.getElementById('lg-open');
    if (o) o.onclick = () => call('open_url', { url: st.url });
    const send = document.getElementById('lg-send');
    if (send) send.onclick = () => attempt(() => call('login_submit', { id: flow.id, code: document.getElementById('lg-code').value }), 'Code sent');
    const c = document.getElementById('lg-cancel');
    if (c) c.onclick = () => closeDialog();
  };
  // closed in any way (Cancel, X, Escape, outside click, Done): stop the sign-in process and forget the flow
  let closed = false;
  openDialog('', { onClose: () => { closed = true; clearInterval(timer); call('login_cancel', { id: flow.id }).catch(() => {}); }, size: 'sm', label: 'Sign in' });
  draw(flow);
  document.getElementById('lg-code')?.focus();
  timer = setInterval(async () => {
    try {
      const st = await call('login_poll', { id: flow.id });
      if (closed) return;
      if (st.state !== 'waiting') clearInterval(timer);
      const typing = document.activeElement && document.activeElement.id === 'lg-code';
      if (!typing || st.state !== 'waiting') draw(st);
    } catch (e) {
      clearInterval(timer);
      if (!closed) draw({ state: 'failed', message: 'Could not check the sign-in: ' + String(e).replace(/^Error: /, '') });
    }
  }, 1000);
}
action('login-done', () => { closeDialog(); changed(); });
