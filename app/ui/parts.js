// The parts every page shares (1.10), written once: the page head, the page names, the segmented choice, the pill,
// and the one rule for an account that needs attention. Nothing here reads the backend or the app's state, so every
// module (core.js too) can import it. Markup arguments are markup: esc() what you interpolate into them.

import { esc } from './lib.js';

/** Every view's stable name: the head's title, the lit item in the sidebar and the window's title. A detail view
 * (an account, a session, an MCP server being edited) carries the name of the page it belongs to. */
export const PAGE = {
  home: 'Overview', sessions: 'Sessions', session: 'Sessions', projects: 'Projects', accounts: 'Accounts', account: 'Accounts',
  computers: 'Computers', mcp: 'MCP servers', 'mcp-edit': 'MCP servers', skills: 'Skills', prompts: 'System prompts',
  routing: 'API keys & routing', apps: 'Apps', cli: 'CLI reference', settings: 'Settings',
};

const head = (lead, title, titleAttrs, status, statusCls, actions) => `<header class="page-head" data-testid="page-head">${lead ? `<div class="lead">${lead}</div>` : ''}<div class="grow"><h1 class="title" ${titleAttrs}>${title}</h1>${status ? `<div class="status ${statusCls}" data-testid="page-status">${status}</div>` : ''}</div>${actions ? `<div class="actions">${actions}</div>` : ''}</header>`;

/** The head of a page: its stable name (20 px), ONE status sentence under it (cut with … when the window is narrow),
 * its actions at the right. `view` is the page's id in PAGE ('accounts'); `status` is a plain sentence that reports
 * state ("8 accounts on 4 tools; 2 need attention."), or the plain fact when there is nothing to report ("No session
 * is running."), never a slogan; `actions` are the page's buttons (`.btn`, one `.btn.primary` at most). Both are markup. */
export const pageHead = (view, status, actions = '') => head('', esc(PAGE[view]), 'data-testid="page-title"', status, 'trunc', actions);

/** The head of a detail page: the "← Back" line, then the item's own name with its facts and actions.
 * `back` is [href, label] ("← label"); `title` is markup (an icon, a rename box); `titleAttrs` goes on the h1 (its
 * testid); `status` is the item's line of facts (it may wrap); `lead` is what stands before the name (an avatar). */
export const detailHead = ({ back, title, titleAttrs = '', status = '', actions = '', lead = '' }) => `<a class="filter-chip back-chip" href="${esc(back[0])}">← ${esc(back[1])}</a>${head(lead, title, titleAttrs, status, '', actions)}`;

/** A segmented choice, as the drawings draw it. `opts` is [[value, label]…], `cur` the chosen value, and `attrs(value)`
 * the attributes of each button (its data-act and data): seg('Text size', [['compact', 'Compact']…], cur, (v) => `data-act="set-now" data-key="text_size" data-v="${v}"`, 'st-textsize') */
export const seg = (label, opts, cur, attrs, testid = '') => `<div class="seg" role="group" aria-label="${esc(label)}" ${testid ? `data-testid="${esc(testid)}"` : ''}>${opts.map(([v, l]) => `<button type="button" class="${String(v) === String(cur) ? 'on' : ''}" aria-pressed="${String(v) === String(cur)}" ${attrs(v)}>${esc(l)}</button>`).join('')}</div>`;

/** A pill: a short state with its tone ('ok' healthy or selected, 'warn' pressure, 'bad' exhausted or failed, 'quiet', or '' neutral). `html` is markup. */
export const pill = (html, tone = '') => `<span class="pill ${tone}">${html}</span>`;

/** The small ▲ that goes with a count of things to act on (the Accounts badge in the sidebar). */
export const alertIcon = '<svg width="11" height="11" viewBox="0 0 12 12" aria-hidden="true"><path d="M6 1l5.2 9.5H.8z" fill="currentColor"/></svg>';

/** An account to act on: a subscription whose login expired or was never made, or a limit used up until its reset.
 * `p` is a profile of the state (an account view's `.p`). The sidebar's badge and the Accounts status count with it. */
export const needsAttention = (p, now = Date.now()) => (p.kind === 'subscription' && (!!p.identity.needs_login || !p.identity.logged_in))
  || ((p.quota && p.quota.tiers) || []).some((t) => t.resets_at > now && Math.round(t.used) >= 100);

/** "1 account", "3 accounts"; a third argument gives an irregular plural ("1 session is", "2 sessions are"). */
export const plural = (n, one, many = one + 's') => `${n.toLocaleString()} ${n === 1 ? one : many}`;
