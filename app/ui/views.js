// Sessions, the reading view, usage, routing, MCP servers, skills, prompts, settings and the CLI reference.

import { S, call, view, go, toast, attempt, acctName, fit, openDialog, closeDialog, dlgHead, confirmDialog, openMenu, action, render, pickFolder, saveFile, pickFile, shownTools, applyTheme } from './core.js';
import { resetGuides } from './guide.js';
import { esc, fmt, hm, when, dur, avatar, toolPill, toolIcon, ic, md, diffHtml, bars } from './lib.js';
import { sessionRow } from './accounts.js';

const TOOL = { claude: 'Claude Code', codex: 'Codex', opencode: 'OpenCode', agy: 'Antigravity' };

// ---------- sessions ----------
const sf = { group: 'project', harness: '', profile: '', days: '7', q: '' };

view('sessions', async (sheet, params) => {
  if (params.q !== undefined) sf.q = params.q;
  if (!S.state) await call('state').then((s) => (S.state = s));
  const filter = { harness: sf.harness || null, profile: sf.profile || null, days: sf.days ? +sf.days : null, query: sf.q || null };
  sheet.className = 'sheet';
  sheet.innerHTML = `<h1>Sessions</h1><p class="find">Reading sessions…</p>`;
  const groups = await call('sessions', { filter, group: sf.group });
  const all = groups.flatMap((g) => g.sessions);
  S.cache.sessionOrder = all.map((s) => ({ profile: s.profile, id: s.id }));
  const tools = [...new Set(all.map((s) => s.harness))].map((h) => TOOL[h]).join(', ').replace(/, ([^,]*)$/, ' and $1');
  const open = all.filter((s) => s.open).length;
  sheet.innerHTML = `
    <div class="trow sesshead"><div><h1>Sessions</h1><p class="find" data-testid="sessions-finding">Every tool's sessions in one place${tools ? ', from ' + esc(tools) : ''}.${open ? ` <b>${open} running now</b>: open one to follow it live.` : ''}</p></div>
      <div class="acts"><span class="muted mono sm">${all.length} ${all.length === 1 ? 'session' : 'sessions'}${sf.days ? ' · ' + sf.days + ' days' : ''} · ${groups.length} ${sf.group === 'project' ? (groups.length === 1 ? 'project' : 'projects') : 'groups'}</span>
      <button class="btn primary" data-act="session-new" data-testid="session-new">${ic.play} New session</button></div></div>
    <div class="fbar sbar"><label class="search"><span>${ic.search}</span><input id="s-q" placeholder="Search titles, prompts and answers" value="${esc(sf.q)}" data-testid="sessions-search"></label>
      <span class="lbl">Group by</span><span class="seg3">${[['project', 'Project'], ['date', 'Date'], ['account', 'Account'], ['tool', 'Tool']].map(([k, l]) => `<a class="${sf.group === k ? 'on' : ''}" data-act="s-group" data-v="${k}" data-testid="group-${k}">${l}</a>`).join('')}</span>
      <span class="seg3">${[['7', '7d'], ['30', '30d'], ['', 'All']].map(([v, l]) => `<a class="${sf.days === v ? 'on' : ''} mono" data-act="s-days" data-v="${v}" data-testid="days-${v || 'all'}">${l}</a>`).join('')}</span></div>
    ${groups.length ? groups.map((g) => { const k = sf.group + ':' + g.key, open = !sFold.has(k); return `<div class="grp ${open ? '' : 'shut'}" data-testid="group"><div class="gh"><h2 class="gname">${esc(g.name)}</h2>${g.root && !g.remote ? `<span class="mono">${esc(g.root)}</span>` : ''}${g.remote ? `<span class="rem">${esc(g.remote)}</span>` : ''}
      ${sf.group === 'project' ? `<button class="iconbtn" style="width:24px;height:24px" data-act="project-menu" data-key="${esc(g.key)}" data-name="${esc(g.name)}" aria-label="Rename or merge">${ic.dots}</button>` : ''}
      <span class="tot mono">${g.sessions.length} ${g.sessions.length === 1 ? 'session' : 'sessions'} · ${fmt(g.tokens)} tokens</span>${sf.group === 'project' && g.root ? `<button class="btn outline sm" data-act="project-new" data-dir="${esc(g.root)}" data-name="${esc(g.name)}" data-profile="${esc(g.sessions[0] ? g.sessions[0].profile : '')}" data-testid="project-new">${ic.play} New session</button>` : ''}<button class="fold" data-act="s-fold" data-k="${esc(k)}" aria-expanded="${open}" aria-label="${open ? 'Fold' : 'Open'} ${esc(g.name)}"><span class="chev" aria-hidden="true">${open ? '▾' : '▸'}</span></button></div>${open ? g.sessions.map(sessionRow).join('') : ''}</div>`; }).join('')
      : '<p class="empty">No sessions match.</p>'}`;
  const q = document.getElementById('s-q');
  q.onkeydown = (e) => { if (e.key === 'Enter') { sf.q = q.value; render(); } };
});
action('s-group', (el) => { sf.group = el.dataset.v; render(); });
action('project-menu', (el) => openMenu(el, [
  { label: 'Rename project', run: () => projectDialog(el.dataset.key, el.dataset.name, 'rename') },
  { label: 'Merge into another project', run: () => projectDialog(el.dataset.key, el.dataset.name, 'merge') },
  { label: 'Reset name', run: () => attempt(() => call('project_alias', { key: el.dataset.key }), 'Reset').then(render) },
]));
function projectDialog(key, name, mode) {
  openDialog(`${dlgHead(mode === 'rename' ? 'Rename project' : 'Merge project')}
    <div class="df"><label>Project</label><div class="txt">${esc(name)} <span class="hint mono">${esc(key)}</span></div></div>
    <div class="df"><label for="pj-v">${mode === 'rename' ? 'New name' : 'Merge into key'}</label><div><input class="input" id="pj-v" value="${mode === 'rename' ? esc(name) : ''}">${mode === 'merge' ? '<span class="hint">The key of the project to merge into, as shown under its name</span>' : ''}</div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="pj-ok">Save</button></span></div>`);
  document.getElementById('pj-ok').onclick = async () => {
    const v = document.getElementById('pj-v').value.trim();
    if (await attempt(() => call('project_alias', mode === 'rename' ? { key, name: v } : { key, merge_into: v }), 'Saved')) { closeDialog(); render(); }
  };
}

// ---------- read a session ----------
// a header, five numbers, and the conversation as a chat with each turn's steps (folded) where they happened;
// Changes and Failed list only those steps. open: the steps unfolded; more: the long runs shown in full;
// follow: stay at the end of the conversation as a live session grows
const rv = { mode: 'all', open: new Set(), more: new Set(), sessionKey: '', follow: false, top: 0 };
const secsOf = (ms) => (ms >= 1000 ? (ms / 1000).toFixed(1) + ' s' : ms + ' ms');

view('session', async (sheet, params) => {
  sheet.className = 'sheet';
  // a redraw of the shown session (live update, unfolding a step) keeps it on screen until the new one is ready
  if (rv.sessionKey !== params.profile + '/' + params.id || !sheet.querySelector('#chat')) sheet.innerHTML = '<p class="muted">Opening the session…</p>';
  const [t, ss] = await Promise.all([call('transcript', { profile: params.profile, id: params.id }), call('session_stats', { profile: params.profile, id: params.id }).catch(() => null)]);
  if (rv.sessionKey !== params.profile + '/' + params.id) { rv.sessionKey = params.profile + '/' + params.id; rv.open = new Set(); rv.more = new Set(); rv.follow = false; rv.top = 0; rv.fresh = true; }
  const m = t.meta;
  const steps = t.turns.flatMap((x) => x.steps).filter((st) => st.kind !== 'thinking');
  const changes = steps.filter((st) => st.change), failed = steps.filter((st) => st.status === 'error');
  const term = S.state && S.state.settings.terminal === 'console' ? 'a console window' : 'Windows Terminal';
  const tool = TOOL[m.harness] || m.harness;
  const project = m.project_name || (m.cwd || '').split(/[\\/]/).filter(Boolean).pop() || 'no folder';
  // one step: ✓ / ✗ / …, the tool, what it worked on, its time or its change; it unfolds to its output or diff
  const row = (st, k) => {
    const open = rv.open.has(k), bad = st.status === 'error', busy = st.status === 'running' || st.status === 'pending';
    const meta = st.change ? `+${st.change.added} −${st.change.removed}` : bad ? `failed${st.duration_ms ? ' · ' + secsOf(st.duration_ms) : ''}` : st.duration_ms ? secsOf(st.duration_ms) : busy ? 'running now' : '';
    const detail = st.change ? diffHtml(st.change.diff) : `<pre class="sout mono">${esc(st.error || st.result || 'No output.')}</pre>`;
    return `<div class="si ${open ? 'open' : ''}"><button class="sb" data-act="step-open" data-k="${k}" aria-expanded="${open}"><span class="ico ${bad ? 'negT' : busy ? 'warnT' : 'acT'}">${bad ? '✗' : busy ? '…' : '✓'}</span><span class="kind">${esc(st.tool)}</span><span class="what mono" title="${esc(st.input)}">${esc(st.input)}</span><span class="meta mono ${bad ? 'negT' : ''}">${esc(meta)}</span></button>${open ? `<div class="sdet">${detail}</div>` : ''}</div>`;
  };
  // a run of steps; a long one (thousands in a Codex run) shows its first and last until asked for all
  const run = (list, id, head, tail) => !list.length ? '' : `<div class="steps">${rv.more.has(id) || list.length <= head + tail + 2 ? list.map(([st, k]) => row(st, k)).join('')
    : list.slice(0, head).map(([st, k]) => row(st, k)).join('') + `<div class="si more"><a class="acT" data-act="more-steps" data-run="${id}" data-testid="more-steps">Show ${(list.length - head - tail).toLocaleString()} more steps</a></div>` + list.slice(-tail).map(([st, k]) => row(st, k)).join('')}</div>`;
  const files = new Set(changes.map((st) => st.change.path)).size;
  const who = (x) => `<span class="who mono">${esc(tool.toUpperCase())}${x.answer_at ? ' · ' + hm(x.answer_at) : ''}${x.model ? ' · ' + esc(x.model) : ''}</span>`;
  // All: each turn in order, the prompt, then the replies with the steps between them where they happened
  let k = 0;
  const chatAll = t.turns.map((x, ti) => {
    let out = x.prompt ? `<div class="msg you"><span class="who mono">YOU${x.prompt_at ? ' · ' + hm(x.prompt_at) : ''}</span><div class="tx">${esc(x.prompt)}</div></div>` : '';
    const at = x.steps.map((st) => (st.kind === 'thinking' ? -1 : k++));
    const pos = x.answer.map((_, i) => (x.answer_pos && x.answer_pos[i] != null ? x.answer_pos[i] : x.steps.length));
    let from = 0;
    const flush = (to) => { const l = x.steps.slice(from, to).map((st, i) => [st, at[from + i]]).filter(([, j]) => j >= 0); out += run(l, ti + ':' + from, 5, 5); from = to; };
    // replies with no step between them share one bubble
    const groups = [];
    x.answer.forEach((a, i) => { if (!groups.length || groups[groups.length - 1].p !== pos[i]) groups.push({ p: pos[i], a: [] }); groups[groups.length - 1].a.push(a); });
    for (const g of groups) { flush(Math.max(from, g.p)); out += `<div class="msg ai">${who(x)}<div class="tx ans">${g.a.map(md).join('')}</div></div>`; }
    flush(x.steps.length);
    return out;
  }).join('');
  // Changes and Failed: only those steps, as one list
  const picked = steps.map((st, j) => [st, j]).filter(([st]) => (rv.mode === 'changes' ? st.change : st.status === 'error'));
  const body = rv.mode === 'all' ? chatAll : run(picked, 'flat', 30, 10) || `<p class="empty">${rv.mode === 'changes' ? 'This session changed no files.' : 'Nothing failed.'}</p>`;
  sheet.innerHTML = `
    <div class="trow sesh"><div><a href="#sessions" class="crumb acT">← Sessions · ${esc(project)}</a><h1 data-testid="session-title">${esc(m.title || 'Untitled session')}</h1>
        <div class="smeta">${m.open ? `<span class="pill teal mono live">● LIVE${rv.follow ? ' · following' : ''}</span>` : ''}<span>${esc(acctName(m.profile))} · ${esc(tool)}</span>${m.model ? `<span class="mono">${esc(m.model)}</span>` : ''}
          <span>started ${esc(when(m.started_at))} · ${m.prompts} ${m.prompts === 1 ? 'turn' : 'turns'}</span><span class="mono">${fmt(m.tokens)} tokens${ss && ss.cost != null ? ' · $' + ss.cost.toFixed(2) : ''}</span></div></div>
      <div class="acts"><button class="btn" data-testid="export-md" data-act="export-md" data-profile="${esc(m.profile)}" data-id="${esc(m.id)}">Export as Markdown</button>
        <span class="splitb"><button data-act="resume" data-profile="${esc(m.profile)}" data-id="${esc(m.id)}" data-cwd="${esc(m.cwd)}" data-testid="resume">Resume in ${term}</button><button data-act="terminal-menu" aria-label="Choose terminal">${ic.caret}</button></span>
        <button class="iconbtn sq" data-act="session-menu" data-path="${esc(m.source_path)}" data-cwd="${esc(m.cwd)}" aria-label="More">${ic.dots}</button></div></div>
    <div class="ovkpis skpis">${[['Tokens', fmt(m.tokens)], ['Read from cache', ss && ss.requests ? Math.round(ss.hit * 100) + '%' : '—'], ['First reply p50', ss && ss.p50 != null ? (ss.p50 / 1000).toFixed(1) + ' s' : '—'],
      ['Failed commands', String(failed.length), failed.length ? 'negT' : ''], ['Files changed', String(files)]]
      .map(([l, v, c]) => `<div class="ovk"><div class="kl"><span>${l}</span></div><strong class="${c || ''}">${v}</strong></div>`).join('')}</div>
    <section class="panel convp" data-testid="conv-panel"><div class="ph"><h2>Conversation <span class="muted mono sm">${t.turns.length} ${t.turns.length === 1 ? 'turn' : 'turns'} · ${steps.length} steps · ${files} changed ${files === 1 ? 'file' : 'files'} · ${failed.length} failed</span></h2>
        <span class="seg3">${[['all', 'All'], ['changes', 'Changes'], ['failed', 'Failed']].map(([v, l]) => `<a class="${rv.mode === v ? 'on' : ''}" data-act="rv-mode" data-v="${v}" data-testid="mode-${v}">${l}</a>`).join('')}</span></div>
      <div class="chat" id="chat" data-testid="chat">${body || '<p class="empty">This session has no messages yet.</p>'}
        <div class="toend" hidden><button class="btn primary sm" data-act="follow-end" data-testid="follow-end">↓ Latest${m.open ? ' · follow live' : ''}</button></div></div></section>`;
  S.cache.transcript = t;
  // a live session opens at its end, following; a redraw keeps the reader's place, or the end while following
  const chat = document.getElementById('chat');
  if (rv.fresh) { rv.fresh = false; rv.follow = m.open; }
  chat.scrollTop = rv.follow ? chat.scrollHeight : rv.top;
  const atEnd = () => chat.scrollHeight - chat.scrollTop - chat.clientHeight < 40;
  const show = () => { chat.querySelector('.toend').hidden = atEnd(); };
  chat.addEventListener('scroll', () => { rv.follow = atEnd(); rv.top = chat.scrollTop; show(); }, { passive: true });
  show();
  watchLive(m);
});

// an open session redraws while its tool writes to it: the log file's size and time are checked every 2 s
let liveTimer;
export function watchLive(m) {
  clearInterval(liveTimer);
  if (!m.open || !m.source_path) return;
  const key = rv.sessionKey;
  let last = null;
  liveTimer = setInterval(async () => {
    if (S.route.view !== 'session' || rv.sessionKey !== key) return clearInterval(liveTimer);
    let stamp;
    try { stamp = JSON.stringify(await call('file_stamp', { path: m.source_path })); } catch { return; }
    if (last === null) { last = stamp; return; }
    if (stamp === last || document.getElementById('dialog-root').innerHTML) return;
    last = stamp;
    render();
  }, 2000);
}
action('follow-end', () => {
  rv.follow = true;
  const chat = document.getElementById('chat');
  if (chat) chat.scrollTop = chat.scrollHeight;
});
action('more-steps', (el) => { rv.more.add(el.dataset.run); render(); });
action('step-open', (el) => { const k = +el.dataset.k; if (rv.open.has(k)) rv.open.delete(k); else rv.open.add(k); render(); });
action('rv-mode', (el) => { rv.mode = el.dataset.v; render(); });
action('terminal-menu', (el) => openMenu(el, [
  { label: 'Open sessions in Windows Terminal', run: () => setTerminal('wt') },
  { label: 'Open sessions in a console window', run: () => setTerminal('console') },
]));
async function setTerminal(v) { if (await attempt(() => call('settings_set', { key: 'terminal', value: v }), 'Saved')) { S.state = await call('state'); render(); } }
action('session-menu', (el) => openMenu(el, [{ label: 'Open the log file folder', run: () => call('open_folder', { path: el.dataset.path.replace(/[\\/][^\\/]*$/, '') }) }]));
action('export-md', async (el) => {
  const path = await saveFile(`${(S.cache.transcript.meta.title || 'session').replace(/[^\w -]+/g, '').slice(0, 60)}.md`);
  if (path) attempt(() => call('export_markdown', { profile: el.dataset.profile, id: el.dataset.id, path }), 'Exported');
});

// session groups folded shut, by grouping and group
const sFold = new Set();
action('s-days', (el) => { sf.days = el.dataset.v; render(); });
action('s-fold', (el) => { const k = el.dataset.k; if (sFold.has(k)) sFold.delete(k); else sFold.add(k); render(); });

// ---------- usage ----------
let usageDays = 7;
// one colour per account, in the brand's order
const SERIES = ['var(--ac)', 'var(--iris)', '#5b6fb8', 'var(--sand)', 'var(--h3)', '#c084fc', '#94a3b8', '#f472b6'];

/** Tokens per day, each day's bar split by account. */
function stacked(days, series, w = 900) {
  const h = 200, padB = 22, n = days.length, gap = Math.max(4, Math.min(14, Math.round((w / n) * 0.18))), bw = (w - gap * (n - 1)) / n;
  const tot = days.map((_, i) => series.reduce((a, x) => a + x.vals[i], 0)), max = Math.max(1, ...tot);
  let svg = `<svg width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" style="display:block;max-width:100%" role="img" aria-label="Tokens per day, by account" data-testid="usage-chart">`;
  days.forEach((d, i) => {
    let y = h - padB;
    const x = i * (bw + gap);
    series.forEach((sr) => {
      const v = sr.vals[i]; if (!v) return;
      const bh = Math.max(1, (v / max) * (h - padB - 8));
      y -= bh;
      svg += `<rect x="${x.toFixed(1)}" y="${y.toFixed(1)}" width="${bw.toFixed(1)}" height="${bh.toFixed(1)}" style="fill:${sr.color}"><title>${esc(sr.name)} · ${esc(d)}: ${fmt(v)} tokens</title></rect>`;
      y -= 1;
    });
    const last = i === n - 1, step = Math.max(1, Math.round(n / 3));
    if (i === 0 || last || i % step === 0) svg += `<text x="${last ? x + bw : x}" y="${h - 6}" text-anchor="${last ? 'end' : 'start'}" font-size="11" style="fill:var(--m);font-family:var(--mono)">${last ? 'today' : esc(new Date(d + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short' }))}</text>`;
  });
  return svg + '</svg>';
}

view('usage', async (sheet) => {
  sheet.className = 'sheet';
  const u = await call('usage', { days: usageDays });
  const s = u.summary;
  const tool = (h) => ({ claude: 'Claude Code', codex: 'Codex', opencode: 'OpenCode', agy: 'Antigravity' }[h] || h);
  const order = u.accounts.map((b) => b.key).concat(Object.keys(s.by_profile).filter((k) => !u.accounts.some((b) => b.key === k)));
  const color = (k) => SERIES[order.indexOf(k) % SERIES.length];
  const series = order.filter((k) => s.by_profile[k]).map((k) => ({ name: acctName(k), color: color(k), vals: s.by_profile[k] }));
  const kind = (k) => { const p = (S.state && S.state.profiles || []).find((x) => x.name === k); return p && p.kind !== 'subscription' ? (p.kind === 'routed' ? ' · routed' : ' · key') : ''; };
  const top = u.models[0] ? u.models[0].tokens : 1;
  sheet.innerHTML = `
    <div class="trow"><div><h1>Usage by account</h1><p class="find">Tokens from each account's own logs, priced at API list prices. Subscriptions are not billed per token.</p></div>
      <div class="acts"><span class="seg3">${[7, 14, 30].map((d) => `<a class="${d === usageDays ? 'on' : ''} mono" data-act="u-days" data-v="${d}" data-testid="u-days-${d}">${d}d</a>`).join('')}</span>
      <button class="btn" data-act="u-csv" data-testid="u-csv">Export CSV</button></div></div>
    <section class="panel"><div class="ph"><h2>Tokens per day, by account</h2><div class="ulegend">${series.map((x) => `<span><i style="background:${x.color}"></i>${esc(x.name)}</span>`).join('')}</div></div>
      ${series.length ? '<div class="uchart"></div>' : '<p class="empty">No usage in this range.</p>'}</section>
    <section class="panel"><div class="ph"><h2>Accounts</h2></div>
      <table class="grid" data-testid="usage-ledger"><thead><tr><th>Account</th><th>Tool</th><th class="num">Tokens</th><th class="num">Input</th><th class="num">Output</th><th class="num">Cache read</th><th class="num">Requests</th><th class="num">List price</th></tr></thead><tbody>
      ${u.accounts.map((b) => `<tr><td><b class="usw"><i style="background:${color(b.key)}"></i>${esc(acctName(b.key))}</b></td><td>${esc(tool(b.harness))}${kind(b.key)}</td><td class="num">${fmt(b.tokens)}</td><td class="num">${fmt(b.input)}</td><td class="num">${fmt(b.output)}</td><td class="num">${fmt(b.cache_read)}</td><td class="num">${b.requests.toLocaleString()}</td><td class="num">${b.cost == null ? '—' : '$' + b.cost.toFixed(2)}</td></tr>`).join('') || '<tr><td colspan="8" class="empty">No usage yet.</td></tr>'}</tbody></table></section>
    <section class="panel"><div class="ph"><h2>By model</h2></div>
      ${u.models.map((m) => `<div class="umodel"><span class="mono">${esc(m.key)}</span><span class="mb"><i style="width:${Math.round((m.tokens / top) * 100)}%"></i></span><b class="mono">${fmt(m.tokens)}</b><span class="mono muted">${m.cost == null ? '—' : '$' + m.cost.toFixed(2)}</span></div>`).join('') || '<p class="empty">No usage yet.</p>'}</section>`;
  const ch = sheet.querySelector('.uchart');
  if (ch) fit(ch, (w) => stacked(s.days, series, w));
});
action('u-days', (el) => { usageDays = +el.dataset.v; render(); });
action('u-csv', async () => {
  const to = await saveFile(`Harness Manager usage ${new Date().toISOString().slice(0, 10)}.csv`, { name: 'CSV', extensions: ['csv'] });
  if (to) attempt(() => call('usage_csv', { days: usageDays, path: to }), 'Exported');
});

// ---------- routing ----------
view('routing', async (sheet) => {
  sheet.className = 'sheet';
  const r = await call('routing');
  const st = await call('state');
  S.state = st;
  const now = r.now, x = r.stats;
  const paused = (id) => r.breakers[id] && r.breakers[id].state === 'open';
  const nPaused = r.providers.filter((p) => paused(p.id)).length;
  const users = (route) => r.routed_profiles.filter((p) => p.route === route).map((p) => acctName(p.name));
  const pname = (id) => (r.providers.find((p) => p.id === id) || { name: id }).name;
  const secs = (ms) => (ms == null ? '—' : (ms / 1000).toFixed(1) + ' s');
  // a route's providers as a chain: serving, paused (with when it is tried again) or standing by
  const chain = (x2) => x2.providers.map((id, k) => {
    const b = r.breakers[id] || {}, isDefault = x2.mode === 'aggregate' && (x2.default_provider || x2.providers[0]) === id;
    const state = paused(id) ? `paused${b.retry_at && b.retry_at > now ? ' · retry ' + dur(b.retry_at - now) : ''}` : x2.mode === 'aggregate' ? (isDefault ? 'default' : 'hm-' + id + '--… models') : k === 0 || paused(x2.providers[k - 1]) ? 'serving' : 'standby';
    const tone = paused(id) ? 'bad' : state === 'serving' || state === 'default' ? 'ok' : '';
    return `${k ? '<span class="arr" aria-hidden="true">→</span>' : ''}<span class="pchip ${tone}"><b>${esc(pname(id))}</b><small class="mono">${esc(state)}</small></span>`;
  }).join('');
  sheet.innerHTML = `
    <div class="trow"><div><h1>API keys &amp; routing</h1><p class="find">Keys live only in Windows Credential Manager. The local router sends routed accounts to providers, moves to the next one when a provider fails, and pauses one that keeps failing. Subscription logins never pass through it.</p></div>
      <div class="acts"><button class="btn" data-act="provider-add" data-testid="provider-add">Add provider</button><button class="btn primary" data-act="route-add" data-testid="route-add" ${r.providers.length ? '' : 'disabled'}>Add route</button></div></div>
    <section class="panel rcard"><div class="rl"><i class="sdot ${r.running ? 'live' : ''}"></i><div><h2 data-testid="router-state">Local router · ${r.running ? 'running' : 'stopped'}</h2>
        <div class="mono muted sm">127.0.0.1:${r.port} · token required · ${r.routes.length} ${r.routes.length === 1 ? 'route' : 'routes'} · ${r.routed_profiles.length} ${r.routed_profiles.length === 1 ? 'account uses' : 'accounts use'} it</div>
        <div style="margin-top:14px">${r.running ? '<button class="btn" data-act="router-stop">Stop router</button>' : '<button class="btn" data-act="router-start" data-testid="router-start">Start router</button>'}</div></div></div>
      <div class="rstats"><div><span>Requests · 24 h</span><b class="mono">${x.requests.toLocaleString()}</b></div><div><span>Failovers</span><b class="mono ${x.failovers ? 'warnT' : ''}">${x.failovers}</b></div>
        <div><span>Errors</span><b class="mono ${x.errors ? 'negT' : ''}">${x.errors}</b></div><div><span>Paused providers</span><b class="mono ${nPaused ? 'negT' : ''}">${nPaused}</b></div></div></section>
    <h2 class="sech">Routes</h2>
    ${r.routes.length ? `<div class="rgrid" data-testid="routes">${r.routes.map((x2) => `<section class="panel route"><div class="ph"><h2 class="mono">${esc(x2.id)}</h2><span class="muted sm">${x2.mode === 'aggregate' ? 'aggregate' : 'failover'} · used by ${esc(users(x2.id).join(', ') || 'no account yet')}</span></div>
        <div class="pchain">${chain(x2)}</div>
        <p class="hint">${x2.mode === 'aggregate' ? 'One model list from all its providers, converted between the Anthropic and OpenAI formats.' : 'Tries each provider in order; one that fails five times in a row is skipped for a while.'}</p>
        <div class="racts"><button class="btn danger sm" data-act="route-remove" data-id="${esc(x2.id)}">Remove</button></div></section>`).join('')}</div>`
      : '<p class="empty">No routes yet. Add a provider, then a route that uses it.</p>'}
    <section class="panel"><div class="ph"><h2>Providers</h2></div>
    ${r.providers.length ? `<table class="grid" data-testid="providers"><thead><tr><th>Provider</th><th>Format</th><th>Base URL</th><th>Key</th><th>Breaker</th><th class="num">Requests · 24 h</th><th class="num">First chunk p95</th><th></th></tr></thead><tbody>${r.providers.map((p) => { const b = r.breakers[p.id] || {}, ps = x.providers[p.id] || {}; return `<tr><td><b>${esc(p.name)}</b> <span class="muted mono">${p.name === p.id ? '' : esc(p.id)}</span></td><td class="mono">${p.format}</td><td class="mono">${esc(p.base_url)}</td><td>${r.keys[p.id] ? 'saved' : '<span class="err">missing</span>'}</td><td>${paused(p.id) ? `<span class="pill neg">open · ${b.failures} failures</span>` : '<span class="pill teal">closed</span>'}</td><td class="num">${(ps.requests || 0).toLocaleString()}</td><td class="num">${secs(ps.p95_first)}</td><td class="num"><button class="btn danger sm" data-act="provider-remove" data-id="${esc(p.id)}">Remove</button></td></tr>`; }).join('')}</tbody></table>` : '<p class="empty">No providers yet. Add OpenRouter, DeepSeek or any Anthropic- or OpenAI-compatible endpoint.</p>'}</section>
    <section class="panel"><div class="ph"><h2>Recent requests through the router</h2></div>
    ${r.log.length ? `<table class="grid"><thead><tr><th>Time</th><th>Account</th><th>Route → provider</th><th>Model</th><th>Status</th><th class="num">First chunk</th><th class="num">Time</th><th class="num">Tokens</th></tr></thead><tbody>${r.log.map((l) => `<tr><td class="mono">${esc(hm(l.ts))}</td><td>${esc(acctName(l.profile))}</td><td class="mono">${esc(l.route)} → ${esc(pname(l.provider))}</td><td class="mono">${esc(l.model)}</td><td class="mono ${l.status === 200 ? 'acT' : 'negT'}">${l.status || 'failed'}</td><td class="num">${secs(l.first_ms)}</td><td class="num">${secs(l.ms)}</td><td class="num">${l.input + l.output ? fmt(l.input + l.output) : '—'}</td></tr>`).join('')}</tbody></table>` : '<p class="empty">No requests yet.</p>'}</section>`;
});
action('router-start', () => attempt(() => call('router_start'), 'Router started').then(render));
action('router-stop', () => attempt(() => call('router_stop'), 'Router stopped').then(render));
action('provider-add', () => {
  openDialog(`${dlgHead('Add provider')}
    <div class="df"><label for="pv-id">Id</label><div><input class="input" id="pv-id" placeholder="openrouter" data-testid="pv-id"></div></div>
    <div class="df"><label for="pv-name">Name</label><div><input class="input" id="pv-name" placeholder="OpenRouter"></div></div>
    <div class="df"><label for="pv-format">Format</label><div><select class="input" id="pv-format" data-testid="pv-format"><option value="anthropic">Anthropic Messages</option><option value="openai">OpenAI Chat Completions</option></select></div></div>
    <div class="df"><label for="pv-url">Base URL</label><div><input class="input" id="pv-url" placeholder="https://openrouter.ai/api/v1" data-testid="pv-url"><span class="hint">Anthropic: the root (…/v1/messages is added). OpenAI: up to /v1 (…/chat/completions is added).</span></div></div>
    <div class="df"><label for="pv-models">Models</label><div><input class="input" id="pv-models" placeholder="deepseek/deepseek-v4, z-ai/glm-5" data-testid="pv-models"><span class="hint">Comma separated. The first is used when a request names a model this provider lacks.</span></div></div>
    <div class="df"><label for="pv-key">Key</label><div><input class="input" type="password" id="pv-key" data-testid="pv-key"><span class="hint">Kept in Windows Credential Manager</span></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="pv-ok" data-testid="pv-save">Add provider</button></span></div>`);
  document.getElementById('pv-ok').onclick = async () => {
    const g = (id) => document.getElementById(id).value.trim();
    if (await attempt(() => call('provider_add', { id: g('pv-id'), name: g('pv-name'), format: g('pv-format'), base_url: g('pv-url'), models: g('pv-models'), key: g('pv-key') }), 'Provider added')) { closeDialog(); render(); }
  };
});
action('provider-remove', async (el) => { if (await confirmDialog('Remove provider', `Remove <b>${esc(el.dataset.id)}</b> and its saved key?`, 'Remove')) attempt(() => call('provider_remove', { id: el.dataset.id }), 'Removed').then(render); });
action('route-add', async () => {
  const r = await call('routing');
  openDialog(`${dlgHead('Add route')}
    <div class="df"><label for="rt-id">Id</label><div><input class="input" id="rt-id" placeholder="cheap" data-testid="rt-id"></div></div>
    <div class="df"><label for="rt-mode">Mode</label><div><select class="input" id="rt-mode" data-testid="rt-mode"><option value="failover">Failover: try providers in order</option><option value="aggregate">One model list across providers</option></select></div></div>
    <div class="df"><label>Providers</label><div>${r.providers.map((p, i) => `<label class="switch" style="display:flex;margin:4px 0"><input type="checkbox" value="${esc(p.id)}" class="rt-p" data-testid="rt-p-${esc(p.id)}" ${i === 0 ? 'checked' : ''}> ${esc(p.name)} <span class="muted mono">${esc(p.id)}</span></label>`).join('')}<span class="hint">Checked in this order; failover tries them top to bottom.</span></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="rt-ok" data-testid="rt-save">Add route</button></span></div>`);
  document.getElementById('rt-ok').onclick = async () => {
    const providers = [...document.querySelectorAll('.rt-p:checked')].map((c) => c.value);
    if (await attempt(() => call('route_add', { id: document.getElementById('rt-id').value.trim(), mode: document.getElementById('rt-mode').value, providers }), 'Route added')) { closeDialog(); render(); }
  };
});
action('route-remove', async (el) => { if (await confirmDialog('Remove route', `Remove route <b>${esc(el.dataset.id)}</b>?`, 'Remove')) attempt(() => call('route_remove', { id: el.dataset.id }), 'Removed').then(render); });

// ---------- MCP servers and skills ----------
export function syncReport(rep, item) {
  if (!rep) return;
  // entries read "<folder>: <name>"; with an item, only its own
  const mine = (l) => (l || []).filter((x) => !item || x.endsWith(': ' + item) || x.includes('skill ' + item + ' '));
  const n = mine(rep.written).length, c = mine(rep.conflicts), s = mine(rep.skipped);
  const copies = c.filter((x) => /\\\.claude-[^\\]+\\skills: /.test(x)).length;
  if (c.length) toast(`${item || 'Some items'} not linked into ${c.length} ${c.length === 1 ? 'folder' : 'folders'} that already ${c.length === 1 ? 'has' : 'have'} its own copy, so ${c.length === 1 ? 'it was' : 'they were'} left alone: ${c[0].replace(/: [^:]*$/, '')}${c.length > 1 ? ' and more' : ''}.${copies ? ' Settings > Link copies to the main profile fixes the Claude accounts\' old copies.' : ''}`, true);
  else if (s.length) toast(`Skipped: ${s[0]}`, true);
  else toast(n ? `Written to ${n} ${n === 1 ? 'place' : 'places'}` : 'Saved');
}

action('toggle-target', async (el) => {
  const rep = await attempt(() => call('target', { kind: el.dataset.kind, item: el.dataset.item, harness: el.dataset.h, on: el.checked }));
  if (rep === undefined) { el.checked = !el.checked; return; }
  syncReport(rep, el.dataset.item);
  // the column's count under its tool icon, and the row's Off tag, follow the ticks
  // (the skills list has no header row: its tools are chips)
  const table = el.closest('table'), small = table && table.tHead && table.tHead.rows[0].cells[el.closest('td').cellIndex]?.querySelector('small');
  if (small) small.textContent = table.querySelectorAll(`input[data-h="${el.dataset.h}"]:checked`).length;
  const th = table && table.tHead && table.tHead.rows[0].cells[el.closest('td').cellIndex];
  if (th && th.dataset.label) th.title = `${th.dataset.label}: ${table.querySelectorAll(`input[data-h="${el.dataset.h}"]:checked`).length} servers`;
  const row = el.closest('tr'), off = row.querySelector('.tag.off'), any = row.querySelector('input[type=checkbox]:checked');
  if (off && any) off.remove();
  else if (!off && !any && row.querySelector('.mn')) row.querySelector('.mn').insertAdjacentHTML('beforeend', '<span class="tag off" title="Not written to any tool">Off</span>');
});
action('item-remove', async (el) => {
  if (await confirmDialog('Remove ' + el.dataset.name, `Remove <b>${esc(el.dataset.name)}</b> from every tool Harness Manager wrote it into?`, 'Remove')) attempt(() => call('item_remove', { kind: el.dataset.kind, name: el.dataset.name })).then((rep) => { syncReport(rep); render(); });
});
action('import-from', async (el) => {
  const hs = shownTools(S.state || await call('state')).filter((h) => (el.dataset.kind === 'mcp' ? h.caps.mcp : h.caps.skills));
  openDialog(`${dlgHead(el.dataset.kind === 'mcp' ? 'Import MCP servers' : 'Import skills')}
    <div class="df"><label for="im-h">From</label><div><select class="input" id="im-h">${hs.map((h) => `<option value="${h.id}">${esc(h.label)} (main profile)</option>`).join('')}</select>
    <span class="hint">${el.dataset.kind === 'mcp' ? 'Its servers become shared and stay enabled for that tool.' : 'Its skill folders move to the shared store and are linked back.'}</span></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="im-ok">Import</button></span></div>`);
  document.getElementById('im-ok').onclick = async () => {
    const r = await attempt(() => call(el.dataset.kind === 'mcp' ? 'mcp_import' : 'skill_import', { harness: document.getElementById('im-h').value }));
    if (r) {
      const added = Array.isArray(r) ? r : r.added, kept = r.kept || [];
      closeDialog();
      toast(`Imported ${added.length}: ${added.join(', ') || 'nothing new'}${kept.length ? '. Kept your own ' + kept.join(', ') + ': a different one is installed' : ''}`);
      render();
    }
  };
});
action('skill-install', () => {
  openDialog(`${dlgHead('Install skill')}
    <div class="df"><label for="sk-src">Source</label><div style="display:flex;gap:8px"><input class="input" id="sk-src" placeholder="https://github.com/owner/repo/tree/main/skills/pdf or a folder" data-testid="sk-src"><button class="btn outline" id="sk-pick">Folder…</button></div></div>
    <div class="df"><label for="sk-name">Name</label><div><input class="input" id="sk-name" placeholder="Optional; the folder name otherwise"></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="sk-ok" data-testid="sk-save">Install</button></span></div>`);
  document.getElementById('sk-pick').onclick = async () => { const p = await pickFolder(); if (p) document.getElementById('sk-src').value = p; };
  document.getElementById('sk-ok').onclick = async () => {
    const r = await attempt(() => call('skill_install', { source: document.getElementById('sk-src').value.trim(), name: document.getElementById('sk-name').value.trim() }));
    if (r) { closeDialog(); toast(`Installed ${r.name}; tick the tools it should be in`); render(); }
  };
});
// ---------- prompts ----------
let promptTool = 'claude', promptSel = null;
view('prompts', async (sheet) => {
  sheet.className = 'sheet';
  const d = await call('items');
  const hs = shownTools(S.state || await call('state')).filter((h) => h.caps.prompts);
  const list = d.prompts.filter((p) => p.harness === promptTool);
  const live = await call('prompt_live', { harness: promptTool }).catch(() => null);
  const sel = list.find((p) => p.id === promptSel) || null;
  const FILE = { claude: 'CLAUDE.md', codex: 'AGENTS.md', opencode: 'AGENTS.md', agy: 'GEMINI.md' };
  const tool = (hs.find((h) => h.id === promptTool) || { label: promptTool }).label;
  const first = (b) => (b || '').split('\n').map((l) => l.replace(/^#+\s*/, '').trim()).find(Boolean) || 'Empty';
  const where = live ? `Writes to <span class="mono">${esc(live.path)}</span> · every ${esc(tool)} account follows it` : `No profile of ${esc(tool)} yet`;
  sheet.innerHTML = `<div class="trow"><div><h1>Prompts</h1><p class="find">A library of instructions per tool. Enabling one writes it into the tool's prompt file; what the file held before is saved here first, so nothing is lost.</p></div>
      <div class="acts"><button class="btn primary" data-act="prompt-new" data-testid="prompt-new">New prompt</button></div></div>
    <div class="tabs2">${hs.map((h) => `<a class="${h.id === promptTool ? 'on' : ''}" data-act="prompt-tool" data-v="${h.id}">${esc(h.label)} · ${FILE[h.id] || 'prompt'}</a>`).join('')}</div>
    <div class="pgrid"><div class="plist">
      <a class="${!sel ? 'on' : ''}" data-act="prompt-sel" data-id=""><span><b>The file now</b><small>${esc(live ? first(live.body) : 'not there yet')}</small></span><span class="pill mono">live</span></a>
      ${list.map((p) => `<a class="${sel && sel.id === p.id ? 'on' : ''}" data-act="prompt-sel" data-id="${esc(p.id)}" data-testid="prompt-${esc(p.id)}"><span><b>${esc(p.title)}</b><small>${esc(first(p.body))}</small></span>${p.enabled ? '<span class="pill teal mono">enabled</span>' : ''}</a>`).join('')}</div>
    <div class="peditor">${sel ? `<div class="ph"><input class="input ptitle" id="pr-title" value="${esc(sel.title)}" aria-label="Prompt name">${sel.enabled ? `<span class="pill teal mono">enabled in ${esc(tool)}</span>` : ''}</div>
      <p class="pwhere">${where}</p>
      <textarea class="input" id="pr-body" aria-label="Prompt text">${esc(sel.body)}</textarea>
      <div class="pfoot"><span class="muted sm">${sel.enabled ? 'This prompt is in the file now.' : 'Not in the file yet.'}</span><span class="acts">
        <button class="btn danger" data-act="prompt-delete" data-id="${esc(sel.id)}">Delete</button>
        <button class="btn" data-act="prompt-sel" data-id="">Show the live file</button>
        ${sel.enabled ? '' : `<button class="btn" data-act="prompt-enable" data-id="${esc(sel.id)}" data-testid="prompt-enable">Use this prompt</button>`}
        <button class="btn primary" data-act="prompt-save" data-id="${esc(sel.id)}">Save</button></span></div>`
      : `<div class="ph"><h2>The file now</h2><span class="pill mono">live</span></div><p class="pwhere">${where}</p>
        <div class="code" style="white-space:pre-wrap;min-height:360px">${esc(live ? live.body || '(the file is empty)' : 'No profile of this tool yet.')}</div>`}</div></div>`;
});
action('prompt-tool', (el) => { promptTool = el.dataset.v; promptSel = null; render(); });
action('prompt-sel', (el) => { promptSel = el.dataset.id || null; render(); });
action('prompt-new', async () => { const id = await attempt(() => call('prompt_add', { harness: promptTool, title: 'New prompt', body: '' })); if (id) { promptSel = id; render(); } });
action('prompt-save', (el) => attempt(() => call('prompt_save', { id: el.dataset.id, title: document.getElementById('pr-title').value, body: document.getElementById('pr-body').value }), 'Saved').then(render));
action('prompt-enable', async (el) => { if (!(await attempt(() => call('prompt_save', { id: el.dataset.id, title: document.getElementById('pr-title').value, body: document.getElementById('pr-body').value })))) return; attempt(() => call('prompt_enable', { id: el.dataset.id }), 'Written to the prompt file').then(render); });
action('prompt-delete', async (el) => { if (await confirmDialog('Delete prompt', 'Delete this prompt from the library? The prompt file is not changed.', 'Delete')) { if (await attempt(() => call('prompt_delete', { id: el.dataset.id }))) { promptSel = null; render(); } } });

// ---------- settings ----------
/** An ⓘ that explains a setting on hover or keyboard focus. */
const info = (id, html) => `<span class="tip"><button type="button" class="tipb" aria-label="About this setting" aria-describedby="tip-${id}" data-testid="tip-${id}">ⓘ</button><span class="bubble" role="tooltip" id="tip-${id}">${html}</span></span>`;

// a switch that saves as soon as it is flipped
const sw = (key, on, label, testid) => `<label class="switch"><input type="checkbox" data-act="set-now" data-key="${key}" ${on ? 'checked' : ''} data-testid="${testid}"> ${label}</label>`;
const seg = (key, cur, opts, testid) => `<span class="seg3" data-testid="${testid}">${opts.map(([v, l]) => `<a class="${cur === v ? 'on' : ''}" data-act="set-now" data-key="${key}" data-v="${v}">${l}</a>`).join('')}</span>`;
const NEW = '<span class="new">NEW</span>';
const row = (title, tip, text, ctl) => `<div class="frow"><div><b>${title}</b>${tip || ''}<p>${text}</p></div><div class="ctl">${ctl}</div></div>`;

view('settings', async (sheet) => {
  sheet.className = 'sheet';
  const st = await call('state');
  S.state = st;
  applyTheme(st);
  const s = st.settings;
  sheet.innerHTML = `<h1>Settings</h1><p class="find">Saved as you change them, for this computer. Keys and logins are never in these files or in backups.</p>
    <h5 class="sh5">Appearance</h5>
    ${row('Light and dark', info('theme', '<b>Dark (the default)</b> or <b>Light</b>: always that.<br><b>System:</b> follows Windows (Settings &gt; Personalization &gt; Colors).'), 'The colours of this window.', seg('theme', s.theme, [['system', 'System'], ['light', 'Light'], ['dark', 'Dark']], 'st-theme'))}
    ${row('Limits shown as', info('quota', '<b>Left (the default):</b> how much of each limit you still have, as the tools show it.<br><b>Used:</b> how much has gone.'), 'What is left of each limit, or what is used.', seg('quota_display', s.quota_display, [['left', 'Left'], ['used', 'Used']], 'st-quota'))}
    ${row('Moving routes' + NEW, info('motion', 'Pulses along the live routes, as fast as their traffic. Off, or with reduced motion in Windows, the routes are a still picture.<br><b>Default: on.</b>'), "The live particles on Overview. Off, or Windows' reduced motion, keeps them still.", sw('moving_routes', s.moving_routes, 'Animate the routes', 'st-motion'))}
    <h5 class="sh5">Window and terminal</h5>
    ${row('Close to the tray', info('tray', '<b>On (the default):</b> the window\'s X hides Harness Manager to the tray; usage and limits keep refreshing, and the tray icon opens it again. Quit from the tray menu.<br><b>Off:</b> the X quits.'), 'The window hides and the app keeps refreshing until you choose Quit.', sw('close_to_tray', s.close_to_tray, 'Keep running in the tray', 'st-tray'))}
    ${row('Launch at sign-in', info('auto', 'Starts Harness Manager in the tray when you sign in to Windows, so usage and limits are current when you open it.<br><b>Default: off.</b>'), 'Starts hidden in the tray.', `<label class="switch"><input type="checkbox" data-act="autostart" ${st.autostart ? 'checked' : ''} data-testid="st-autostart"> Start with Windows</label>`)}
    ${row('Terminal', info('term', '<b>Windows Terminal</b> (the default): Start and Resume open a new Windows Terminal window running the account.<br><b>Console window</b>: the classic console, for computers without Windows Terminal or if you prefer it.'), 'Where Start and Resume open a console.',
      seg('terminal', s.terminal, [['wt', 'Windows Terminal'], ['console', 'Console']], 'st-term'))}
    <h5 class="sh5">Updates</h5>
    ${row('Harness Manager', '', selfUpd && selfUpd.version ? `Version <b>${esc(selfUpd.version)}</b> is available; you have ${esc(st.version)}.${selfUpd.notes ? ' ' + esc(selfUpd.notes) : ''}` : `Version ${esc(st.version)}. New versions are looked for at launch and once a day.`,
      selfUpd && selfUpd.version ? '<button class="btn primary" data-act="self-install" data-testid="self-install">Update and restart</button>' : '<button class="btn outline" data-act="self-check" data-testid="self-check">Check for updates</button>')}
    ${row('New versions of your tools', info('upd', 'Asks npm for the newest Claude Code, Codex and OpenCode when Harness Manager starts; Apps in the sidebar shows how many have one.<br><b>Default: off.</b> Apps &gt; Check for updates looks at any time.'), 'Look for newer versions of the tools when the app starts.', sw('check_updates_at_launch', s.check_updates_at_launch, 'Check at launch', 'st-updcheck'))}
    <h5 class="sh5">Usage and limits</h5>
    ${row('Refresh', info('refresh', 'How often, in minutes, Harness Manager reads new usage from the tools\' logs and the cached limits, in the background.<br><b>Default: 5.</b> A smaller number updates sooner but reads the logs more often. <b>Sync now</b> on Home reads them at once whatever this is.'), 'How often usage and limits are read in the background.', seg('refresh_minutes', String(s.refresh_minutes), [['1', '1 min'], ['5', '5 min'], ['15', '15 min']], 'st-refresh'))}
    ${row('Live Claude limits', info('live', '<b>Off (the default):</b> limits are what Claude Code last saved, so they are as fresh as the account\'s last run; a window whose reset time has passed shows as empty.<br><b>On:</b> when those numbers are more than 10 minutes old, Harness Manager asks Anthropic for current ones with the login Claude Code saved, while it is valid. It never refreshes or copies that login.'), 'Off: limits are what Claude Code last wrote. On: Harness Manager also asks Anthropic, with the login Claude Code saved on this computer while it is valid. It never refreshes or copies that login.', `<label class="switch"><input type="checkbox" id="st-live" data-act="live-now" ${st.live_quota ? 'checked' : ''} data-testid="st-live"> Ask Anthropic for current limits</label>`)}
    ${row('First reply goal' + NEW, info('goal', 'Overview checks that 95% of replies start within this many seconds, from the times each tool logs.<br><b>Default: 20 s.</b>'), 'The time to a first reply you aim for.', `<select class="input" data-act="set-now" data-key="first_reply_goal_s" style="max-width:150px" aria-label="First-reply goal" data-testid="st-goal">${[5, 10, 15, 20, 30, 45, 60].map((v) => `<option value="${v}" ${s.first_reply_goal_s === v ? 'selected' : ''}>${v} seconds</option>`).join('')}</select>`)}
    ${row('Levels, XP and moves' + NEW, info('game', 'An efficiency score (cache hits, limit room, the first-reply goal and balance across accounts), a level from your use, and the next best moves.<br><b>Default: on.</b>'), 'The game layer on Overview.', sw('game_layer', s.game_layer, 'Show score, level and moves', 'st-game'))}
    <h5 class="sh5">Connections</h5>
    ${row('Router port', info('port', 'The port of the local router that routed accounts talk to. It listens on 127.0.0.1 only, so nothing outside this computer can reach it.<br><b>Default: 15731.</b> Change it only if another program uses that port; routed accounts use the new port the next time they start.'), 'The router listens on 127.0.0.1 only.', `<input class="input mono" type="number" id="st-port" value="${s.router_port}" style="max-width:110px" aria-label="Router port"><button class="btn outline" data-act="set-field" data-key="router_port" data-from="st-port" data-testid="st-port-save">Change</button>`)}
    ${row('Account list', info('reg', 'Where profiles.json lives: your accounts, providers, routes, MCP servers, skills and prompts. No keys or logins are ever stored in it.<br><b>Empty (the default):</b> it stays in this computer\'s Harness Manager folder (%APPDATA%\\HarnessManager). Enter a path in a OneDrive folder to share the same accounts with your other computer; each computer keeps its own keys and logins.'), 'Profiles, providers, routes, MCP servers, skills and prompts. No keys or logins are stored in it.', `<input class="input mono" id="st-reg" value="${esc(s.registry_path)}" placeholder="${esc(st.registry_path)}" aria-label="Account list path" style="min-width:300px"><button class="btn outline" data-act="set-field" data-key="registry_path" data-from="st-reg" data-testid="st-reg-save">Move…</button>`)}
    <h5 class="sh5">Diagnostic log</h5>
    ${row('Keep a log', info('log', 'What Harness Manager did and what went wrong (a usage file it could not read, a failed backup), with the time. It holds no keys or logins.<br><b>Default: on, Warnings.</b> Information also records backups, restores and the like.'), 'Kept in the data folder; nothing leaves this computer.', sw('log_enabled', s.log_enabled, 'Keep a log', 'st-log'))}
    ${row('Level', '', 'How much it records.', seg('log_level', s.log_level, [['error', 'Errors'], ['warn', 'Warnings'], ['info', 'Everything']], 'st-loglevel'))}
    ${row('Log file', '', 'Kept in the data folder; it holds no keys or logins.', `<button class="btn outline" data-act="log-open" data-testid="log-open">Open log</button>`)}
    <h5 class="sh5">Maintenance</h5>
    ${row('Find tools again', '', 'Adds a main profile for Claude Code, Codex, OpenCode or Antigravity when found.', '<button class="btn outline" data-act="detect">Find tools</button>')}
    ${row('Old launcher', '', 'Accounts and keys from <span class="mono">claude-accounts.json</span>. Keys can only be read on the computer that saved them.', '<button class="btn outline" data-act="import-ps">Import</button>')}
    ${row('Tours', '', 'The walkthrough each page shows the first time you open it. The ? at the top replays the tour of the page you are on.', '<button class="btn outline" data-act="guides-reset" data-testid="guides-reset">Show the tours again</button>')}
    <h5 class="sh5">Backups</h5>
    <div id="st-backups" data-testid="backups"><p class="muted">Reading backups…</p></div>`;
  // every section a card that folds; open by default, and its fields stay on the page while folded
  const [head, ...parts] = sheet.innerHTML.split('<h5 class="sh5">');
  sheet.innerHTML = head + parts.map((part) => {
    const i = part.indexOf('</h5>'), title = part.slice(0, i), id = title.toLowerCase().replace(/[^a-z]+/g, '-'), open = !setFold.has(id);
    const n = (part.match(/class="frow"/g) || []).length;
    return `<section class="panel sset ${open ? '' : 'shut'}" data-testid="sec-${id}"><h2 class="sh5"><button class="fold" data-act="set-fold" data-v="${id}" aria-expanded="${open}">${title}<span class="cnt">${n ? `${n} ${n === 1 ? 'setting' : 'settings'}` : ''}<span class="chev" aria-hidden="true">${open ? '▾' : '▸'}</span></span></button></h2><div class="sbody">${part.slice(i + 5)}</div></section>`;
  }).join('');
  drawBackups(s);
});
const setFold = new Set();
action('set-fold', (el) => {
  const id = el.dataset.v, sec = el.closest('.sset');
  if (setFold.has(id)) setFold.delete(id); else setFold.add(id);
  sec.classList.toggle('shut', setFold.has(id));
  el.setAttribute('aria-expanded', String(!setFold.has(id)));
  el.querySelector('.chev').textContent = setFold.has(id) ? '▸' : '▾';
});

const kb = (n) => (n >= 1048576 ? (n / 1048576).toFixed(1) + ' MB' : Math.max(1, Math.round(n / 1024)) + ' KB');
async function drawBackups(s) {
  const b = await call('backups').catch(() => null);
  const el = document.getElementById('st-backups');
  if (!b || !el) return;
  el.innerHTML = `${row('Data folder', '', 'Accounts list (unless it lives elsewhere), settings, the usage database, backups and the log.', `<span class="mono muted" style="overflow-wrap:anywhere">${esc(b.data)}</span> <button class="btn outline" data-act="open-folder" data-path="${esc(b.data)}">${ic.folder} Open</button>`)}
    ${row('Automatic backups', info('bk', 'A backup holds the account list, settings, what Harness Manager wrote into each tool, and the folders of the skills it manages. API keys and logins stay in Windows Credential Manager and are never in a backup.<br><b>Default: every 24 hours, keeping 10.</b>'), 'Made while the app runs, when the newest is older than this.',
      `<select class="input" data-act="set-now" data-key="backup_hours" style="max-width:170px" aria-label="Backup interval" data-testid="st-bkhours">${[[0, 'Off'], [6, 'Every 6 hours'], [12, 'Every 12 hours'], [24, 'Every 24 hours'], [168, 'Every week']].map(([v, l]) => `<option value="${v}" ${s.backup_hours === v ? 'selected' : ''}>${l}</option>`).join('')}</select>
       <select class="input" data-act="set-now" data-key="backup_keep" style="max-width:130px" aria-label="Backups kept">${[3, 5, 10, 20, 50].map((v) => `<option value="${v}" ${s.backup_keep === v ? 'selected' : ''}>Keep ${v}</option>`).join('')}</select>`)}
    <div class="bklist"><div class="bkhead"><b>Backups</b> <span class="muted">${b.list.length} · ${kb(b.list.reduce((a, x) => a + x.size, 0))}</span><span class="acts"><button class="btn outline sm" data-act="backup-import" data-testid="backup-import">Import…</button><button class="btn outline sm" data-act="backup-export" data-testid="backup-export">Export…</button><button class="btn sm primary" data-act="backup-now" data-testid="backup-now">${ic.download} Back up now</button></span></div>
      ${b.list.map((x) => `<div class="bkrow" data-testid="backup-${esc(x.label)}"><span class="mono">${esc(new Date(x.at).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' }))}</span><span>${esc(x.label)}</span><span class="muted">${kb(x.size)}</span>
        <span class="acts"><button class="iconbtn" data-act="backup-rename" data-name="${esc(x.name)}" data-label="${esc(x.label)}" title="Rename" aria-label="Rename">${ic.edit}</button><button class="iconbtn" data-act="backup-delete" data-name="${esc(x.name)}" title="Delete" aria-label="Delete">${ic.x}</button><button class="btn outline sm" data-act="backup-restore" data-name="${esc(x.name)}" data-testid="restore-${esc(x.label)}">${ic.refresh} Restore</button></span></div>`).join('') || '<p class="empty">No backups yet.</p>'}</div>`;
}
const restored = (r) => {
  const sk = r.restored.skills.length;
  toast(`Restored${sk ? `, with ${sk} skill ${sk === 1 ? 'folder' : 'folders'} put back` : ''}. The state before is kept as a backup named Before restore.`);
  S.state = null; render();
};
action('backup-now', async () => { if (await attempt(() => call('backup_create', { label: 'Manual' }), 'Backed up')) render(); });
action('backup-delete', async (el) => { if (await confirmDialog('Delete backup', 'Delete this backup? It cannot be undone.', 'Delete')) attempt(() => call('backup_delete', { name: el.dataset.name })).then(render); });
action('backup-rename', (el) => {
  openDialog(`${dlgHead('Rename backup')}<div class="df"><label for="bk-l">Label</label><div><input class="input" id="bk-l" value="${esc(el.dataset.label)}" maxlength="60"></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="bk-ok">Rename</button></span></div>`);
  document.getElementById('bk-ok').onclick = async () => { if (await attempt(() => call('backup_rename', { name: el.dataset.name, label: document.getElementById('bk-l').value }))) { closeDialog(); render(); } };
});
action('backup-restore', async (el) => {
  if (!(await confirmDialog('Restore this backup?', 'The account list, settings and what Harness Manager wrote into each tool go back to this backup, and skill folders missing from the store come back. The state now is kept as a backup first. API keys and logins are not touched.', 'Restore', false))) return;
  const r = await attempt(() => call('backup_restore', { name: el.dataset.name }));
  if (r) restored(r);
});
action('backup-export', async () => {
  const to = await saveFile(`Harness Manager backup ${new Date().toISOString().slice(0, 10)}.zip`, { name: 'Backup', extensions: ['zip'] });
  if (to) attempt(() => call('backup_export', { path: to }), 'Exported: keep it somewhere safe; it holds no keys or logins');
});
action('backup-import', async () => {
  const from = await pickFile({ name: 'Backup', extensions: ['zip'] });
  if (!from) return;
  if (!(await confirmDialog('Restore from this file?', `Everything in <span class="mono">${esc(from)}</span> is put back, as for a backup made here. The state now is kept as a backup first.`, 'Restore', false))) return;
  const r = await attempt(() => call('backup_import', { path: from }));
  if (r) restored(r);
});
action('log-open', () => attempt(() => call('log_open')));
// Harness Manager's own update: found at launch (the self-update event) or by Check for updates
let selfUpd = null;
export function setSelfUpdate(v) { selfUpd = v; }
action('self-check', async () => {
  const v = await attempt(() => call('self_update_check'));
  if (!v) return;
  selfUpd = v;
  if (!v.version) toast(`Harness Manager ${v.current} is the newest version`);
  render();
});
action('self-install', async (el) => {
  el.disabled = true;
  el.textContent = 'Updating…';
  // on success the app restarts, so only a failure comes back
  if (!(await attempt(() => call('self_update_install')))) { el.disabled = false; el.textContent = 'Update and restart'; }
});
action('set-now', async (el) => {
  // a switch's state, a segment's value, or a dropdown's choice (numbers as numbers)
  const raw = el.tagName === 'SELECT' ? el.value : el.dataset.v;
  const value = raw === undefined ? el.checked : /^\d+$/.test(raw) ? +raw : raw;
  if (await attempt(() => call('settings_set', { key: el.dataset.key, value }), 'Saved')) { S.state = null; render(); }
  else if (el.type === 'checkbox') el.checked = !el.checked;
});
action('live-now', async (el) => { if (!(await attempt(() => call('live_quota_set', { on: el.checked }), 'Saved'))) el.checked = !el.checked; });
action('autostart', async (el) => {
  const on = await attempt(() => call('autostart_set', { on: el.checked }));
  if (on === undefined) { el.checked = !el.checked; return; }
  toast(on ? 'Harness Manager starts in the tray when you sign in' : 'It no longer starts at sign-in');
});
action('guides-reset', () => { resetGuides(); toast('Each page will show its tour again'); render(); });
// a typed setting (the port, the account list's place): saved by the button beside it
action('set-field', async (el) => {
  const raw = document.getElementById(el.dataset.from).value.trim(), value = el.dataset.key === 'router_port' ? +raw : raw;
  if (await attempt(() => call('settings_set', { key: el.dataset.key, value }), 'Saved')) { S.state = null; render(); }
});
action('import-ps', async () => { const r = await attempt(() => call('import_ps', {})); if (r) { toast(`Imported ${r.accounts.length} accounts and ${r.keys.length} keys${r.skipped.length ? '; skipped ' + r.skipped.length : ''}`); } });
action('detect', async () => { const r = await attempt(() => call('detect', {})); if (r) toast(r.main_profiles.length ? `Added ${r.main_profiles.join(', ')}` : 'No new tools found'); });
action('relink', async () => { const r = await attempt(() => call('sync', { relink: true })); if (r) toast(r.relinked.length ? `Linked: ${r.relinked.join('; ')}` : 'Nothing to link; accounts are in step'); });

// ---------- CLI reference ----------
const CLI = [
  ['Accounts', [['list', 'Accounts with plan, limits, tokens and open consoles'], ['show', 'One account in detail'], ['add', 'Add an account (subscription login by default)'],
    ['login', 'Log an account in'], ['remove', 'Remove an account; --delete-home also deletes its folder'], ['key', 'API keys, kept in Windows Credential Manager']]],
  ['Run', [['run', 'Run the tool as an account in this console'], ['start', 'Open a new terminal running the account'], ['resume', 'Reopen a session in its own account'], ['open', 'Open the desktop app']]],
  ['Usage and limits', [['usage', 'Token usage per account and day'], ['overview', 'Cost at list prices, requests, tokens, cache hits'], ['requests', 'Every model reply, newest first'], ['quota', 'Limits of one account; --live asks Anthropic now']]],
  ['Sessions and projects', [['sessions', 'Sessions of every tool, grouped by project, date, account or tool'], ['session', 'Print one session as a transcript'], ['projects', 'List, rename or merge projects']]],
  ['Shared setup', [['sync', 'Bring every account in step'], ['mcp', 'MCP servers shared across tools'], ['skills', 'Skills shared across tools'], ['prompts', 'Prompt files per tool']]],
  ['Routing and setup', [['provider', 'Router providers: endpoints and keys'], ['route', 'Failover or aggregate routes'], ['router', 'Start, stop or check the local router'], ['settings', 'Read or change settings'],
    ['init', 'Find installed tools and add their main profiles'], ['import', 'Import from the old claude-accounts.ps1 launcher']]],
];
view('cli', async (sheet) => {
  sheet.className = 'sheet';
  const ex = [['hrm list', 'accounts, plans, limits, tokens, open consoles'], ['hrm start work-max --cwd D:\\Projects\\my-app', 'a new terminal, in that folder'],
    ['hrm quota work-max --live', 'limits straight from Anthropic'], ['hrm sessions --group project --json', "every tool's sessions, for scripts"]];
  const w = Math.max(...ex.map((e) => e[0].length)) + 2;
  sheet.innerHTML = `<h1>CLI reference</h1><p class="find"><span class="mono">hrm</span> does everything this window does. Add <span class="mono">--json</span> to any command for scripts.</p>
    <pre class="panel cliex mono">${ex.map(([c, d]) => `${esc(c.padEnd(w))}<span class="muted"># ${esc(d)}</span>`).join('\n')}</pre>
    <div class="cligrid">${CLI.map(([g, rows]) => `<section class="panel"><h2>${esc(g)}</h2>${rows.map(([c, d]) => `<div class="clir"><span class="mono acT">${esc(c)}</span><span>${esc(d)}</span></div>`).join('')}</section>`).join('')}</div>`;
});
