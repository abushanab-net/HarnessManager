// API keys & routing, as the reference draws it: the finding, the router card with its four stats (failovers amber,
// errors and paused providers red, with the word beside the number), the routes as provider chains, the providers
// table and the recent requests; Add provider / Add route as dialogs. The old usage address still redirects.

import { S, call, view, go, attempt, mutate, changed, acctName, openDialog, closeDialog, dlgHead, confirmDialog, action, patch } from './core.js';
import { esc, fmt, hm, dur } from './lib.js';

// Usage by account lives on Accounts now; the old address still works
view('usage', async () => { go('accounts'); });

const panel = (title, body, testid = '') => `<section class="panel-card" ${testid ? `data-testid="${testid}"` : ''}><div class="panel-head"><h2 class="panel-title">${title}</h2></div>${body}</section>`;
const empty = (title, text = '') => `<div class="b-empty"><b>${esc(title)}</b><span class="muted">${esc(text)}</span></div>`;
const pill = (text, tone) => `<span class="plan-pill b-${tone}">${esc(text)}</span>`;
const secs = (ms) => (ms == null ? '—' : (ms / 1000).toFixed(1) + ' s');
const n = (c, one, many) => `${c.toLocaleString()} ${c === 1 ? one : many}`;

view('routing', async (sheet) => {
  sheet.className = 'sheet';
  const [r, st] = await Promise.all([call('routing'), call('state')]);
  S.state = st;
  const now = r.now, x = r.stats;
  const paused = (id) => !!(r.breakers[id] && r.breakers[id].state === 'open');
  const nPaused = r.providers.filter((p) => paused(p.id)).length;
  const users = (route) => r.routed_profiles.filter((p) => p.route === route).map((p) => acctName(p.name));
  const pname = (id) => (r.providers.find((p) => p.id === id) || { name: id }).name;
  // a route's providers as a chain: serving (only while the router runs), paused with when it is tried again, or standing by
  const chain = (x2) => x2.providers.map((id, k) => {
    const b = r.breakers[id] || {}, isDefault = x2.mode === 'aggregate' && (x2.default_provider || x2.providers[0]) === id;
    const state = paused(id) ? `paused${b.retry_at && b.retry_at > now ? ' · retry ' + dur(b.retry_at - now) : ''}` : x2.mode === 'aggregate' ? (isDefault ? 'default' : 'hm-' + id + '--… models') : r.running && (k === 0 || paused(x2.providers[k - 1])) ? 'serving' : 'standby';
    return `${k ? '<span aria-hidden="true">→</span>' : ''}<div class="candidate-pill ${paused(id) ? 'b-warn' : ''}"><b>${esc(pname(id))}</b><span>${esc(state)}</span></div>`;
  }).join('');
  const finding = r.running ? `${n(x.requests, 'request', 'requests')} in 24 h.${nPaused ? ` ${n(nPaused, 'provider', 'providers')} paused.` : x.errors ? ` ${n(x.errors, 'error', 'errors')}.` : ''}` : 'The local router is stopped.';
  const stat = (label, v, tone) => `<div data-key="stat-${label}"><span class="muted">${label}</span><b class="${v && tone ? 'b-' + tone : ''}">${v.toLocaleString()}${v && tone ? `<small> ${label === 'Paused providers' ? 'paused' : label.toLowerCase()}</small>` : ''}</b></div>`;
  patch(sheet, `<div class="page-body b-page" data-b-page="routing">
    <header class="page-heading"><div><h1>${finding}</h1><div class="heading-facts">127.0.0.1:${r.port} · token required · keys stay in Windows Credential Manager · subscription logins stay outside the router</div></div>
      <div class="page-actions"><button class="btn-secondary" data-act="provider-add" data-testid="provider-add">Add provider</button><button class="btn-primary" data-act="route-add" data-testid="route-add" ${r.providers.length ? '' : 'disabled'}>Add route</button></div></header>
    <section class="panel-card"><div class="panel-head"><h2 class="panel-title" data-testid="router-state">Local router · ${r.running ? 'running' : 'stopped'}</h2></div>
      <div class="b-router-stats">${stat('Requests · 24 h', x.requests)}${stat('Failovers', x.failovers, 'warn')}${stat('Errors', x.errors, 'danger')}${stat('Paused providers', nPaused, 'danger')}</div>
      <div class="b-toolbar"><span class="muted">${n(r.routes.length, 'route', 'routes')} · ${n(r.routed_profiles.length, 'account uses', 'accounts use')} it</span>${r.running ? '<button class="btn-secondary" data-act="router-stop">Stop router</button>' : '<button class="btn-secondary" data-act="router-start" data-testid="router-start">Start router</button>'}</div></section>
    ${panel('Routes', r.routes.map((x2) => `<div class="b-route" data-key="route-${esc(x2.id)}"><div class="panel-head"><b class="mono">${esc(x2.id)}</b><button class="btn-secondary" data-act="route-remove" data-id="${esc(x2.id)}" data-users="${esc(users(x2.id).join(', '))}">Remove…</button></div>
        <div class="muted">${x2.mode} · used by ${esc(users(x2.id).join(', ') || 'no account yet')}</div><div class="b-chain">${chain(x2)}</div>
        <details><summary>How ${x2.mode} works</summary><span class="muted">${x2.mode === 'aggregate' ? 'One model list from all its providers, converted between the Anthropic and OpenAI formats.' : 'Tries each provider in order; one that fails five times in a row is skipped for a while.'}</span></details></div>`).join('') || empty('No routes yet.', 'Add a provider, then a route that uses it.'), 'routes')}
    ${panel('Providers', r.providers.length ? `<div class="table-wrap"><table class="replies-table b-providers"><thead><tr><th>Provider</th><th>Format / endpoint</th><th>Key</th><th>Breaker</th><th class="num">Requests · 24 h</th><th class="num">First chunk p95</th><th></th></tr></thead><tbody>${r.providers.map((p) => { const b = r.breakers[p.id] || {}, ps = x.providers[p.id] || {}; return `<tr data-key="provider-${esc(p.id)}"><td><b>${esc(p.name)}</b>${p.name === p.id ? '' : `<div class="muted">${esc(p.id)}</div>`}</td><td>${esc(p.format)}<div class="mono b-command" title="${esc(p.base_url)}">${esc(p.base_url)}</div></td><td>${r.keys[p.id] ? pill('Saved', 'good') : pill('Missing', 'bad')}</td><td>${paused(p.id) ? pill(`Open · ${b.failures} failures`, 'bad') : pill('Closed', 'good')}</td><td class="num">${(ps.requests || 0).toLocaleString()}</td><td class="num">${secs(ps.p95_first)}</td><td><button class="btn-secondary" data-act="provider-remove" data-id="${esc(p.id)}">Remove…</button></td></tr>`; }).join('')}</tbody></table></div>` : empty('No providers yet.', 'Add OpenRouter, DeepSeek or any Anthropic- or OpenAI-compatible endpoint.'), 'providers')}
    ${panel('Recent requests through the router', r.log.length ? `<div class="table-wrap"><table class="replies-table"><thead><tr><th>Time</th><th>Account</th><th>Route → provider</th><th>Model</th><th>Status</th><th class="num">First chunk</th><th class="num">Time</th><th class="num">Tokens</th></tr></thead><tbody>${r.log.map((l) => `<tr><td>${esc(hm(l.ts))}</td><td>${esc(acctName(l.profile))}</td><td>${esc(l.route)} → ${esc(pname(l.provider))}</td><td class="mono">${esc(l.model)}</td><td class="${l.status === 200 ? 'b-good' : 'b-danger'}">${l.status || 'failed'}</td><td class="num">${secs(l.first_ms)}</td><td class="num">${secs(l.ms)}</td><td class="num">${l.input + l.output ? fmt(l.input + l.output) : '—'}</td></tr>`).join('')}</tbody></table></div>` : empty('No requests yet.'))}
  </div>`);
});
action('router-start', () => mutate(() => call('router_start'), 'Router started'));
action('router-stop', () => mutate(() => call('router_stop'), 'Router stopped'));

const field = (id, label, extra = '', hint = '') => `<div class="form-field"><label for="${id}">${label}</label><input id="${id}" ${extra}>${hint ? `<span class="muted">${hint}</span>` : ''}</div>`;
const foot = (okId, okLabel) => `<div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" id="${okId}" data-testid="${okId}">${okLabel}</button></span></div>`;
// known providers fill the endpoint and format; Custom leaves them to type
const PRESETS = { openrouter: ['OpenRouter', 'openai', 'https://openrouter.ai/api/v1'], deepseek: ['DeepSeek', 'openai', 'https://api.deepseek.com/v1'], custom: ['', 'anthropic', ''] };
action('provider-add', () => {
  openDialog(`${dlgHead('Add provider')}
    <div class="form-field"><label for="pv-preset">Provider</label><select id="pv-preset" data-act="pv-preset"><option value="custom">Custom endpoint</option><option value="openrouter">OpenRouter</option><option value="deepseek">DeepSeek</option></select></div>
    ${field('pv-id', 'Id', 'placeholder="openrouter" data-testid="pv-id"')}
    ${field('pv-name', 'Name', 'placeholder="OpenRouter"')}
    <div class="form-field"><label for="pv-format">Format</label><select id="pv-format" data-testid="pv-format"><option value="anthropic">Anthropic Messages</option><option value="openai">OpenAI Chat Completions</option></select></div>
    ${field('pv-url', 'Base URL', 'type="url" placeholder="https://openrouter.ai/api/v1" data-testid="pv-url"', 'Anthropic: the root (…/v1/messages is added). OpenAI: up to /v1 (…/chat/completions is added).')}
    ${field('pv-models', 'Models (comma separated)', 'placeholder="deepseek/deepseek-v4, z-ai/glm-5" data-testid="pv-models"', 'The first is used when a request names a model this provider lacks.')}
    ${field('pv-key', 'Key', 'type="password" autocomplete="off" data-testid="pv-key"', 'Kept in Windows Credential Manager, never in a file.')}
    ${foot('pv-save', 'Add provider')}`, { label: 'Add provider' });
  document.getElementById('pv-save').onclick = async () => {
    const g = (id) => document.getElementById(id).value.trim();
    if (await attempt(() => call('provider_add', { id: g('pv-id'), name: g('pv-name'), format: g('pv-format'), base_url: g('pv-url'), models: g('pv-models'), key: g('pv-key') }), 'Provider added')) { closeDialog(); changed(); }
  };
});
action('pv-preset', (el) => {
  const [name, format, url] = PRESETS[el.value], set = (id, v) => { document.getElementById(id).value = v; };
  set('pv-id', el.value === 'custom' ? '' : el.value); set('pv-name', name); set('pv-format', format); set('pv-url', url);
});
action('provider-remove', async (el) => { if (await confirmDialog('Remove provider', `Remove <b>${esc(el.dataset.id)}</b> and its saved key?`, 'Remove')) mutate(() => call('provider_remove', { id: el.dataset.id }), 'Removed'); });
action('route-add', async () => {
  const r = await call('routing');
  openDialog(`${dlgHead('Add route')}
    ${field('rt-id', 'Id', 'placeholder="cheap" data-testid="rt-id"')}
    <div class="form-field"><label for="rt-mode">Mode</label><select id="rt-mode" data-testid="rt-mode"><option value="failover">Failover: try providers in order</option><option value="aggregate">One model list across providers</option></select></div>
    <div class="form-field"><label>Providers</label>${r.providers.map((p, i) => `<label class="b-switch"><input type="checkbox" value="${esc(p.id)}" class="rt-p" data-testid="rt-p-${esc(p.id)}" ${i === 0 ? 'checked' : ''}> ${esc(p.name)} <span class="muted">${esc(p.id)}</span></label>`).join('')}<span class="muted">Checked in this order; failover tries them top to bottom.</span></div>
    ${foot('rt-save', 'Add route')}`, { label: 'Add route' });
  document.getElementById('rt-save').onclick = async () => {
    const providers = [...document.querySelectorAll('.rt-p:checked')].map((c) => c.value);
    if (await attempt(() => call('route_add', { id: document.getElementById('rt-id').value.trim(), mode: document.getElementById('rt-mode').value, providers }), 'Route added')) { closeDialog(); changed(); }
  };
});
action('route-remove', async (el) => {
  const who = el.dataset.users;
  if (await confirmDialog('Remove route', `Remove route <b>${esc(el.dataset.id)}</b>?${who ? ` <b>${esc(who)}</b> ${who.includes(',') ? 'use' : 'uses'} it and cannot start until given another route in its settings.` : ''}`, 'Remove')) mutate(() => call('route_remove', { id: el.dataset.id }), 'Removed');
});
