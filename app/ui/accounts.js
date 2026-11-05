// Accounts: the list with limits and the reset timeline, account detail, and Add account with sign-in.

import { S, call, view, go, toast, attempt, mutate, changed, acctName, fit, refreshText, shownTools, openDialog, closeDialog, dlgHead, openMenu, action, loadState, render, pickFolder } from './core.js';
import { esc, fmt, hm, when, dur, resetText, lvl, avatar, toolPill, toolIcon, meter, ic, bars, timeline } from './lib.js';
import { tagsHtml, stacked } from './views.js';
import { saveFile } from './core.js';

const tier = (p, id) => p.quota && p.quota.tiers.find((t) => t.id === id);
const weekTier = (p) => tier(p, 'week') || (p.quota && p.quota.tiers.find((t) => t.id.startsWith('week')));
const dayLabels = () => [...Array(7)].map((_, i) => { const d = new Date(); d.setDate(d.getDate() - 6 + i); return d.toLocaleDateString([], { weekday: 'short', day: 'numeric' }); });
const emailOf = (p) => p.identity.email || p.expected_email || '';

function planCell(p) {
  if (p.kind === 'api_key') return `<span class="pill warn">API key</span>`;
  if (p.kind === 'routed') return `<span class="pill iris">Routed · ${esc(p.route || '')}</span>`;
  if (!p.identity.logged_in) return `<span class="pill neg">Not logged in</span>`;
  if (p.identity.needs_login) return `<span class="pill neg">Login expired</span>`;
  return `<span class="pill teal" title="${esc(p.identity.plan || '')}">${esc(p.identity.plan || 'Logged in')}</span>`;
}

/** Every limit of the plan as "% left" with the time until it resets; or why there is none to show. */
function limits(p, now, x = {}) {
  const cap = x.caps && x.caps[p.name];
  if (cap) {
    const left = Math.max(0, Math.round(100 - (cap.spent / cap.cap) * 100)), mid = new Date(now); mid.setHours(24, 0, 0, 0);
    return `<div class="l1"><span class="ln">$${cap.cap.toFixed(2)} daily cap</span><span class="bar"><i class="${lvl(100 - left)}" style="width:${left}%"></i></span><span class="lp">${left}% left</span><span class="rs">${ic.refresh} ${dur(mid - now)}</span></div>`;
  }
  if (p.kind === 'api_key') return '<span class="dash">Pay per use</span>';
  if (p.kind === 'routed') return `<span class="dash">Through route ${esc(p.route || '')}</span>`;
  const tiers = (p.quota && p.quota.tiers) || [];
  if (!tiers.length) return `<span class="dash">${!p.identity.logged_in ? 'Log in to see its plan and limits.' : p.caps.quota ? 'Limits not read yet' : 'No plan limits'}</span>`;
  const used = S.state && S.state.settings && S.state.settings.quota_display === 'used';
  return tiers.map((t) => {
    const left = Math.max(0, Math.round(100 - t.used));
    if (used) {
      const u = Math.min(100, Math.round(t.used));
      const reset = t.resets_at && t.resets_at > now ? `<span class="rs" title="Resets ${esc(resetText(t.resets_at, now).replace(/^until /, ''))}">${ic.refresh} ${dur(t.resets_at - now)}</span>` : '<span class="rs"></span>';
      return `<div class="l1"><span class="ln">${esc(t.label)}</span><span class="bar used"><i class="${lvl(t.used)}" style="width:${u}%"></i></span><span class="lp ${u >= 100 ? 'err' : ''}">${u}% used</span>${reset}</div>`;
    }
    const reset = t.resets_at && t.resets_at > now ? `<span class="rs" title="Resets ${esc(resetText(t.resets_at, now).replace(/^until /, ''))}">${ic.refresh} ${dur(t.resets_at - now)}</span>` : '<span class="rs"></span>';
    return `<div class="l1"><span class="ln">${esc(t.label)}</span><span class="bar"><i class="${lvl(t.used)}" style="width:${left}%"></i></span><span class="lp ${left === 0 ? 'err' : ''}">${left === 0 ? 'Used up' : left + '% left'}</span>${reset}</div>`;
  }).join('');
}

/** The line under an account's limits: the best place to start, or a limit that runs out before it resets. */
function rowNote(p, best, now, x = {}) {
  if (best && p.name === best.name && p.kind === 'subscription') return '<div class="rnote ok" data-testid="best-note">Most room of any account: start here next.</div>';
  for (const t of (p.quota && p.quota.tiers) || []) {
    const f = x.forecast && x.forecast[p.name + '|' + t.id];
    if (f && f.runs_out && t.resets_at && f.runs_out < t.resets_at) {
      const when = new Date(f.runs_out).toLocaleString([], { weekday: 'short', hour: '2-digit', minute: '2-digit', hour12: false });
      return `<div class="rnote warn">${esc(t.label)} runs out ${esc(when)} at this pace.</div>`;
    }
  }
  return '';
}

/** One account in its tool's card. */
// usage drawn into the rows: the range, each account's breakdown and day series, and the colour it has in the chart
const au = { days: 7, by: {}, series: {}, color: {}, models: [], fold: new Set(['models']) };
const money = (c, d = 2) => (c == null ? '—' : '$' + c.toLocaleString(undefined, { minimumFractionDigits: d, maximumFractionDigits: d }));
const sparkOf = (vals) => { const m = Math.max(1, ...vals); return `<div class="spark" aria-hidden="true">${vals.slice(-30).map((v, i, a) => `<i style="height:${Math.max(3, Math.round((v / m) * 100))}%" class="${i === a.length - 1 ? 'last' : ''}"></i>`).join('')}</div>`; };
function accountRow(p, best, now, x = {}) {
  const cap = x.caps && x.caps[p.name];
  const b = au.by[p.name], vals = au.series[p.name] || [];
  const read = p.quota && p.quota.fetched_at;
  const live = p.harness === 'claude' && p.kind === 'subscription' && p.identity.logged_in;
  const added = p.created_at ? 'Added ' + new Date(p.created_at).toLocaleDateString([], { day: 'numeric', month: 'short' }) : '';
  return `<div class="acr click ${best && p.name === best.name ? 'best' : ''}" data-act="open-account" data-name="${esc(p.name)}" data-testid="row-${esc(p.name)}">
    ${avatar(p.label)}
    <div class="who"><b>${esc(p.label)}${p.main ? ' <span class="muted" style="font-weight:400">· main</span>' : ''}</b>
      <span class="mono">${esc(emailOf(p) || p.identity.name || p.base_url || (p.identity.logged_in ? 'logged in' : 'not logged in'))}</span>
      <span class="meta">${planCell(p)}${added ? `<span>${added}</span>` : ''}${p.email_mismatch ? ' <span class="pill neg" title="Logged in as a different email than you entered">email differs</span>' : ''}</span></div>
    <div class="lims">${limits(p, now, x)}${rowNote(p, best, now, x)}</div>
    <div class="usp" title="${esc(au.days + ' days: ' + (vals.length ? vals.map((v) => fmt(v)).join(' · ') : 'no usage'))}">${sparkOf(vals.length ? vals : [0])}</div>
    <div class="use" title="${b ? esc(`input ${fmt(b.input)} · output ${fmt(b.output)} · cache read ${fmt(b.cache_read)} · ${b.requests.toLocaleString()} requests · ${Math.round(b.cache_hit_rate * 100)}% from cache`) : 'no usage in this range'}"><b>${fmt(b ? b.tokens : 0)}</b><small>tokens · ${au.days} days</small>${p.running.length ? `<span class="run"><i></i>${p.running.length} open</span>` : ''}</div>
    <div class="use cost" data-testid="cost-${esc(p.name)}">${cap ? `<b>$${cap.spent.toFixed(2)}</b><small>today, cap ${money(cap.cap)}</small>` : `<b>${money(b ? b.cost : null)}</b><small>list price</small>`}</div>
    <div class="read">${read ? `<span title="When these limits were read">${now - read < 60000 ? 'just now' : dur(now - read) + ' ago'}</span>` : ''}${live ? `<button class="iconbtn" data-act="quota-live" data-name="${esc(p.name)}" title="Read this account's limits from Anthropic now, with the login Claude Code saved" aria-label="Read limits of ${esc(p.label)} now" data-testid="live-${esc(p.name)}">${ic.refresh}</button>` : ''}</div>
    <div class="go">${p.identity.logged_in || p.kind !== 'subscription'
      ? `<button class="btn primary sm" data-act="start" data-name="${esc(p.name)}" data-testid="start-${esc(p.name)}">Start</button>`
      : `<button class="btn primary sm" data-act="login" data-name="${esc(p.name)}" data-testid="login-${esc(p.name)}">Log in</button>`}
      <button class="iconbtn sq" data-act="account-menu" data-name="${esc(p.name)}" data-home="${esc(p.home)}" data-main="${p.main ? 1 : ''}" aria-label="More for ${esc(p.label)}" data-testid="menu-${esc(p.name)}">${ic.dots}</button></div></div>`;
}

/** Rows of "When limits free up": every account whose plan has limits; the ones not read yet say so instead of disappearing. */
export function limitRows(ps) {
  return ps.filter((p) => (p.quota && p.quota.tiers.length) || (p.kind === 'subscription' && p.caps.quota)).map((p) => ({ name: p.label, session: tier(p, 'session'), week: weekTier(p),
    note: p.quota && p.quota.tiers.length ? '' : p.identity.logged_in ? 'limits not read yet: they show once it runs on this computer' : 'not logged in' }));
}

// Accounts page: the tool chip picked, and the tool groups folded shut (open by default)
let acctTool = '';
const folded = new Set();
const roomUsed = (p) => (p.identity.logged_in || p.kind !== 'subscription' ? 0 : 1000) + (tier(p, 'session')?.used || 0) + (weekTier(p)?.used || 0);

/** The account with the most room right now, and limits that need attention. */
export function findings(ps, now) {
  const withLimits = ps.filter((p) => p.identity.logged_in && p.kind === 'subscription' && p.quota && p.quota.tiers.length);
  const used = (p) => (tier(p, 'session')?.used || 0) + (weekTier(p)?.used || 0);
  const best = withLimits.slice().sort((a, b) => used(a) - used(b))[0] || ps.find((p) => p.identity.logged_in);
  const hot = [];
  for (const p of withLimits) for (const t of p.quota.tiers) if (t.used >= 75 && !(t.resets_at && t.resets_at <= now)) hot.push(`<b>${esc(p.label)}</b> is at ${Math.round(t.used)}% of its ${esc(t.label.toLowerCase())}${t.resets_at ? ' until ' + esc(resetText(t.resets_at, now).replace(/^until /, '')) : ''}.`);
  const notIn = ps.filter((p) => !p.identity.logged_in).length;
  const mismatch = ps.filter((p) => p.email_mismatch);
  let text = '';
  if (best) {
    const s = tier(best, 'session'), w = weekTier(best);
    text = `<b>${esc(best.name)}</b> has the most room` + (s || w ? `: ${s && s.used > 0 ? Math.round(s.used) + '% of its 5-hour limit' : 'no 5-hour session open'} and ${w ? Math.round(w.used) + '% of its week' : 'no weekly limit'} used.` : '.');
  }
  if (hot.length) text += ' ' + hot.slice(0, 2).join(' ');
  if (mismatch.length) text += ` <b>${esc(mismatch[0].name)}</b> is logged in as a different email than expected.`;
  if (notIn) text += ` ${notIn} ${notIn === 1 ? 'account is' : 'accounts are'} not logged in yet.`;
  return { best, text: text.trim() || 'Add an account to get started.' };
}

view('accounts', async (sheet) => {
  const [st, u] = await Promise.all([loadState(), call('usage', { days: au.days }).catch(() => null)]);
  if (u) {
    au.by = Object.fromEntries(u.accounts.map((b) => [b.key, b]));
    au.series = u.summary.by_profile || {};
    au.models = u.models || [];
    const order = u.accounts.map((b) => b.key);
    au.color = Object.fromEntries(order.map((k, i) => [k, SERIES[i % SERIES.length]]));
    au.days_list = u.summary.days;
  }
  // the accounts of hidden tools are left out too
  const shownIds = shownTools(st).map((h) => h.id);
  const ps = st.profiles.filter((p) => shownIds.includes(p.harness)), now = st.now;
  sheet.className = 'sheet';
  if (st.first_run && (st.first_run.main_profiles.length || st.first_run.accounts.length)) {
    toast(`Found ${st.first_run.main_profiles.length} installed tools and imported ${st.first_run.accounts.length} accounts from your PowerShell launcher.`);
  }
  const { best } = findings(ps, now);
  const x = await call('limits_extra').catch(() => ({}));
  const q = (document.getElementById('find').value || '').toLowerCase();
  const shown = ps.filter((p) => !q || p.name.toLowerCase().includes(q) || p.label.toLowerCase().includes(q) || emailOf(p).toLowerCase().includes(q));
  const fetched = ps.map((p) => p.quota && p.quota.fetched_at).filter(Boolean);
  const readAgo = fetched.length ? `Limits read ${dur(now - Math.max(...fetched))} ago` : '';
  const tools = shownTools(st).filter((h) => !acctTool || h.id === acctTool);
  const chip = (id, label) => `<a class="${acctTool === id ? 'on' : ''}" data-act="acct-tool" data-v="${id}" data-testid="chip-${id || 'all'}">${label}</a>`;
  sheet.innerHTML = `
    <div class="trow"><div><h1>Accounts</h1><p class="find" data-testid="finding">${ps.length} ${ps.length === 1 ? 'account' : 'accounts'} across ${new Set(ps.map((p) => p.harness)).size} ${new Set(ps.map((p) => p.harness)).size === 1 ? 'tool' : 'tools'}. Each runs in its own folder with its own login, side by side.</p></div>
      <div class="acts"><span class="seg3">${[7, 14, 30].map((d) => `<a class="${d === au.days ? 'on' : ''} mono" data-act="acct-days" data-v="${d}" data-testid="u-days-${d}">${d}d</a>`).join('')}</span><button class="btn" data-act="acct-csv" data-testid="u-csv">Export CSV</button>${best ? `<button class="btn" data-act="start-in-folder" data-name="${esc(best.name)}" data-testid="start-best">Start in a folder…</button>` : ''}
      <button class="btn primary" data-act="add-account" data-testid="add-account">Add account</button></div></div>
    ${(() => { const series = ps.filter((p) => au.series[p.name] && au.series[p.name].some((v) => v)).map((p) => ({ name: p.label, color: au.color[p.name] || SERIES[0], vals: au.series[p.name] })); const open = !au.fold.has('chart');
      return `<section class="panel uchartp ${open ? '' : 'shut'}" data-testid="usage-panel"><div class="ph"><h2 class="gh2"><button class="fold" data-act="acct-fold" data-h="chart" aria-expanded="${open}"><span class="thead"><b>Tokens per day, by account</b><span class="muted">${au.days} days</span></span><span class="chev" aria-hidden="true">${open ? '▾' : '▸'}</span></button></h2>${open ? `<div class="ulegend">${series.map((x) => `<span><i style="background:${x.color}"></i>${esc(x.name)}</span>`).join('')}</div>` : ''}</div>
        ${open ? (series.length ? '<div class="uchart" data-testid="usage-ledger"></div>' : '<p class="empty">No usage in this range.</p>') : ''}</section>`; })()}
    <div class="fbar"><div class="toolfilter" role="group" aria-label="Tool">${chip('', 'All')}${shownTools(st).map((h) => chip(h.id, esc(h.label))).join('')}</div>
      <span class="muted sm">Sorted by room left${readAgo ? ' · ' + readAgo.toLowerCase() : ''}${u ? ' · usage at API list prices' : ''}</span></div>
    <div class="acgroups" data-testid="accounts-table">${tools.map((h) => {
      const list = shown.filter((p) => p.harness === h.id).sort((a, b) => roomUsed(a) - roomUsed(b));
      const open = !folded.has(h.id), on = list.filter((p) => p.identity.logged_in).length;
      const next = list.flatMap((p) => (p.quota ? p.quota.tiers.filter((t) => t.id === 'session' && t.resets_at > now) : [])).sort((a, b) => a.resets_at - b.resets_at)[0];
      const open2 = list.reduce((a, p) => a + p.running.length, 0);
      const tok = list.reduce((a, p) => a + (au.by[p.name] ? au.by[p.name].tokens : 0), 0), cst = list.some((p) => au.by[p.name] && au.by[p.name].cost != null) ? list.reduce((a, p) => a + ((au.by[p.name] || {}).cost || 0), 0) : null;
      const meta = list.length ? `${on} logged in · ${open2 ? `<span class="acT" data-act="go-live" role="link">${open2} ${open2 === 1 ? 'console' : 'consoles'} open</span>` : '0 consoles open'}${next ? ' · next reset ' + hm(next.resets_at) : ''}${tok ? ` · ${fmt(tok)} tokens · ${money(cst)} · ${au.days} days` : ''}` : '';
      return `<section class="acg ${open ? '' : 'shut'}" data-testid="group-${h.id}"><header>
        <h2 class="gh2"><button class="fold" data-act="acct-fold" data-h="${h.id}" aria-expanded="${open}"><span class="thead">${toolIcon(h.id, 18)}<b>${esc(h.label)}</b>${list.length ? `<span class="muted">${list.length}</span>` : ''}</span><span class="muted sm">${meta}</span><span class="chev" aria-hidden="true">${open ? '▾' : '▸'}</span></button></h2>
</header>
        ${!open ? '' : list.length ? list.map((p) => accountRow(p, best, now, x)).join('') : `<p class="empty">${h.installed ? (q ? 'No account matches the search.' : `No ${esc(h.label)} account yet.`) : `${esc(h.label)} is not installed on this computer.`}</p>`}</section>`;
    }).join('')}</div>
    ${(() => { const open = !au.fold.has('models'), top = au.models[0] ? au.models[0].tokens : 1;
      return `<section class="panel ${open ? '' : 'shut'}" data-testid="models-panel"><div class="ph"><h2 class="gh2"><button class="fold" data-act="acct-fold" data-h="models" aria-expanded="${open}"><span class="thead"><b>By model</b><span class="muted">${au.days} days</span></span><span class="chev" aria-hidden="true">${open ? '▾' : '▸'}</span></button></h2></div>
        ${open ? (au.models.map((m) => `<div class="umodel"><span class="mono">${esc(m.key)}</span><span class="mb"><i style="width:${Math.round((m.tokens / top) * 100)}%"></i></span><b class="mono">${fmt(m.tokens)}</b><span class="mono muted">${m.cost == null ? '—' : money(m.cost)}</span></div>`).join('') || '<p class="empty">No usage yet.</p>') : ''}</section>`; })()}
`;
  const ch = sheet.querySelector('.uchart');
  if (ch && au.days_list) { const series = ps.filter((p) => au.series[p.name] && au.series[p.name].some((v) => v)).map((p) => ({ name: p.label, color: au.color[p.name] || SERIES[0], vals: au.series[p.name] })); fit(ch, (w) => stacked(au.days_list, series, w)); }
});
const SERIES = ['var(--ac)', 'var(--iris)', '#5b6fb8', 'var(--sand)', 'var(--h3)', '#c084fc', '#94a3b8', '#f472b6'];
action('acct-days', (el) => { au.days = +el.dataset.v; render(); });
action('acct-csv', async () => {
  const to = await saveFile(`Harness Manager usage ${new Date().toISOString().slice(0, 10)}.csv`, { name: 'CSV', extensions: ['csv'] });
  if (to) attempt(() => call('usage_csv', { days: au.days, path: to }), 'Exported');
});
action('acct-tool', (el) => { acctTool = el.dataset.v; render(); });
action('acct-fold', (el) => { const h = el.dataset.h, set = h === 'chart' || h === 'models' ? au.fold : folded; if (set.has(h)) set.delete(h); else set.add(h); render(); });

// ---------- account detail ----------
view('account', async (sheet, params) => {
  const name = params.name, tab = params.tab || 'overview';
  const d = await call('detail', { name });
  const p = d.view, now = Date.now();
  sheet.className = 'sheet';
  const tabs = [['overview', 'Overview'], ['sessions', 'Sessions'], ['projects', 'Projects'], ['settings', 'Profile settings']];
  const sub = [emailOf(p), p.identity.plan, p.identity.org].filter(Boolean).map(esc).join(' · ');
  const openTxt = p.running.length ? ` ${p.running.length} ${p.running.length === 1 ? 'console' : 'consoles'} open${p.running[0].cwd ? ' in ' + esc(p.running[0].cwd.split(/[\\/]/).pop()) : ''}.` : '';
  let body = '';
  if (tab === 'overview') {
    const [k, x, u] = await Promise.all([call('account_stats', { name }).catch(() => null), call('limits_extra').catch(() => ({})), call('usage', { days: 30, profile: name }).catch(() => null)]);
    const models = (u && u.models) || [], topModel = models[0] ? models[0].tokens : 1;
    const w = k ? k.week : null, tiers = p.quota ? p.quota.tiers : [];
    const money = (c) => (c == null ? '—' : '$' + c.toFixed(2));
    const secs = (ms) => (ms == null ? '—' : (ms / 1000).toFixed(1) + ' s');
    const week = d.sessions.filter((s) => now - s.last_active_at < 7 * 86400000), running = d.sessions.filter((s) => s.open).length;
    const tile = (label, value, note) => `<div class="ovk"><div class="kl"><span>${label}</span></div><strong>${value}</strong><div class="kn">${note}</div></div>`;
    const lim = (t) => {
      const left = Math.max(0, Math.round(100 - t.used)), f = x.forecast && x.forecast[p.name + '|' + t.id];
      const tone = left < 20 ? 'neg' : left < 45 ? 'warn' : '';
      const reset = !t.resets_at ? 'no window open' : t.resets_at <= now ? 'window has reset' : 'resets ' + (t.resets_at - now < 86400000 ? 'in ' + dur(t.resets_at - now) : new Date(t.resets_at).toLocaleString([], { weekday: 'short', hour: '2-digit', minute: '2-digit', hour12: false }));
      const note = f && f.runs_out && t.resets_at && f.runs_out < t.resets_at ? `Runs out ${new Date(f.runs_out).toLocaleString([], { weekday: 'short', hour: '2-digit', minute: '2-digit', hour12: false })} at this pace.`
        : f ? 'Lasts until the reset at this pace.' : left >= 60 ? 'Plenty left.' : '';
      return `<div class="bd"><div class="bt"><b>${esc(t.label)}</b><span class="mono ${tone}T">${left === 0 ? 'Used up' : left + '% left'} · ${esc(reset)}</span></div>
        <div class="bb"><i class="${tone}" style="width:${Math.min(100, t.used)}%"></i>${f ? `<em style="left:${Math.min(99.5, f.at_reset)}%"></em>` : ''}</div>${note ? `<div class="bn">${note}</div>` : ''}</div>`;
    };
    const facts = [
      ['Login', p.kind !== 'subscription' ? (p.identity.logged_in ? 'key saved' : '<span class="negT">no key</span>') : !p.identity.logged_in ? '<span class="negT">not logged in</span>' : p.identity.needs_login ? '<span class="negT">expired</span>' : '<span class="acT">valid</span>'],
      ['Folder', `<a class="mono" style="cursor:pointer" data-act="open-folder" data-path="${esc(p.home)}">${esc(p.home)}</a>`],
      ['Started with', { claude: 'CLAUDE_CONFIG_DIR', codex: 'CODEX_HOME', opencode: 'XDG_CONFIG_HOME', agy: 'its own login', pi: 'PI_CODING_AGENT_DIR', omp: 'PI_CODING_AGENT_DIR' }[p.harness] || '—'],
      ...(p.harness === 'claude' && !p.main ? [['Follows the main profile', 'CLAUDE.md, rules, skills, MCP'], ['Own', 'login, history, usage']] : []),
      ...(p.harness === 'claude' && p.kind === 'subscription' ? [['Live limits', S.state && S.state.live_quota ? 'on (reads the saved login)' : 'off']] : []),
      ['CLI', esc(d.cli)],
    ];
    const per = k ? k.per_day : [], max = Math.max(1, ...per);
    const chart = per.length ? `<svg viewBox="0 0 ${per.length * 20} 120" preserveAspectRatio="none" class="abars" role="img" aria-label="Tokens per day, 30 days">${per.map((v, i) => `<rect x="${i * 20 + 3}" y="${120 - (v / max) * 116}" width="14" height="${(v / max) * 116}" rx="2" class="${i === per.length - 1 ? 'today' : ''}"><title>${esc(k.days[i])}: ${fmt(v)} tokens</title></rect>`).join('')}</svg>
      <div class="hl mono"><span>${esc(new Date(k.days[0] + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short' }))}</span><span>${esc(new Date(k.days[14] + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short' }))}</span><span>today</span></div>` : '';
    body = `<section class="ovkpis acctk">
        ${tile('Tokens · 7 days', w ? fmt(w.tokens) : '—', w ? Math.round(w.hit * 100) + '% read from cache' : '')}
        ${tile('At list prices', w ? money(w.cost) : '—', p.kind === 'subscription' ? 'subscription: not billed per token' : 'billed per token')}
        ${tile('First reply p95', w ? secs(w.p95) : '—', w && w.replies ? `p50 ${secs(w.p50)} · ${w.replies.toLocaleString()} replies` : 'no timed replies')}
        ${tile('Sessions · 7 days', week.length, running ? `${running} running now` : 'none running now')}
        ${tile('Busiest project', w && w.top_project ? Math.round(w.top_project.share * 100) + '%' : '—', w && w.top_project ? esc(w.top_project.name) : 'no usage this week')}</section>
      <div class="dgrid acctg"><div class="stack">
        <section class="panel"><div class="ph"><h2>Limits</h2><span class="muted sm">${p.quota && p.quota.fetched_at ? 'read ' + dur(now - p.quota.fetched_at) + ' ago' : p.caps.quota ? 'not read yet' : esc(p.harness_label) + ' has no limits to read'} · mark = level at reset at this pace</span></div>
          ${tiers.length ? tiers.map(lim).join('') : `<p class="empty">${p.kind === 'subscription' ? 'Limits show up after this account runs on this computer.' : 'API keys are billed per use and have no plan limits.'}</p>`}</section>
        <section class="panel"><div class="ph"><h2>Tokens per day, 30 days</h2><span class="muted mono sm">${k ? fmt(per.reduce((a2, v) => a2 + v, 0)) + ' · ' + money(k.cost30) + ' at list prices' : ''}</span></div>${chart || '<p class="empty">No usage yet.</p>'}</section>
        <section class="panel" data-testid="account-models"><div class="ph"><h2>By model, 30 days</h2></div>
          ${models.map((m) => `<div class="umodel"><span class="mono">${esc(m.key)}</span><span class="mb"><i style="width:${Math.round((m.tokens / topModel) * 100)}%"></i></span><b class="mono">${fmt(m.tokens)}</b><span class="mono muted">${m.cost == null ? '—' : money(m.cost)}</span></div>`).join('') || '<p class="empty">No usage yet.</p>'}</section>
      </div><div class="stack">
        <section class="panel"><div class="ph"><h2>Login and folder</h2></div><dl class="facts">${facts.map(([a2, b]) => `<dt>${a2}</dt><dd>${b}</dd>`).join('')}</dl>
          ${p.kind === 'subscription' ? `<button class="btn sm" data-act="login" data-name="${esc(name)}">Log in again</button>` : ''}</section>
        ${p.running.length ? `<section class="panel"><div class="ph"><h2>Open now</h2></div>${p.running.map((r) => `<div class="list"><div><span class="mono">${esc(r.cwd || 'folder unknown')}</span></div><div><span class="muted">PID ${r.pid}${r.started_at ? ' · since ' + hm(r.started_at) : ''}</span><button class="btn outline sm" data-act="show-window" data-pid="${r.pid}">Show window</button></div></div>`).join('')}</section>` : ''}
        <section class="panel"><div class="ph"><h2>Recent sessions</h2><a href="#account?name=${encodeURIComponent(name)}&tab=sessions" class="acT sm">All ${d.sessions.length}</a></div>
          ${d.sessions.slice(0, 5).map((s2) => `<div class="rs2" data-act="open-session" data-profile="${esc(s2.profile)}" data-id="${esc(s2.id)}" role="button" tabindex="0"><i class="sdot ${s2.open ? 'live' : s2.failed ? 'bad' : ''}"></i><div><b>${esc(s2.title || 'Untitled session')}</b><small>${esc(s2.project_name || (s2.cwd || '').split(/[\\/]/).pop() || '')}</small></div><span class="mono ${s2.open ? 'acT' : 'muted'}">${s2.open ? 'live' : esc(when(s2.last_active_at))}</span></div>`).join('') || '<p class="empty">No sessions in the last 30 days.</p>'}</section>
      </div></div>`;
  } else if (tab === 'sessions') {
    body = d.sessions.length ? `<div style="margin-top:12px">${d.sessions.map((s) => sessionRow(s)).join('')}</div>` : '<p class="empty">No sessions in the last 30 days.</p>';
  } else if (tab === 'projects') {
    body = d.projects.length ? `<table class="grid"><thead><tr><th>Project</th><th>Key</th><th class="num">Tokens, 7 days</th></tr></thead><tbody>${d.projects.map(([k, n, t]) => `<tr><td>${esc(n)}</td><td class="mono muted">${esc(k)}</td><td class="num">${fmt(t)}</td></tr>`).join('')}</tbody></table>` : '<p class="empty">No usage this week.</p>';
  } else {
    body = `<div class="frow" style="margin-top:14px"><div><b>Name</b><p>How this account is shown in Harness Manager. Its folder and the command line keep the id <span class="mono">${esc(name)}</span>.</p></div><div class="ctl"><input class="input" id="pf-label" value="${esc(p.label)}" placeholder="${esc(name)}" maxlength="60" style="max-width:340px" aria-label="Account name" data-testid="rename-input"><button class="btn outline" data-act="save-label" data-name="${esc(name)}" data-testid="rename-save">Save</button></div></div>
      <div class="frow"><div><b>Expected email</b><p>Checked after each login, so you notice a wrong account.</p></div><div class="ctl"><input class="input" id="pf-email" value="${esc(p.expected_email || '')}" style="max-width:340px"><button class="btn outline" data-act="save-email" data-name="${esc(name)}">Save</button></div></div>
      ${p.kind === 'api_key' ? `<div class="frow"><div><b>API key</b><p>Kept in Windows Credential Manager, never in a file.</p></div><div class="ctl"><input class="input" type="password" id="pf-key" placeholder="Paste a new key" style="max-width:340px"><button class="btn outline" data-act="save-key" data-name="${esc(name)}">Replace key</button></div></div>
        <div class="frow"><div><b>Endpoint and model</b><p>Leave empty to use the vendor's own API and default model.</p></div><div class="ctl"><input class="input" id="pf-url" placeholder="Base URL" value="${esc(p.base_url || '')}" style="max-width:340px"><input class="input" id="pf-model" placeholder="Model" value="${esc(p.model || '')}" style="max-width:220px"><button class="btn outline" data-act="save-endpoint" data-name="${esc(name)}">Save</button></div></div>` : ''}
      ${p.kind === 'routed' ? `<div class="frow"><div><b>Daily spend cap</b><p>The most the router spends for this account in a day, at API list prices. At the cap it stops answering for it until midnight. Empty: no cap.</p></div><div class="ctl"><span class="mono">$</span><input class="input" type="number" min="0" step="0.5" id="pf-cap" value="${p.daily_cap_usd ?? ''}" placeholder="no cap" style="max-width:140px" aria-label="Daily cap in dollars" data-testid="cap-input"><button class="btn outline" data-act="save-cap" data-name="${esc(name)}" data-testid="cap-save">Save</button></div></div>` : ''}
      ${p.harness === 'claude' && !p.main ? `<div class="frow"><div><b>Shared with the main profile</b><p>CLAUDE.md, rules, skills, plugins and MCP servers follow the main profile. Login, history and usage stay with this account.</p></div><div class="ctl"><button class="btn outline" data-act="sync-now">Sync now</button></div></div>` : ''}
      <div class="frow"><div><b>Remove account</b><p>${p.main ? "Stops managing the tool's own profile. Its folder and login are never deleted." : 'Removes it from Harness Manager. Its folder stays unless you choose to delete it.'}</p></div><div class="ctl"><button class="btn danger" data-act="remove-account" data-name="${esc(name)}" data-main="${p.main ? 1 : ''}" data-testid="remove-account">Remove…</button></div></div>`;
  }
  const planPill = p.kind === 'subscription' ? (p.identity.logged_in ? `<span class="pill teal">${esc(p.identity.plan || 'Logged in')}</span>` : '<span class="pill neg">Not logged in</span>') : `<span class="pill ${p.kind === 'routed' ? 'iris' : 'warn'}">${p.kind === 'routed' ? 'Routed' : 'API key'}</span>`;
  sheet.innerHTML = `<div class="ahead"><span class="av big">${esc(p.label.split(/[-_ ]/).filter(Boolean).slice(0, 2).map((x) => x[0].toUpperCase()).join(''))}</span>
    <div class="aname"><a href="#accounts" class="crumb acT">← Accounts</a><h1 data-testid="detail-name">${esc(p.label)}</h1>
      <p class="find">${esc(p.harness_label)} ${emailOf(p) ? `<span class="mono">${esc(emailOf(p))}</span>` : ''} ${planPill} ${esc([p.identity.org, p.identity.role].filter(Boolean).join(' · '))}${p.main ? ' <span class="muted">· main profile</span>' : ''}${openTxt}</p></div>
    <div class="acts"><button class="btn" data-act="start-in-folder" data-name="${esc(name)}">Start in a folder…</button>
      <button class="btn primary" data-act="start" data-name="${esc(name)}" ${!p.identity.logged_in && p.kind === 'subscription' ? 'disabled' : ''}>Start ${esc(p.label)}</button>
      <button class="iconbtn sq" data-act="account-menu" data-name="${esc(name)}" data-home="${esc(p.home)}" data-main="${p.main ? 1 : ''}" aria-label="More">${ic.dots}</button></div></div>
    <div class="tabs2">${tabs.map(([k, l]) => `<a class="${k === tab ? 'on' : ''}" href="#account?name=${encodeURIComponent(name)}&tab=${k}" data-testid="tab-${k}">${l}</a>`).join('')}</div>${body}`;
});

export function sessionRow(s) {
  // a dot (teal: open now), the title with where it ran, the account, the tool, tokens, when, and one action
  const tool = { claude: 'Claude Code', codex: 'Codex', opencode: 'OpenCode', agy: 'Antigravity', pi: 'Pi', omp: 'Oh My Pi' }[s.harness];
  const where = [`${s.messages} ${s.messages === 1 ? 'msg' : 'msgs'}`, s.cwd ? s.cwd.split(/[\\/]/).slice(-2).join('\\') : ''].filter(Boolean).join(' · ');
  return `<div class="srow" data-search="${esc(((s.title || '') + ' ' + (s.cwd || '') + ' ' + acctName(s.profile) + ' ' + (tool || '')).toLowerCase())}" data-testid="session-${esc(s.id)}"><i class="sdot ${s.open ? 'live' : s.failed ? 'bad' : ''}" ${s.failed ? `title="${s.failed} failed ${s.failed === 1 ? 'command' : 'commands'}"` : 'aria-hidden="true"'}></i>
    <div class="t" data-act="open-session" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}" data-testid="read-${esc(s.id)}" role="button" tabindex="0"><b>${esc(s.title || 'Untitled session')}</b><span class="mono" title="${esc(s.cwd)}">${esc(where)}${tagsHtml(s.tags, 'session', s.profile + '/' + s.id, s.title || 'this session')}</span></div>
    <div class="acct">${esc(acctName(s.profile))}</div><div>${toolPill(s.harness, tool)}</div><div class="num mono">${fmt(s.tokens)}</div>
    <div class="when mono ${s.open ? 'live' : ''}">${s.open ? 'live' : esc(when(s.last_active_at))}</div>
    <div class="num">${s.open ? `<button class="btn outline sm" data-act="open-session" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}">Follow</button>`
      : `<button class="btn outline sm" data-act="resume" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}" data-cwd="${esc(s.cwd)}">Resume</button>`}</div></div>`;
}

// ---------- actions ----------
action('open-account', (el, e) => { if (!e.target.closest('button')) go('account', { name: el.dataset.name }); });
action('start', (el) => attempt(() => call('start', { name: el.dataset.name }), `Opened a terminal for ${acctName(el.dataset.name)}`));
// the tool's own flag for running without asking before each step, where it has one
const SKIP = { claude: '--dangerously-skip-permissions', codex: '--dangerously-bypass-approvals-and-sandbox' };

/** Start an account in a folder: a recent project of this account or any folder, optionally without
 * permission prompts, with extra arguments for this terminal only. */
async function startDialog(name) {
  const d = await attempt(() => call('detail', { name }));
  if (!d) return;
  const p = d.view, label = p.label;
  // the account's recent folders, newest first
  const recent = [...new Set(d.sessions.map((s) => s.cwd).filter(Boolean))].slice(0, 6);
  let dir = recent[0] || '';
  openDialog(`${dlgHead('Start ' + label + ' in a folder')}
    <div class="sdlg">
      <div class="note">${ic.info} <div><b>Runs as ${esc(label)}</b><br>With its own login, history and usage. Other accounts, and the consoles they have open, are not affected.</div></div>
      ${recent.length ? `<span class="fl">Recent folders of this account</span><div class="recent">${recent.map((r) => `<button class="chipbtn mono ${r === dir ? 'on' : ''}" data-dir="${esc(r)}" title="${esc(r)}">${esc(r.split(/[\\/]/).slice(-2).join('\\'))}</button>`).join('')}</div>` : ''}
      <span class="fl">Folder</span>
      <div class="pick"><span class="mono" id="sd-dir" data-testid="sd-dir">${esc(dir || 'No folder chosen')}</span><button class="btn outline" id="sd-choose">${ic.folder} Choose folder…</button></div>
      ${SKIP[p.harness] ? `<label class="switch"><input type="checkbox" id="sd-skip" data-testid="sd-skip"> Skip permission prompts <span class="mono muted">${SKIP[p.harness]}</span></label>
        <span class="hint">${esc(p.harness_label)} then runs commands and edits files without asking first. Only for folders you trust.</span>` : ''}
      <label class="fl" for="sd-args">Extra arguments <span class="muted">(optional)</span></label>
      <input class="input mono" id="sd-args" placeholder='e.g. --model opus --append-system-prompt "Be brief"' data-testid="sd-args">
      <span class="hint">Added after the start command, for this terminal only. Separate with spaces; quote values that contain spaces.</span>
    </div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="sd-ok" data-testid="sd-ok" ${dir ? '' : 'disabled'}>${ic.play} Open terminal</button></span></div>`);
  const set = (v) => {
    dir = v;
    document.getElementById('sd-dir').textContent = v || 'No folder chosen';
    document.getElementById('sd-ok').disabled = !v;
    document.querySelectorAll('.sdlg .chipbtn').forEach((b) => b.classList.toggle('on', b.dataset.dir === v));
  };
  document.querySelectorAll('.sdlg .chipbtn').forEach((b) => (b.onclick = () => set(b.dataset.dir)));
  document.getElementById('sd-choose').onclick = async () => { const v = await pickFolder(); if (v) set(v); };
  document.getElementById('sd-ok').onclick = async () => {
    const extra = (document.getElementById('sd-args').value.match(/"[^"]*"|'[^']*'|\S+/g) || []).map((a) => a.replace(/^(["'])(.*)\1$/, '$2'));
    const skip = document.getElementById('sd-skip');
    const args = [...(skip && skip.checked ? [SKIP[p.harness]] : []), ...extra];
    if (await attempt(() => call('start', { name, cwd: dir, args }), `Opened ${label} in ${dir}`)) closeDialog();
  };
}
action('start-in-folder', (el) => startDialog(el.dataset.name));

/** A new session in a project's folder, or in a folder picked here when no project is given, as any account
 * of any tool; the project's latest account comes first. */
function newInProject(dir, project, preferred) {
  const tools = shownTools(S.state).map((h) => h.id);
  const ps = (S.state.profiles || []).filter((p) => tools.includes(p.harness) && (p.kind !== 'subscription' || p.identity.logged_in));
  let name = (ps.find((p) => p.name === preferred) || ps[0] || {}).name;
  openDialog(`${dlgHead(project ? 'New session in ' + project : 'New session')}
    <div class="sdlg">
      <span class="fl">Folder</span>
      <div class="pick"><span class="mono" id="np-dir" data-testid="np-dir">${esc(dir || 'No folder chosen')}</span>${project ? '' : `<button class="btn outline" id="np-choose" data-testid="np-choose">${ic.folder} Choose folder…</button>`}</div>
      <span class="fl">Account</span>
      ${ps.length ? `<div class="recent np-accts">${ps.map((p) => `<button class="chipbtn" data-name="${esc(p.name)}" data-testid="np-${esc(p.name)}">${toolIcon(p.harness, 14)} ${esc(p.label)}</button>`).join('')}</div>`
        : '<p class="empty">No account can start yet: add one, or log one in, on Accounts.</p>'}
      <div id="np-skipw" hidden><label class="switch"><input type="checkbox" id="np-skip" data-testid="np-skip"> Skip permission prompts <span class="mono muted" id="np-flag"></span></label>
        <span class="hint">The tool then runs commands and edits files without asking first. Only for folders you trust.</span></div>
    </div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="np-ok" data-testid="np-ok" ${name && dir ? '' : 'disabled'}>${ic.play} Start session</button></span></div>`);
  const pick = (n) => {
    name = n;
    const flag = SKIP[ps.find((x) => x.name === n).harness];
    document.querySelectorAll('.np-accts .chipbtn').forEach((b) => b.classList.toggle('on', b.dataset.name === n));
    document.getElementById('np-skipw').hidden = !flag;
    document.getElementById('np-flag').textContent = flag || '';
    document.getElementById('np-skip').checked = false;
  };
  document.querySelectorAll('.np-accts .chipbtn').forEach((b) => (b.onclick = () => pick(b.dataset.name)));
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
action('open-session', (el) => go('session', { profile: el.dataset.profile, id: el.dataset.id }));
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
    : `Harness Manager forgets <b>${esc(shown)}</b> and its saved key. Its sessions stay on disk unless you also delete its folder.
      <label class="switch" style="margin-top:12px"><input type="checkbox" id="rm-home" data-testid="remove-home"> Also delete its folder</label>`;
  openDialog(`${dlgHead('Remove ' + shown)}<div class="df" style="grid-template-columns:1fr"><div>${body}</div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn danger" id="rm-ok" data-testid="remove-confirm">Remove</button></span></div>`);
  document.getElementById('rm-ok').onclick = async () => {
    const del = !main && document.getElementById('rm-home').checked;
    closeDialog();
    if (await attempt(() => call('remove', { name, delete_home: del }), `Removed ${shown}`)) { go('accounts'); changed(); }
  };
}
action('remove-account', (el) => removeAccount(el.dataset.name, el.dataset.main === '1'));
action('account-menu', (el) => openMenu(el, [
  { label: 'Start in a folder…', run: () => startDialog(el.dataset.name) },
  { label: 'Log in again', run: () => startLogin(el.dataset.name) },
  { label: 'Open folder', run: () => call('open_folder', { path: el.dataset.home }) },
  { label: 'Profile settings', run: () => go('account', { name: el.dataset.name, tab: 'settings' }) },
  { label: 'Remove account…', danger: true, run: () => removeAccount(el.dataset.name, el.dataset.main === '1') },
]));
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

function addDialog() {
  const hs = shownTools(S.state);
  const h = hs.find((x) => x.id === add.harness) || hs[0];
  const kinds = [['subscription', 'Subscription', true], ['api_key', 'API key', h.caps.api_key], ['routed', 'Routed', h.caps.routable]];
  if (!kinds.find((k) => k[0] === add.kind && k[2])) add.kind = 'subscription';
  const keep = (id) => { const e = document.getElementById(id); return e ? e.value : ''; };
  const v = { name: keep('ad-name'), email: keep('ad-email'), key: keep('ad-key'), url: keep('ad-url'), model: keep('ad-model'), route: keep('ad-route') };
  const routes = S.state.routes || [];
  const kindLabel = { subscription: { claude: 'Pro, Max, Team or Enterprise. You sign in in the browser.', codex: 'ChatGPT Plus, Pro or Team. You sign in in the browser.', opencode: 'OpenCode signs in to its providers in a terminal.', agy: 'Google sign-in. Antigravity keeps one login at a time; Start swaps it.', pi: 'Pi signs in to a provider with /login inside the tool.', omp: 'Oh My Pi signs in to a provider with /login inside the tool.' }[h.id], api_key: 'Pay per use. The key is kept in Windows Credential Manager.', routed: 'Sent through the local router to a route of providers.' }[add.kind];
  const prefix = { claude: '.claude-', codex: '.codex-', opencode: '.opencode-', agy: '.agy-', pi: '.pi-', omp: '.omp-' }[h.id];
  const cliOf = (v) => `hrm add ${v.name || '<name>'}${h.id !== 'claude' ? ' --harness ' + h.id : ''}${add.kind === 'api_key' ? ' --api-key' : ''}${add.kind === 'routed' ? ' --route ' + (v.route || '<route>') : ''}${v.email ? ' --email ' + v.email : ''}`;
  const cli = cliOf(v);
  openDialog(`${dlgHead('Add account')}
    <div class="df"><label>Tool</label><div style="display:flex;gap:10px;align-items:center">${toolIcon(h.id, 24)}<select class="input" id="ad-harness" data-testid="add-harness">${hs.map((x) => `<option value="${x.id}" ${x.id === h.id ? 'selected' : ''} ${x.installed ? '' : 'disabled'}>${esc(x.label)}${x.installed ? '' : ' (not installed)'}</option>`).join('')}</select></div></div>
    <div class="dseg">${kinds.map(([k, l, ok]) => `<a class="${k === add.kind ? 'on' : ''} ${ok ? '' : 'off'}" data-kind="${k}" data-ok="${ok ? 1 : 0}" data-testid="kind-${k}">${l}</a>`).join('')}</div>
    <div class="df"><label>Type</label><div class="txt muted">${esc(kindLabel)}</div></div>
    <div class="df"><label for="ad-name">Name</label><div><input class="input" id="ad-name" value="${esc(v.name)}" placeholder="work-design" data-testid="add-name"><span class="mono hint">~\\${prefix}${esc(v.name || '<name>')}</span></div></div>
    ${add.kind === 'subscription' ? `<div class="df"><label for="ad-email">Email</label><div><input class="input" id="ad-email" value="${esc(v.email)}" placeholder="you@company.com" data-testid="add-email"><span class="hint">Checked after you log in, so a wrong account is caught</span></div></div>` : ''}
    ${add.kind === 'api_key' ? `<div class="df"><label for="ad-key">API key</label><div><input class="input" type="password" id="ad-key" value="${esc(v.key)}" data-testid="add-key"></div></div>
      <div class="df"><label for="ad-url">Endpoint</label><div><input class="input" id="ad-url" value="${esc(v.url)}" placeholder="Optional: an Anthropic-compatible base URL"><span class="hint">Leave empty for the vendor's own API</span></div></div>
      <div class="df"><label for="ad-model">Model</label><div><input class="input" id="ad-model" value="${esc(v.model)}" placeholder="Optional"></div></div>` : ''}
    ${add.kind === 'routed' ? `<div class="df"><label for="ad-route">Route</label><div>${routes.length ? `<select class="input" id="ad-route" data-testid="add-route">${routes.map((r) => `<option ${r.id === v.route ? 'selected' : ''}>${esc(r.id)}</option>`).join('')}</select>` : '<span class="txt muted">No routes yet. Create one under API keys &amp; routing.</span>'}</div></div>
      <div class="df"><label for="ad-model">Model</label><div><input class="input" id="ad-model" value="${esc(v.model)}" placeholder="Optional, e.g. hm-openrouter--z-ai/glm-5"></div></div>` : ''}
    ${h.id === 'claude' && add.kind === 'subscription' ? `<div class="df"><label>Kept in sync</label><div class="txt">CLAUDE.md, rules, skills, plugins and MCP servers, from your main profile</div></div>
      <div class="df"><label>Its own</label><div class="txt">Login, history and usage, so each account's numbers stay separate</div></div>` : ''}
    <div class="df"><label>CLI</label><div class="clibox mono">${esc(cli)}</div></div>
    <div class="dfoot"><span class="kb">Tab</span> next field <span class="kb">Esc</span> cancel<span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="ad-create" data-testid="add-create">${add.kind === 'subscription' ? 'Create and log in' : 'Create'}</button></span></div>`);
  document.getElementById('ad-harness').onchange = (e) => { add.harness = e.target.value; addDialog(); };
  document.querySelectorAll('.dseg a[data-kind]').forEach((a) => (a.onclick = () => { if (a.dataset.ok === '1') { add.kind = a.dataset.kind; addDialog(); } }));
  document.getElementById('ad-name').oninput = (e) => { document.querySelector('.hint.mono').textContent = '~\\' + prefix + (e.target.value || '<name>'); };
  document.querySelector('.dlg').addEventListener('input', () => { document.querySelector('.dlg .clibox').textContent = cliOf({ name: keep('ad-name'), email: keep('ad-email'), route: keep('ad-route') }); });
  document.getElementById('ad-create').onclick = async () => {
    const req = { name: document.getElementById('ad-name').value.trim(), harness: add.harness, kind: add.kind,
      email: keep('ad-email') || null, api_key: keep('ad-key') || null, base_url: keep('ad-url') || null, model: keep('ad-model') || null, route: keep('ad-route') || null };
    const created = await attempt(() => call('add', req));
    if (!created) return;
    closeDialog();
    await changed();
    if (add.kind !== 'subscription') return toast(`Added ${req.name}`);
    startLogin(req.name);
  };
}

/** Sign in: Claude in the dialog (link plus pasted code); other tools in a terminal window. */
async function startLogin(name) {
  const p = (S.state.profiles || []).find((x) => x.name === name) || { harness: add.harness };
  if (p.harness !== 'claude') {
    if (!(await attempt(() => call('login_terminal', { name })))) return;
    // the dialog watches the account's login every 2 s and says when the terminal's sign-in has landed
    const before = JSON.stringify(p.identity || null);
    const body = (msg, ok) => `${dlgHead('Sign in ' + acctName(name))}<div class="df" style="grid-template-columns:1fr"><div class="${ok ? 'ok' : ''}" data-testid="login-status">${msg}</div></div>
      <div class="dfoot"><span class="acts"><button class="btn ${ok ? 'primary' : ''}" data-act="login-done" data-testid="login-done">${ok ? 'Done' : 'Close'}</button></span></div>`;
    let timer;
    openDialog(body('A terminal window opened with the sign-in. Finish it there; this dialog follows along.', false), { onClose: () => clearInterval(timer) });
    timer = setInterval(async () => {
      const st = await call('state').catch(() => null);
      const now = st && st.profiles.find((x) => x.name === name);
      if (!now || JSON.stringify(now.identity) === before || !now.identity.logged_in) return;
      clearInterval(timer);
      S.state = st;
      const dlg = document.querySelector('.dlg');
      if (dlg) dlg.innerHTML = body(`Signed in${now.identity.email ? ' as ' + esc(now.identity.email) : ''}.`, true);
    }, 2000);
    return;
  }
  const flow = await attempt(() => call('login_start', { name }));
  if (!flow) return;
  let timer;
  const draw = (st) => {
    const dlg = document.querySelector('.dlg');
    if (!dlg) return;
    dlg.innerHTML = `${dlgHead('Sign in ' + acctName(name))}
      <div class="df"><label>Status</label><div class="txt ${st.state === 'failed' ? 'err' : st.state === 'done' ? (st.email_mismatch ? 'err' : 'ok') : ''}" data-testid="login-status">${esc(st.message)}</div></div>
      ${st.url && st.state === 'waiting' ? `<div class="df"><label>Link</label><div><button class="btn outline" id="lg-open">Open the sign-in page</button><span class="hint mono" style="overflow-wrap:anywhere">${esc(st.url)}</span></div></div>
        <div class="df"><label for="lg-code">Code</label><div style="display:flex;gap:8px"><input class="input" id="lg-code" placeholder="Paste the code or callback URL" data-testid="login-code"><button class="btn outline" id="lg-send" data-testid="login-send">Send</button></div></div>` : ''}
      <div class="dfoot"><span class="acts">${st.state === 'waiting' ? `<button class="btn" id="lg-cancel">Cancel sign-in</button>` : `<button class="btn primary" data-act="login-done" data-testid="login-done">Done</button>`}</span></div>`;
    const o = document.getElementById('lg-open');
    if (o) o.onclick = () => call('open_url', { url: st.url });
    const send = document.getElementById('lg-send');
    if (send) send.onclick = () => attempt(() => call('login_submit', { id: flow.id, code: document.getElementById('lg-code').value }), 'Code sent');
    const c = document.getElementById('lg-cancel');
    if (c) c.onclick = () => closeDialog();
  };
  // closed in any way (Cancel, X, Escape, outside click, Done): stop the sign-in process and forget the flow
  let closed = false;
  openDialog('', { onClose: () => { closed = true; clearInterval(timer); call('login_cancel', { id: flow.id }).catch(() => {}); } });
  draw(flow);
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
