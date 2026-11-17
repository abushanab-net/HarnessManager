// Accounts: one card per tool with a 56 px row per account, the charts folded under them; Account detail with its four tabs;
// Add account with sign-in; Start in a folder; the account menu. The account adapter, the one eligibility rule, the ring
// renderer and the tray popup's rings are limits.js's (the Overview and the popup share them); their names are still exported here.

import { S, call, view, go, toast, attempt, mutate, changed, acctName, refreshText, shownTools, openDialog, closeDialog, dlgHead, openMenu, action, loadState, render, pickFolder, saveFile, patch, keyStore, rectOf, windowSize } from './core.js';
import { esc, fmt, hm, when, dur, toolIcon } from './lib.js';
import { TOOL, PLAN_TOOLS, money, fmtDuration, fmtAbsoluteReset, showUsed, acctView, accountViews, eligible, w5of, wkof, metricsOf, winTrack } from './limits.js';
import './sessions.js'; // nothing of it is used here: kept so the window's modules still load in the order they did
import { syncOn, nameOf } from './computers.js';
import { pageHead, detailHead, seg, needsAttention, plural } from './parts.js';

// the names other modules import from here, as before limits.js held them
export { TOOL, money, fmtDuration, fmtAbsoluteReset, showUsed, acctView, accountViews, eligible, w5of, wkof, metricsOf, renderRingSvg, winTrack, trayHtml, drawTray } from './limits.js';
const agoText = (t, now) => (t == null ? 'never' : now - t < 60000 ? 'just now' : dur(now - t) + ' ago');
// ---------- the Accounts page (1.10, Option A: docs/project-review-2026-11/mockups/a-accounts.html) ----------
// usage drawn into the rows: the range, each account's breakdown and day series, and the colour it has in the chart
const au = { days: 7, by: {}, series: {}, color: {}, models: [], days_list: null };
let acctTool = '';
const folded = new Set();
// the two charts under the groups are one panel, folded until asked for; the choice is remembered on this computer
const CHARTS_KEY = 'hm.accounts.charts';
let chartsOpen = false;
try { chartsOpen = localStorage.getItem(CHARTS_KEY) === '1'; } catch {}
const SERIES = 8;
/** fmt() without the zeros it pads with: "168M", "285.5M", "9.5M". */
const fmtTrim = (n) => fmt(n).replace(/(\.\d*?)0+(?=\D*$)/, '$1').replace(/\.(?=\D*$)/, '');
/** A cost at list prices: whole dollars from $100, cents under it. */
const usd = (c) => money(c, c >= 100 ? 0 : 2);
/** The range's tokens per day as a line, scaled to its own peak (a shape, not a comparison). */
function sparkLine(vals) {
  const v = vals.length > 1 ? vals : [vals[0] || 0, vals[0] || 0], m = Math.max(1, ...v), w = 96, h = 20;
  return `<svg width="${w}" height="${h}" role="img" aria-label="Tokens per day, last ${au.days} days"><polyline points="${v.map((x, i) => `${((i * w) / (v.length - 1)).toFixed(1)},${(h - 2 - (x / m) * (h - 4)).toFixed(1)}`).join(' ')}" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round"/></svg>`;
}

/** The 44 px ring of a row: the week (or the daily cap) left, or used by the setting, with the number inside at 12 px;
 * dashed with "—" when there is nothing read, in the failed colour when the login is the reason. `note` ends its title. */
function rowRing(a, used, note) {
  const r = 19, c = 2 * Math.PI * r, circle = (cls, extra) => `<circle class="${cls}" cx="22" cy="22" r="${r}" fill="none" stroke-width="3" ${extra}/>`;
  const draw = (title, pct, tone) => `<span class="a-ring ${tone}" title="${esc(title + note)}"><svg width="44" height="44" viewBox="0 0 44 44" aria-hidden="true">${circle('track', pct == null ? 'stroke-dasharray="3 3"' : '')}${pct == null ? '' : circle('val', `stroke-linecap="round" stroke-dasharray="${((c * pct) / 100).toFixed(1)} ${c.toFixed(1)}" transform="rotate(-90 22 22)"`)}</svg><b>${pct == null ? '—' : pct + '%'}</b></span>`;
  if (a.cap) {
    const left = Math.max(0, Math.round(100 - (a.cap.spentUsd / a.cap.limitUsd) * 100));
    return draw(`${a.name}: daily cap ${money(a.cap.limitUsd - a.cap.spentUsd)} left of ${money(a.cap.limitUsd)}`, used ? 100 - left : left, left === 0 ? 'neg' : left <= 30 ? 'warn' : 'r2');
  }
  if (a.needsLogin) return draw('Log in to read its limits', null, 'none');
  if (!a.measured) return draw(a.kind === 'api_key' ? 'API key: no plan limits' : a.noPlan ? 'No plan limits' : 'Limits not read', null, '');
  const wk = wkof(a);
  return draw(`${a.name}: ${wk.label.toLowerCase()} ${wk.left}% left (${wk.used}% used)${wk.open ? ', resets ' + fmtAbsoluteReset(wk.resetsAt) : ''}`, used ? wk.used : wk.left, wk.left === 0 ? 'neg' : wk.left <= 30 || wk.outMs != null ? 'warn' : '');
}

/** A row's limits in ONE line: a short label and the % over a bar, three at most (the rest behind "+N"); a long label is
 * cut with … and every bar says the whole of it in its title. An account with nothing to draw says why in their place. */
function limsLine(a, best, used, now) {
  const out = a.windows.find((w) => w.outAt);
  const advice = best && a.id === best.id ? 'Most room of any account: start here next.' : out ? `${out.label} runs out ≈ ${fmtAbsoluteReset(out.outAt, now)} at this pace.` : '';
  const lim = (label, title, pct, tone, zero, cls = '') => `<div class="lim ${cls}" title="${esc(title)}"><div class="t"><span>${esc(label)}</span><b class="${zero ? 'neg' : ''}">${pct}%</b></div>${winTrack(pct, tone)}</div>`;
  if (a.cap) {
    const left = Math.max(0, Math.round(100 - (a.cap.spentUsd / a.cap.limitUsd) * 100));
    return `<div class="lims">${lim(`${money(a.cap.limitUsd)} daily cap`, `Daily spend cap: ${money(a.cap.limitUsd - a.cap.spentUsd)} left of ${money(a.cap.limitUsd)} (${money(a.cap.spentUsd)} spent today), resets in ${fmtDuration(a.cap.resetMs)}`, used ? 100 - left : left, left === 0 ? 'neg' : left <= 30 ? 'warn' : 'r2', left === 0, 'cap')}</div>`;
  }
  if (a.needsLogin || !a.windows.length) {
    const [text, why, bad] = a.needsLogin ? [a.p.identity.logged_in ? 'Login expired' : 'Not logged in', 'Log in to read its plan and limits.', true]
      : a.kind === 'api_key' ? ['No plan limits · pay per use', 'An API key is billed per token and has no plan limits.']
      : a.kind === 'routed' ? ['No plan limits · no daily cap', 'A routed account has no plan limits. Daily spend cap… in the ⋯ menu sets the most it may spend in a day.']
      : a.noPlan ? ['No plan limits', `${TOOL[a.tool] || a.tool} has no plan limits to read: usage only.`]
      : ['Limits not read', 'Limits show up after this account runs on this computer. Refresh limits in the ⋯ menu reads them now.'];
    return `<div class="unread trunc" title="${esc(why)}"><span class="${bad ? 'neg' : ''}">— ${esc(text)}</span></div>`;
  }
  const title = (w) => `${w.label}: ${w.left}% left (${w.used}% used)${w.open ? ', resets ' + fmtAbsoluteReset(w.resetsAt, now) + ' (in ' + fmtDuration(w.resetMs) + ')' : w.resetsAt ? ', window has reset' : ''}`;
  const rest = a.windows.slice(3);
  return `<div class="lims ${rest.length ? 'more' : ''}" ${best && a.id === best.id ? 'data-testid="best-note"' : ''} title="${esc(advice)}">${a.windows.slice(0, 3).map((w) => lim(w.kind === '5h' ? '5h' : w.kind === 'week' ? 'Wk' : w.label.split(' · ').pop(), title(w) + (advice ? '\n' + advice : ''), used ? w.used : w.left, w.left === 0 ? 'neg' : w.left <= 30 || w.outMs != null ? 'warn' : '', w.left === 0)).join('')}${rest.length ? `<span class="lim-more" title="${esc(rest.map(title).join('\n'))}">+${rest.length}</span>` : ''}</div>`;
}

/** One account, 56 px: ring, name and login, plan, limits, the range's sparkline, tokens and ≈ cost, ONE button for its state, ⋯. */
function accountRow(a, best, now, used, pcs) {
  const p = a.p, b = au.by[a.id], vals = au.series[a.id] || [];
  // under the name: the login; without one, what kind of account it is (a missing login is said once, in the limits' place)
  const under = a.email || p.identity.name || p.base_url || (a.kind === 'api_key' ? 'Pay per use' : a.kind === 'routed' ? `Through route ${a.route || ''}` : a.noPlan ? 'Usage only' : p.identity.logged_in ? 'Logged in' : '');
  const plan = a.kind !== 'subscription' ? a.plan : a.needsLogin ? '' : p.identity.plan || '';
  const openNow = a.openSessions ? plural(a.openSessions, 'session') + ' open' + (a.far.length ? ' · ' + [...new Set(a.far.map((r) => nameOf(pcs, r.device)))].join(', ') : '') : '';
  return `<div class="acct-row" tabindex="0" role="button" data-act="open-account" data-name="${esc(a.id)}" data-key="${esc(a.id)}" data-testid="row-${esc(a.id)}">
    ${rowRing(a, used, ` · limits read ${agoText(a.readAt, now)}`)}
    <div class="who" title="${esc([a.folder, a.added ? 'added ' + a.added : ''].filter(Boolean).join(' · '))}"><div class="row"><b class="trunc">${esc(a.name)}</b>${openNow ? `<span class="dot live" role="img" title="${esc(openNow)}" aria-label="${esc(openNow)}"></span>` : ''}${a.isMain ? `<span class="pill quiet" title="Main tool profile (${esc(a.folder)}): can be hidden, not deleted">main</span>` : ''}${a.mismatch ? '<span class="pill bad" title="Logged in as a different email than you entered">email differs</span>' : ''}</div>${under ? `<div class="small muted trunc">${esc(under)}</div>` : ''}</div>
    <div>${plan ? `<span class="pill plan" title="${esc(plan)}">${esc(plan)}</span>` : `<span class="unread" title="${a.needsLogin ? 'Log in to read its plan' : 'No plan'}">—</span>`}</div>
    ${limsLine(a, best, used, now)}
    <div class="spark" title="${esc(au.days + ' days: ' + (vals.some(Boolean) ? vals.map((v) => fmt(v)).join(' · ') : 'no usage'))}">${sparkLine(vals)}<span>${au.days} days</span></div>
    <div class="use" title="${b ? esc(`input ${fmt(b.input)} · output ${fmt(b.output)} · cache read ${fmt(b.cache_read)} · ${b.requests.toLocaleString()} requests · ${Math.round(b.cache_hit_rate * 100)}% from cache · cost at API list prices`) : 'no usage in this range'}"><div><b>${fmtTrim(b ? b.tokens : 0)}</b> <span class="small muted">tokens</span></div><div class="small muted" data-testid="cost-${esc(a.id)}">${b && b.cost != null ? '≈ ' + usd(b.cost) : '—'}</div></div>
    <div class="acts">${a.needsLogin ? `<button class="btn sm login" data-act="login" data-name="${esc(a.id)}" title="${p.identity.logged_in ? 'Its login has expired' : 'Not logged in yet'}: sign in to start it" data-testid="login-${esc(a.id)}">Log in</button>` : `<button class="btn sm" data-act="start" data-name="${esc(a.id)}" data-testid="start-${esc(a.id)}">Start</button>`}<button class="btn sm icon" data-act="account-menu" data-name="${esc(a.id)}" data-home="${esc(a.folder)}" data-main="${a.isMain ? 1 : ''}" title="More actions" aria-label="More actions for ${esc(a.name)}" data-testid="menu-${esc(a.id)}">⋯</button></div></div>`;
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
  const [st, u, x] = await Promise.all([loadState(), call('usage', { days: au.days }).catch(() => null), call('limits_extra').catch(() => ({}))]); S.cache.extra = x;
  if (u) {
    au.by = Object.fromEntries(u.accounts.map((b) => [b.key, b]));
    au.series = u.summary.by_profile || {};
    au.models = u.models || [];
    au.color = Object.fromEntries(u.accounts.map((b, i) => [b.key, i % SERIES]));
    au.days_list = u.summary.days;
  }
  const now = st.now, pcs = S.cache.computers, used = showUsed();
  const accts = accountViews(st, x, now, pcs);
  // said once, then marked as told (not removed: the first-run dialog in computers.js reads the same report)
  if (st.first_run && !st.first_run.told && (st.first_run.main_profiles.length || st.first_run.accounts.length)) { st.first_run.told = true; toast(`Found ${st.first_run.main_profiles.length} installed tools and imported ${st.first_run.accounts.length} accounts from your PowerShell launcher.`); }
  // Start in a folder… opens on the account with the most room; with no limits read, on the first that can start
  const best = metricsOf(accts).bestRoomAcct, first = best || accts.find((a) => !a.needsLogin);
  const q = (document.getElementById('find').value || '').toLowerCase();
  const shown = accts.filter((a) => !q || a.id.toLowerCase().includes(q) || a.name.toLowerCase().includes(q) || a.email.toLowerCase().includes(q));
  const tools = shownTools(st), groups = tools.filter((h) => !acctTool || h.id === acctTool);
  const series = accts.filter((a) => au.series[a.id] && au.series[a.id].some((v) => v)).map((a) => ({ name: a.name, ci: au.color[a.id] || 0, vals: au.series[a.id] }));
  const topModel = au.models[0] ? au.models[0].tokens : 1;
  const nTools = new Set(accts.map((a) => a.tool)).size, nAtt = accts.filter((a) => needsAttention(a.p, now)).length;
  const status = accts.length ? `${plural(accts.length, 'account')} on ${plural(nTools, 'tool')}; ${nAtt ? `${nAtt} ${nAtt === 1 ? 'needs' : 'need'} attention` : 'none needs attention'}.` : 'No account yet.';
  const chip = (v, label, n, testid) => `<button type="button" class="${acctTool === v ? 'on' : ''}" aria-pressed="${acctTool === v}" data-act="acct-tool" data-v="${v}" data-testid="${testid}">${label} <span class="${v ? 'dim' : 'muted'}">${n}</span></button>`;
  patch(sheet, `<div class="page-body">
    ${pageHead('accounts', status, `<button class="btn" data-act="acct-csv" data-testid="u-csv">Export CSV</button>${first ? `<button class="btn" data-act="start-in-folder" data-name="${esc(first.id)}" data-testid="start-best">Start in a folder…</button>` : ''}<button class="btn primary" data-act="add-account" data-testid="add-account">Add account</button>`)}
    <div class="a-filters"><div class="seg" role="group" aria-label="Tool">${chip('', 'All', accts.length, 'chip-all')}${tools.map((h) => chip(h.id, toolIcon(h.id, 14) + esc(h.label), accts.filter((a) => a.tool === h.id).length, 'chip-' + h.id)).join('')}</div>
      ${used ? '<span class="small muted" data-testid="used-note">Rings and bars show % used</span>' : ''}${seg('Range', [[7, '7 d'], [14, '14 d'], [30, '30 d']], au.days, (d) => `data-act="acct-days" data-v="${d}" data-testid="u-days-${d}"`)}</div>
    <div class="a-groups" data-testid="accounts-table">${groups.map((h) => {
      const list = shown.filter((a) => a.tool === h.id).sort((a, b) => (eligible(b) - eligible(a)) || (b.measured - a.measured) || ((b.measured && a.measured) ? (wkof(b).left - wkof(a).left) : 0));
      const open = !folded.has(h.id), on = list.filter((a) => a.p.identity.logged_in).length, consoles = list.reduce((s, a) => s + a.openSessions, 0);
      const next = Math.min(...list.flatMap((a) => a.windows.filter((w) => w.open).map((w) => w.resetMs)).concat([Infinity]));
      const tok = list.reduce((s, a) => s + (au.by[a.id] ? au.by[a.id].tokens : 0), 0), cst = list.some((a) => au.by[a.id] && au.by[a.id].cost != null) ? list.reduce((s, a) => s + ((au.by[a.id] || {}).cost || 0), 0) : null;
      return `<section class="card tool-group-card" data-key="g-${h.id}" data-testid="group-${h.id}"><button class="tool-group-header" data-act="acct-fold" data-h="${h.id}" aria-expanded="${open}" title="${open ? 'Hide' : 'Show'} the ${esc(h.label)} accounts">
        ${toolIcon(h.id, 18)}<h2>${esc(h.label)}</h2>${list.length ? `<span class="muted" title="${esc(`${on} logged in · ${plural(consoles, 'console')} open`)}">${list.length}</span>` : ''}<span class="chev" aria-hidden="true"></span>
        ${list.length ? `<span class="sum"><span ${isFinite(next) ? '' : 'title="No limit window is open"'}>${isFinite(next) ? 'Next reset in ' + fmtDuration(next) : list.some((a) => !a.noPlan) ? 'Next reset —' : 'No plan limits'}</span><span><b>${fmtTrim(tok)}</b> tokens · ${cst == null ? '—' : '≈ ' + usd(cst)} in ${au.days} days</span></span>` : ''}</button>
        ${!open ? '' : list.length ? list.map((a) => accountRow(a, best, now, used, pcs)).join('') : `<div class="group-empty">${h.installed ? (q ? 'No account matches the search.' : `No ${esc(h.label)} account yet.`) : `${esc(h.label)} is not installed on this computer.`}</div>`}</section>`;
    }).join('')}</div>
    <section class="panel-card a-usage" data-testid="usage-panel"><div class="panel-head"><button class="panel-title fold" data-act="acct-fold" data-h="charts" aria-expanded="${chartsOpen}">Tokens per day, by account, and by model · ${au.days} days</button><span class="panel-note">Cost at API list prices ≈</span></div>
      ${chartsOpen ? `<div class="a-charts"><div>${series.length && au.days_list ? `<div class="chart-legend">${series.map((s) => `<span><i class="pc-dot c${s.ci}"></i>${esc(s.name)}</span>`).join('')}</div><div data-testid="usage-ledger">${stackedChart(au.days_list, series, true)}</div>` : '<span class="muted" data-testid="usage-ledger">No usage in this range.</span>'}</div>
        <div data-testid="models-panel"><h3 class="label">By model</h3><div class="model-rows">${au.models.map((m) => `<div class="win-bar-line"><span class="mono trunc" title="${esc(m.key)}">${esc(m.key)}</span>${winTrack(Math.max(4, Math.round((m.tokens / topModel) * 100)))}<span>${fmt(m.tokens)}</span><span class="r">${m.cost == null ? '—' : '≈ ' + money(m.cost)}</span></div>`).join('') || '<span class="muted">No usage yet.</span>'}</div></div></div>` : ''}</section></div>`);
});
action('acct-days', (el) => { au.days = +el.dataset.v; render(); });
action('acct-csv', async () => {
  const to = await saveFile(`Harness Manager usage ${new Date().toISOString().slice(0, 10)}.csv`, { name: 'CSV', extensions: ['csv'] });
  if (to) attempt(() => call('usage_csv', { days: au.days, path: to }), 'Exported');
});
action('acct-tool', (el) => { acctTool = el.dataset.v; render(); });
action('acct-fold', (el) => {
  const h = el.dataset.h;
  if (h === 'charts') { chartsOpen = !chartsOpen; try { localStorage.setItem(CHARTS_KEY, chartsOpen ? '1' : '0'); } catch {} }
  else if (folded.has(h)) folded.delete(h); else folded.add(h);
  render();
});
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
    // nothing read: the Limits card says why, with the action that fixes it, in one line across the page (no empty box beside the login)
    const unread = a.needsLogin ? [`— Not read: ${p.identity.logged_in ? 'the login has expired' : 'this account is not logged in'}.`, `<button class="btn" data-act="login" data-name="${esc(name)}" data-testid="limits-fix">Log in</button>`]
      : p.kind === 'routed' ? ['A routed account has no plan limits; a daily spend cap is the most it may spend in a day.', `<a class="btn" href="#account?name=${encodeURIComponent(name)}&tab=settings" data-testid="limits-fix">Daily spend cap…</a>`]
      : p.kind !== 'subscription' ? ['An API key is billed per token and has no plan limits.', '']
      : !PLAN_TOOLS.includes(p.harness) ? [esc(p.harness_label) + ' has no plan limits to read: usage only.', '']
      : ['— Not read yet: limits show up after this account runs on this computer.', `<button class="btn" data-act="limits-refresh" data-name="${esc(name)}" data-testid="limits-fix">Refresh limits</button>`];
    body = `<section class="account-stats" aria-label="Account readouts">${stat('Tokens · 7 days', w ? fmt(w.tokens) : '—', w ? Math.round(w.hit * 100) + '% read from cache' : 'Cache share not read')}${stat('At list prices', w ? money(w.cost) : '—', p.kind === 'subscription' ? 'Subscription: not billed per token' : 'Billed per token')}${stat('First reply p95', w && w.p95 != null ? secs(w.p95) : '—', w && w.replies ? `p50 ${secs(w.p50)} · ${w.replies.toLocaleString()} replies` : 'No timed replies')}${stat('Sessions · 7 days', week.length, running ? `${running} running now` : 'None running now')}${stat('Busiest project', w && w.top_project ? Math.round(w.top_project.share * 100) + '%' : '—', w && w.top_project ? esc(w.top_project.name) : 'No usage this week')}</section>
      <div class="account-columns ${a.windows.length ? '' : 'solo'}"><section class="panel-card" data-testid="limits-card">${a.windows.length ? `<div class="panel-head"><h2 class="panel-title">Limits</h2><span class="muted">read ${agoText(a.readAt, now)}</span></div>
          ${a.windows.map(lim).join('')}<span class="muted">Bars: % ${used ? 'used' : 'left'} · dashed mark: ≈ level at reset.</span>` : `<div class="a-unread"><h2 class="panel-title">Limits</h2><span class="muted grow">${unread[0]}</span>${unread[1]}</div>`}</section>
        <section class="panel-card"><h2 class="panel-title">Login and folder</h2><dl class="facts-grid">${facts.map(([t, v]) => `<dt>${t}</dt><dd>${v}</dd>`).join('')}</dl>${p.kind === 'subscription' && !a.needsLogin ? `<div><button class="btn-secondary" data-act="login" data-name="${esc(name)}">Log in again</button></div>` : ''}</section></div>
      <div class="account-columns"><section class="panel-card"><div class="panel-head"><h2 class="panel-title">Tokens per day · 30 days</h2><span class="muted">${k && per.some(Boolean) ? fmt(per.reduce((s, v) => s + v, 0)) + ' · ' + money(k.cost30) + ' at list prices' : '—'}</span></div>
          ${per.some(Boolean) ? `<div class="account-chart">${per.map((v, i) => `<div class="stack-col ${i === per.length - 1 ? 'today' : ''}" title="${esc(dayLabel(i))}: ${fmt(v)} tokens"><svg class="stack-bars" height="${Math.round((v / max) * 76)}" aria-hidden="true"><rect width="100%" height="100%" rx="2"/></svg><span class="stack-lbl">${i === 0 || i === 14 ? esc(dayLabel(i)) : i === per.length - 1 ? 'Today' : ''}</span></div>`).join('')}</div>` : '<span class="muted">No usage yet.</span>'}</section>
        <section class="panel-card" data-testid="account-models"><h2 class="panel-title">By model · 30 days</h2>${models.length ? `<div class="model-rows">${models.map((m) => `<div class="win-bar-line"><span class="mono trunc" title="${esc(m.key)}">${esc(m.key)}</span>${winTrack(Math.round((m.tokens / topModel) * 100))}<span>${fmt(m.tokens)}</span><span class="r">${m.cost == null ? '—' : money(m.cost)}</span></div>`).join('')}</div>` : '<span class="muted">No usage yet.</span>'}</section></div>
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
      ${p.kind === 'api_key' ? `<section class="panel-card"><h2 class="panel-title">API key</h2>${row(field('pf-key', 'New API key', '', 'password', 'placeholder="Paste a new key"'), 'save-key', 'Replace key')}<span class="muted">Kept in ${esc(keyStore())}, never in the account list.</span>${row(field('pf-url', 'Base URL', p.base_url || '') + field('pf-model', 'Model', p.model || ''), 'save-endpoint')}<span class="muted">Leave empty to use the vendor's own API and default model.</span></section>` : ''}
      ${p.kind === 'routed' ? `<section class="panel-card"><h2 class="panel-title">Daily spend cap</h2>${row(field('pf-cap', 'Cap in USD', p.daily_cap_usd ?? '', 'number', 'min="0" step="0.5" placeholder="no cap" data-testid="cap-input"'), 'save-cap', 'Save', 'cap-save')}<span class="muted">The most the router spends for this account in a day, at API list prices. At the cap it stops answering for it until midnight. Empty: no cap.</span></section>` : ''}
      ${p.harness === 'claude' && !p.main ? `<section class="panel-card"><h2 class="panel-title">Shared with the main profile</h2><span>CLAUDE.md, rules, skills, plugins and MCP servers follow the main profile. Login, history and usage stay with this account.</span><div><button class="btn-secondary" data-act="sync-now">Sync now</button></div></section>` : ''}
      <section class="panel-card"><h2 class="panel-title">${p.main ? 'Hide main profile' : 'Remove account'}</h2><span class="muted">${p.main ? "Stops managing the tool's own profile. Its folder and login are never deleted." : 'Removes it from Harness Manager. Its folder stays unless you choose to delete it.'}</span><div><button class="btn-secondary negT" data-act="remove-account" data-name="${esc(name)}" data-main="${p.main ? 1 : ''}" data-testid="remove-account">${p.main ? 'Hide main profile…' : 'Remove account…'}</button></div></section></div>`;
  }
  const open = p.running.length ? `${p.running.length} ${p.running.length === 1 ? 'console' : 'consoles'} open${p.running[0].cwd ? ' in ' + esc(p.running[0].cwd.split(/[\\/]/).pop()) : ''}` : '';
  // why Start cannot work now, said on the disabled button
  const blocked = a.needsLogin ? (p.identity.logged_in ? 'Its login has expired: log in first' : 'Not logged in: log in first') : !tool.installed ? `${p.harness_label} is not installed on this computer` : '';
  // the account's own name with its tool's icon; under it, one line: the tool, the login, the plan, the organisation, what is open
  const facts = [esc(p.harness_label), a.email ? esc(a.email) : '', `<span class="plan-pill ${a.planTone}">${esc(a.plan)}</span>`, esc([p.identity.org, p.identity.role].filter(Boolean).join(' · ')), p.main ? 'main profile' : '', open].filter(Boolean).join(' · ');
  patch(sheet, `<div class="page-body a-account">${detailHead({ back: ['#accounts', 'Accounts'], title: `${toolIcon(p.harness, 20)}<span class="trunc">${esc(p.label)}</span>`, titleAttrs: 'data-testid="detail-name"', status: facts,
      lead: `<div class="account-avatar" aria-hidden="true">${esc(p.label.split(/[-_ ]/).filter(Boolean).slice(0, 2).map((s) => s[0].toUpperCase()).join(''))}</div>`,
      // the primary is the action that works in the state shown: Log in for a login that is missing or expired
      // (Start stays, disabled, with the reason), Start otherwise
      actions: `<button class="btn" data-act="start-in-folder" data-name="${esc(name)}">Start in a folder…</button>${a.needsLogin
        ? `<button class="btn" disabled title="${esc(blocked)}" data-testid="detail-start">Start</button><button class="btn primary" data-act="login" data-name="${esc(name)}" data-testid="detail-login">Log in</button>`
        : `<button class="btn ${blocked ? '' : 'primary'}" data-act="start" data-name="${esc(name)}" ${blocked ? `disabled title="${esc(blocked)}"` : ''} data-testid="detail-start"><span class="trunc">Start ${esc(p.label)}</span></button>`}<button class="btn icon" data-act="account-menu" data-name="${esc(name)}" data-home="${esc(p.home)}" data-main="${p.main ? 1 : ''}" aria-label="More actions" title="More actions">⋯</button>` })}
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
    <div class="form-field"><label for="sd-recent">Folder</label><div class="field-row">${recent.length ? `<select id="sd-recent" class="mono" aria-label="Recent folders of this account">${recent.map((r) => `<option value="${esc(r)}">${esc(r)}</option>`).join('')}</select>` : ''}<button class="btn-secondary" id="sd-choose">Choose folder…</button></div><span class="mono muted" id="sd-dir" data-testid="sd-dir">${esc(dir || 'No folder chosen')}</span></div>
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
// the ▾ beside a lane's Start: pick a folder and go, or the full dialog (recent folders, flags, arguments)
action('start-menu', (el) => openMenu(el, [
  { label: 'Choose folder…', testid: 'sm-choose', run: async () => { const dir = await pickFolder(); if (dir) attempt(() => call('start', { name: el.dataset.name, cwd: dir }), `Opened ${acctName(el.dataset.name)} in ${dir}`); } },
  { label: 'Recent folders and options…', testid: 'sm-dialog', run: () => startDialog(el.dataset.name) },
]));
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
    document.querySelectorAll('.np-accts .filter-chip').forEach((b) => { b.classList.toggle('active', b.dataset.name === n); b.setAttribute('aria-pressed', b.dataset.name === n); });
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
/** A one-field dialog of the ⋯ menu (Rename, Daily spend cap…): `field` is the label and the input #fd-in, `save`
 * gets what was typed; Enter saves too, and a failure is said in the dialog, which stays open. */
function fieldDialog(title, field, note, save) {
  openDialog(`${dlgHead(title)}<div class="form-field">${field}</div><span class="muted">${note}</span>
    <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" id="fd-ok" data-testid="fd-ok">Save</button></span></div>`, { size: 'sm', label: title });
  const input = document.getElementById('fd-in');
  const ok = async () => { if ((await attempt(() => save(input.value.trim()), 'Saved')) !== undefined) { closeDialog(); changed(); } };
  document.getElementById('fd-ok').onclick = ok;
  input.onkeydown = (e) => { if (e.key === 'Enter') ok(); };
  input.select();
}
async function refreshNow() {
  const r = await attempt(() => call('refresh', { live: S.state && S.state.live_quota }));
  if (r) { toast(refreshText(r)); changed(); }
}
/** One account's limits, now: a Claude subscription's from Anthropic, with the login Claude Code saved; another
 * tool's from what the tool wrote, which is what the top bar's refresh reads (for every account). */
const refreshLimits = (p) => (p.harness === 'claude' ? mutate(() => call('quota_live', { name: p.name }), 'Limits read') : refreshNow());
/** The ⋯ of an account, on its row and on its page: every action besides the row's one button, Remove… last. */
action('account-menu', (el) => {
  const name = el.dataset.name, p = (S.state.profiles || []).find((x) => x.name === name) || { identity: {} };
  const sub = p.kind === 'subscription', needsLogin = sub && (p.identity.needs_login || !p.identity.logged_in);
  openMenu(el, [
    { label: 'Start in a folder…', testid: 'am-folder', run: () => startDialog(name) },
    ...(sub && !needsLogin && PLAN_TOOLS.includes(p.harness) ? [{ label: 'Refresh limits', testid: 'am-refresh', run: () => refreshLimits(p) }] : []),
    { label: 'Rename', testid: 'am-rename', run: () => fieldDialog('Rename ' + (p.label || name), `<label for="fd-in">Name</label><input id="fd-in" value="${esc(p.label || '')}" placeholder="${esc(name)}" maxlength="60" data-testid="fd-in">`,
      `How this account is shown. Its folder and the command line keep the id <span class="mono">${esc(name)}</span>.`, (v) => call('set_label', { name, label: v })) },
    { label: 'Edit', testid: 'am-edit', run: () => go('account', { name, tab: 'settings' }) },
    { label: 'Open folder', testid: 'am-open', run: () => call('open_folder', { path: el.dataset.home }) },
    ...(sub && !needsLogin ? [{ label: 'Log in again', testid: 'am-login', run: () => startLogin(name) }] : []),
    ...(p.kind === 'routed' ? [{ label: 'Daily spend cap…', testid: 'am-cap', run: () => fieldDialog('Daily spend cap of ' + (p.label || name), `<label for="fd-in">Cap in USD</label><input id="fd-in" type="number" min="0" step="0.5" value="${esc(p.daily_cap_usd ?? '')}" placeholder="no cap" data-testid="fd-in">`,
      'The most the router spends for this account in a day, at API list prices. At the cap it stops answering for it until midnight. Empty: no cap.', (v) => call('set_cap', { name, cap: v === '' ? null : +v })) }] : []),
    ...(p.harness === 'claude' && !p.main ? [{ label: 'Sync from the main profile', testid: 'am-sync', run: () => mutate(() => call('sync', {}), 'Accounts are in step') }] : []),
    ...((p.running || []).length ? [{ label: 'Show console window', testid: 'am-window', run: () => call('show_window', { pid: p.running[0].pid }) }] : []),
    { label: p.main ? 'Hide main profile…' : 'Remove…', danger: true, testid: 'am-remove', run: () => removeAccount(name, el.dataset.main === '1') },
  ]);
  // as drawn: 30 px items, a line before Remove…, the menu's right edge under the button's (pages-accounts.css)
  const m = document.querySelector('.menu'), r = rectOf(el);
  m.classList.add('acct-menu');
  m.style.left = Math.max(8, r.right - m.offsetWidth) + 'px';
  const [, H] = windowSize();
  m.style.top = (r.bottom + 4 + m.offsetHeight > H - 8 ? Math.max(8, r.top - 4 - m.offsetHeight) : r.bottom + 4) + 'px';
});
action('login', (el) => startLogin(el.dataset.name));
// a new dialog starts from a Claude Code subscription, not from the last one's choices
action('add-account', () => { add.harness = 'claude'; add.kind = 'subscription'; addDialog(); });
action('add-account-for', (el) => { add.harness = el.dataset.h; add.kind = 'subscription'; addDialog(); });
action('limits-refresh', (el) => refreshLimits((S.state.profiles || []).find((x) => x.name === el.dataset.name) || {}));
action('refresh-now', refreshNow);

// ---------- add account ----------
const add = { harness: 'claude', kind: 'subscription' };
const PREFIX = { claude: '.claude-', codex: '.codex-', opencode: '.opencode-', agy: '.agy-', pi: '.pi-', omp: '.omp-' };
const KIND_TEXT = { subscription: { claude: 'Pro, Max, Team or Enterprise. You sign in in the browser.', codex: 'ChatGPT Plus, Pro or Team. You sign in in the browser.', opencode: 'OpenCode signs in to its providers in a terminal.', agy: 'Google sign-in. Antigravity keeps one login at a time; Start swaps it.', pi: 'Pi signs in to a provider with /login inside the tool.', omp: 'Oh My Pi signs in to a provider with /login inside the tool.' }, api_key: 'Pay per use. The key is kept in {store}.', routed: 'Sent through the local router to a route of providers.' };
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
    <div class="form-field"><label>Account kind for ${esc(h.label)}</label><div class="kind-pick-grid" role="group" aria-label="Account kind">${kinds.map(([k, l, ok]) => `<button type="button" class="kind-pick-card ${k === add.kind ? 'active' : ''} ${ok ? '' : 'disabled'}" aria-pressed="${k === add.kind}" data-act="add-kind" data-kind="${k}" ${ok ? '' : 'disabled'} data-testid="kind-${k}"><div class="kind-pick-head"><b>${l}</b><span class="plan-pill ${ok ? 'ac' : 'neg'}">${ok ? 'Supported ✓' : 'Unsupported'}</span></div><div class="tool-pick-sup">${ok ? esc(k === 'subscription' ? KIND_TEXT.subscription[h.id] || 'Sign in with the tool.' : KIND_TEXT[k].replace('{store}', esc(keyStore()))) : `${esc(h.label)} takes ${k === 'api_key' ? 'no API key' : 'no route'}.`}</div></button>`).join('')}</div></div>
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
      st.first_run ||= S.state.first_run; // this read is not the loader's: the first-run report stays
      S.state = st;
      const dlg = document.querySelector('.modal-card');
      if (dlg) dlg.innerHTML = body(`Signed in${now.identity.email ? ' as ' + esc(now.identity.email) : ''}.`, true);
    }, 2000);
    return;
  }
  const flow = await attempt(() => call('login_start', { name }));
  if (!flow) return;
  let timer, drawn;
  const draw = (st) => {
    const dlg = document.querySelector('.modal-card');
    if (!dlg) return;
    // the poll answers every second: the dialog is built again only when what it shows has changed
    const now = JSON.stringify([st.state, st.message, st.url, st.email_mismatch]);
    if (now === drawn) return;
    drawn = now;
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
