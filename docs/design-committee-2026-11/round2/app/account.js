// Lead-owned registered page. All additional fixtures stay in this file.
(function () {
  const H = window.HM, D = H.data, esc = D.esc;
  H.data.accountDetails = {
    'service-account-resal': {
      cache: 72, p95: 2.4, p50: 1.1, replies: 412, sessions7d: 34,
      topProject: 'resal-wiki', topShare: 64, org: 'Resal', role: 'Member',
      tokens30: 9.8e6, cost30: 7.14,
      days: Array.from({length:30}, (_,i) => ({label:i === 29 ? 'Today' : new Date(D.NOW-(29-i)*D.DAY).toLocaleDateString('en-US',{month:'short',day:'numeric'}), tokens:(10 + ((i*13)%61))*1e6})),
      models30: [{model:'claude-sonnet-4-5',tokens:7.62e6,cost:5.14},{model:'claude-opus-4-6',tokens:2.18e6,cost:2.00}],
      projects: [{name:'resal-wiki',key:'D:\\Projects\\resal\\resal-wiki',tokens:1.76e6},{name:'HarnessManager',key:'D:\\Projects\\abushanab-net\\HarnessManager',tokens:.99e6}],
      openNow: [{sessionId:'s3',pid:48210,cwd:'D:\\Projects\\resal\\resal-wiki'}],
      recent: [{id:'recent-1',project:'resal-wiki',title:'Review deployment notes',when:'Yesterday',status:'done'},{id:'recent-2',project:'HarnessManager',title:'Repair profile import',when:'2 days ago',status:'failed'}]
    }
  };
  const fixture=H.data.accountDetails['service-account-resal'];
  const weekTokens=D.BASE_ACCOUNTS.find((a)=>a.id==='service-account-resal').tokens7d;
  for (const [start,end,total] of [[0,23,fixture.tokens30-weekTokens],[23,30,weekTokens]]) {
    const sum=fixture.days.slice(start,end).reduce((value,d)=>value+d.tokens,0);
    fixture.days.slice(start,end).forEach((d)=>d.tokens=d.tokens/sum*total);
  }
  function account(state) { return state.accounts.find((a) => a.id === state.detailAccountId); }
  const preview = (label, message) => `<button class="btn-secondary" data-action="toast-and-close" data-msg="${esc(message)}">${label}</button>`;
  function sessions(rows) {
    return rows.length ? rows.map((s) => `<div class="account-session"><div><b>${esc(s.title || s.project)}</b><div class="session-meta"><span>${esc(s.project)}</span><span>${esc(D.COMPUTERS.find((c) => c.id === s.computer)?.name || s.when || '')}</span><span>${esc(s.status)}</span>${s.branch ? `<span>${esc(s.branch)}</span>` : ''}</div></div>${s.computer ? `<button class="btn-follow" data-action="follow-session" data-session="${s.id}">Follow</button>` : `<button class="filter-chip" data-action="nav-page" data-page="session" data-session="${s.id}">Open</button>`}</div>`).join('') : '<div class="muted">No sessions in the last 30 days.</div>';
  }
  function settings(a, tool, state) {
    const field = (name, label, value, type='text') => `<div class="form-field"><label for="account-${name}">${label}</label><input id="account-${name}" data-field="${name}" type="${type}" value="${esc(state.accountDraft?.[a.id]?.[name] ?? value)}" oninput="window.HM.editAccount('${name}',this.value)"/></div>`;
    const row = (fields, save, title='Save') => `<div class="settings-row">${fields}<button class="btn-secondary" data-action="save-account" data-save="${save}">${title}</button></div>`;
    return `<div class="account-settings"><section class="panel-card"><h2 class="panel-title">Profile settings</h2>${row(field('label','Name',a.name),'label')}<span class="muted">Its folder and command line keep the id ${esc(a.id)}.</span>${row(field('email','Expected email',a.expectedEmail || '' ,'email'),'email')}<span class="muted">Checked after each login.</span></section>
    ${a.kind === 'apikey' ? `<section class="panel-card"><h2 class="panel-title">API key</h2>${row(field('key','New API key','','password'),'key','Replace key')}<span class="muted">Production stores keys in Windows Credential Manager. This preview keeps no credential.</span>${row(field('endpoint','Base URL',a.endpoint || '') + field('model','Model',a.defaultModel || ''),'endpoint')}<span class="muted">Leave empty for vendor defaults.</span></section>` : ''}
    ${a.kind === 'routed' ? `<section class="panel-card"><h2 class="panel-title">Daily spend cap</h2>${row(field('cap','Cap in USD',a.cap?.limitUsd ?? '', 'number'),'cap')}<span class="muted">At the cap, requests stop until midnight. Empty: no cap.</span></section>` : ''}
    ${a.tool === 'claude' && !a.isMain ? `<section class="panel-card"><h2 class="panel-title">Shared with the main profile</h2><span>${esc(tool.syncedItems)}</span>${preview('Sync now','Sync profile settings from the main profile')}</section>` : ''}
    <section class="panel-card"><h2 class="panel-title">${a.isMain ? 'Hide main profile' : 'Remove account'}</h2><span class="muted">${a.isMain ? 'Stops managing the tool’s profile. Its folder and login are never deleted.' : 'Its folder stays unless you choose to delete it.'}</span><button class="btn-secondary" data-action="confirm-remove" data-account="${a.id}">${a.isMain ? 'Hide main profile…' : 'Remove account…'}</button></section></div>`;
  }
  H.editAccount = (field, value) => {
    const state = H.state;
    state.accountDraft ||= {}; state.accountDraft[state.detailAccountId] ||= {};
    state.accountDraft[state.detailAccountId][field] = value;
  };
  H.saveAccount = (section) => {
    const state = H.state, a = account(state), draft = state.accountDraft?.[a.id] || {};
    if (section === 'label') {
      if (!(draft.label ?? a.name).trim()) { state.toastText = 'Enter a name.'; return; }
      a.name = (draft.label ?? a.name).trim();
    }
    if (section === 'email') {
      if (draft.email && !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(draft.email)) {state.toastText='Enter a valid email.';return;}
      a.expectedEmail = draft.email || '';
    }
    if (section === 'endpoint') {
      if (draft.endpoint && !/^https?:\/\//.test(draft.endpoint)) {state.toastText='Enter an http or https URL.';return;}
      a.endpoint = draft.endpoint ?? a.endpoint; a.defaultModel = draft.model ?? a.defaultModel;
    }
    if (section === 'key') {
      if (!draft.key?.trim()) { state.toastText='Paste a replacement API key.';return; }
      delete draft.key; const input=document.getElementById('account-key'); if(input) input.value='';
    }
    if (section === 'cap') {
      if (draft.cap && (!Number.isFinite(Number(draft.cap)) || Number(draft.cap)<=0)) {state.toastText='Enter a positive cap or leave it empty.';return;}
      a.cap = draft.cap ? {...a.cap, limitUsd:Number(draft.cap), spentUsd:a.cap?.spentUsd || 0, resetMs:8*D.H} : null;
    }
    // Persist preview edits on the active profile source, including first-run clones.
    const source = [...D.BASE_ACCOUNTS,...D.EXTRA_4_ACCOUNTS,...state.customAccounts].find((x)=>x.id===a.id);
    if (source) for (const key of ['name','expectedEmail','endpoint','defaultModel','cap']) source[key] = a[key];
    state.toastText = section === 'key' ? 'Key replacement preview; no credential was stored.' : 'Saved profile changes in this preview.';
  };
  H.pages.account = {
    title: 'Account',
    render(state) {
      const a=account(state);
      if(!a) return '<div class="page-body"><section class="panel-card empty-state"><h1>Account unavailable.</h1><button class="btn-secondary" data-action="nav-page" data-page="accounts">Back to Accounts</button></section></div>';
      const tool=D.TOOLS.find((t)=>t.id===a.tool), read=!state.metrics.isFresh && (a.tokens7d || 0)>0;
      const stats=read ? D.accountDetails[a.id] || {} : {};
      const active=state.metrics.activeSessions.filter((s)=>s.accountId===a.id), recent=[...active,...(stats.recent || [])];
      const stat=(title,value,note)=>`<div class="account-stat"><span>${title}</span><b>${value}</b><span>${note}</span></div>`;
      const tab=['overview','sessions','projects','settings'].includes(state.detailTab) ? state.detailTab : 'overview';
      const limits=(a.windows || []).map((w)=>`<div class="account-limit"><div class="account-limit-head"><b>${esc(w.label)}</b><span>${w.left===0?'Used up':w.left+'% left'} · resets in ${D.fmtDuration(w.resetMs)}</span></div><div class="win-track"><div class="win-fill" style="width:${w.used}%"></div>${w.forecastAtReset != null ? `<i class="limit-forecast" style="left:calc(${Math.min(100,w.forecastAtReset)}% - 1px)" title="≈ ${w.forecastAtReset}% used at reset"></i>` : ''}</div><span class="muted">${w.outMs != null && w.outMs<w.resetMs ? '≈ Runs out '+D.fmtAbsoluteReset(w.outMs)+' at this pace.' : '≈ Lasts until reset at this pace.'}</span></div>`).join('');
      const models=stats.models30 || [];
      const days=stats.days || [], max=Math.max(1,...days.map((d)=>d.tokens));
      return `<div class="page-body"><button class="filter-chip" style="align-self:flex-start" data-action="nav-page" data-page="accounts">← Accounts</button><header class="page-heading"><div class="account-identity"><div class="account-avatar" aria-hidden="true">${esc(a.name.split('-').map((v)=>v[0]).slice(0,2).join('').toUpperCase())}</div><div><h1>${D.LOGOS[a.tool]} ${esc(a.name)}</h1><div class="heading-facts"><span>${tool.name}</span><span>${esc(a.email)}</span><span class="plan-pill">${esc(a.plan)}</span><span>${stats.org ? esc(stats.org+' · '+stats.role) : ''}${a.isMain?' · main profile':''}</span></div><div class="muted">${active.length} consoles open${active.length ? ' in '+esc([...new Set(active.map((s)=>s.project))].join(', ')) : ''}</div></div></div><div class="page-actions"><button class="btn-secondary" data-action="open-start-folder" data-account="${a.id}">Start in a folder…</button><button class="btn-primary" data-action="start-account" data-account="${a.id}" ${a.needsLogin || !tool.installed ? 'disabled' : ''}>Start ${esc(a.name)}</button><button class="icon-btn" data-action="open-account-menu" data-account="${a.id}" title="Account actions">⋯</button></div></header>
      <nav class="page-tabs" aria-label="Account tabs">${['overview','sessions','projects','settings'].map((t)=>`<button class="filter-chip ${tab===t?'active':''}" aria-current="${tab===t?'page':'false'}" data-action="set-detail-tab" data-tab="${t}">${t[0].toUpperCase()+t.slice(1)}</button>`).join('')}</nav>
      ${tab==='settings' ? settings(a,tool,state) : tab==='sessions' ? `<section class="panel-card"><h2 class="panel-title">Sessions · ${recent.length}</h2>${sessions(recent)}</section>` : tab==='projects' ? `<section class="panel-card"><h2 class="panel-title">Projects · 7 days</h2>${stats.projects?.length ? `<table class="replies-table"><thead><tr><th>Project</th><th>Key</th><th class="num">Tokens</th></tr></thead><tbody>${stats.projects.map((p)=>`<tr><td><button class="filter-chip" data-action="nav-page" data-page="projects" data-project="${esc(p.name)}">${esc(p.name)}</button></td><td class="mono">${esc(p.key)}</td><td class="num">${D.fmtTokens(p.tokens)}</td></tr>`).join('')}</tbody></table>` : '<span class="muted">No usage this week.</span>'}</section>` : `
      <section class="account-stats" aria-label="Account readouts">${stat('Tokens · 7 days',read?D.fmtTokens(a.tokens7d):'—',stats.cache!=null?stats.cache+'% read from cache':'Cache share not read')}${stat('At list prices',read?D.fmtMoney(a.cost7d):'—',a.kind==='subscription'?'Subscription: not billed per token':'Billed per token')}${stat('First reply p95',stats.p95?'≈ '+stats.p95+'s':'—',stats.p50?'p50 ≈ '+stats.p50+'s · '+stats.replies+' replies':'No timed replies')}${stat('Sessions · 7 days',stats.sessions7d ?? '—',active.length+' running now')}${stat('Busiest project',stats.topShare?stats.topShare+'%':'—',esc(stats.topProject || 'No usage this week'))}</section>
      <div class="account-columns"><section class="panel-card"><div class="panel-head"><h2 class="panel-title">Limits</h2><span class="muted">${a.readAgoSec!=null?'read '+Math.max(1,Math.round(a.readAgoSec/60))+'m ago':'not read yet'}</span></div>${limits || `<span class="muted">${a.kind!=='subscription'?'API keys and routes have no plan limits.':!['claude','codex'].includes(a.tool)?tool.name+' has no limits to read.':'— Limits show up after this account runs on this computer.'}</span>`}${limits ? '<span class="muted">Bars: used share · dashed mark: ≈ level at reset.</span>' : ''}</section><section class="panel-card"><h2 class="panel-title">Login and folder</h2><dl class="facts-grid"><dt>Login</dt><dd>${a.needsLogin?'Expired':a.kind==='apikey'?'Key saved':'Valid'}</dd><dt>Folder</dt><dd>${preview(esc(a.folder),'Open '+a.folder)}</dd><dt>Started with</dt><dd class="mono">${esc(({claude:'CLAUDE_CONFIG_DIR',codex:'CODEX_HOME',opencode:'XDG_CONFIG_HOME',agy:'Its own login',pi:'PI_CODING_AGENT_DIR',omp:'PI_CODING_AGENT_DIR'})[a.tool])}</dd><dt>CLI</dt><dd class="mono">hrm start ${esc(a.id)}</dd></dl>${a.tool==='claude' ? `<span class="muted">${!a.isMain?'Follows main: CLAUDE.md, rules, skills, MCP · Own: login, history, usage.':''} ${a.kind==='subscription'?'Live limits off.':''}</span>` : ''}${a.kind==='subscription'?`<button class="btn-secondary" data-action="reauth-account" data-account="${a.id}">Log in again</button>`:''}</section></div>
      <div class="account-columns"><section class="panel-card"><div class="panel-head"><h2 class="panel-title">Tokens per day · 30 days</h2><span class="muted">${stats.tokens30?D.fmtTokens(stats.tokens30)+' · '+D.fmtMoney(stats.cost30)+' at list prices':'—'}</span></div>${days.length?`<div class="account-chart">${days.map((d,i)=>`<div class="stack-col ${i===29?'today':''}" title="${d.label}: ${D.fmtTokens(d.tokens)} tokens"><div class="stack-bars" style="height:${d.tokens/max*76}px"></div><span class="stack-lbl">${i===0||i===14||i===29?d.label:''}</span></div>`).join('')}</div>`:'<span class="muted">No usage yet.</span>'}</section><section class="panel-card"><h2 class="panel-title">By model · 30 days</h2>${models.length ? models.map((m)=>`<div class="win-bar-line"><span>${esc(m.model)}</span><div class="win-track"><div class="win-fill" style="width:${m.tokens/stats.tokens30*100}%"></div></div><span>${D.fmtTokens(m.tokens)}</span><span>${D.fmtMoney(m.cost)}</span></div>`).join(''):'<span class="muted">No usage yet.</span>'}</section></div>
      ${active.length?`<section class="panel-card"><h2 class="panel-title">Open now</h2>${active.map((s,i)=>`<div class="account-session"><div><span class="mono">${esc(stats.openNow?.find((row)=>row.sessionId===s.id)?.cwd || '— Folder not read')}</span><div class="muted">PID ${stats.openNow?.find((row)=>row.sessionId===s.id)?.pid ?? '—'} · since ${s.sinceMin}m · ${D.COMPUTERS.find((c)=>c.id===s.computer)?.name}</div></div>${preview('Show window','That console is in another window; switch to it from the taskbar.')}</div>`).join('')}</section>`:''}
      <section class="panel-card"><div class="panel-head"><h2 class="panel-title">Recent sessions</h2><button class="filter-chip" data-action="set-detail-tab" data-tab="sessions">All ${recent.length}</button></div>${sessions(recent.slice(0,5))}</section>`}</div>`;
    }
  };
})();
