// Limits, with no page behind them: the account as every page draws it, the one rule for a recommendation, the week
// ring and the tray popup's rings. The Overview, Accounts and the tray popup (popup.js) share these; nothing here
// imports a page, so the popup's small window loads no page's timers or listeners.

import { S, call, shownTools, patch } from './core.js';
import { esc, toolIcon } from './lib.js';

// sync is on with a folder or a hub, or with paired computers: computers.js's rule, said again here because that module
// carries the Computers dialogs and their timers
const syncOn = (c) => !!c && (c.mode !== 'off' || (c.peers || []).length > 0 || !!(S.state && S.state.settings && S.state.settings.direct_connection));

export const TOOL = { claude: 'Claude Code', codex: 'Codex', opencode: 'OpenCode', agy: 'Antigravity', pi: 'Pi', omp: 'Oh My Pi' };
/** The tools whose plans have windows to read; the others show usage only. */
export const PLAN_TOOLS = ['claude', 'codex', 'agy'];
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

// ---------- the tray popup: every account's ring, in the tray's own window (limits.html) ----------
export function trayHtml(accts) {
  const { bestRoomAcct: best } = metricsOf(accts), b5 = best && w5of(best);
  const line = (a) => { const wk = a.measured && wkof(a); return wk && wk.open ? `↻ ${fmtDuration(wk.resetMs)}` : a.cap ? `Daily cap · ${money(a.cap.limitUsd - a.cap.spentUsd)} left` : a.kind === 'api_key' ? 'Pay per use' : a.needsLogin ? '— sign in to read' : a.noPlan ? 'No plan limits' : '— unread'; };
  return `<aside class="tray-popup" role="dialog" aria-label="Limits of every account" tabindex="-1">
    <div class="tray-head"><span class="tray-title"><span class="brand-logo" id="tray-mark"></span>Limits · ${accts.length} ${accts.length === 1 ? 'account' : 'accounts'}</span></div>
    <div class="tray-next">${best ? `Use next: <b>${esc(best.name)}</b>${b5 && b5.open ? ` (${b5.left}% 5h · ↻ ${fmtDuration(b5.resetMs)})` : ''}` : accts.length ? '— Limits not read; no recommendation' : 'No accounts configured'}</div>
    ${accts.length ? `<div class="tray-rings-grid" data-testid="nav-limits">${accts.map((a) => `<button class="tray-ring-card" data-act="tray-open" data-name="${esc(a.id)}" data-key="${esc(a.id)}" title="Inspect ${esc(a.name)}" data-testid="nav-limit-${esc(a.id)}">${renderRingSvg(a)}<span class="who"><span class="name">${toolIcon(a.tool, 14)}<b>${esc(a.name)}</b></span><span class="sub">${esc(line(a))}</span></span></button>`).join('')}</div>` : '<div class="tray-empty">No accounts configured. Add one on Accounts.</div>'}
  </aside>`;
}
/** Draw the popup window's content (limits.html) from the state read; the mark is drawn by the caller. */
export async function drawTray(root) {
  // the rings first, from the state already read; the forecasts patch in when their read returns (seconds, on a busy database)
  const html = (x) => trayHtml(accountViews(S.state, x, Date.now(), S.cache.computers));
  if (root.innerHTML) patch(root, html(S.cache.extra || {})); else root.innerHTML = html(S.cache.extra || {});
  const x = await call('limits_extra').catch(() => null);
  if (x) { S.cache.extra = x; patch(root, html(x)); }
}
