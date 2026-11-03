// First-run walkthrough: on a page's first visit, a short tour highlights one part of the page at a time with
// a note beside it (Next / Back, or the arrow keys). Finishing a tour marks its page; Skip or Esc turns every
// page's tour off. Remembered on this computer; the ? button replays the current page's tour, and
// Settings > Show the tours again turns them all back on.

import { S, action } from './core.js';
import { esc } from './lib.js';

// [selector of the part to highlight (null: the middle of the window), title, explanation]
const TOURS = {
  home: [
    [null, 'Welcome to Harness Manager', 'It keeps every account of Claude Code, Codex, OpenCode and Antigravity on this computer in one place. This short tour shows each page the first time you open it.'],
    ['aside.side nav', 'Every page', 'Accounts to add and start accounts, Sessions to read past work, Usage, routing to other providers, and the MCP servers, skills and prompts your tools share.'],
    ['.ovkpis', 'Key numbers', 'Usage of every account against the period before, read from each tool\'s own logs on this computer; nothing is sent anywhere. Cost is what the tokens would cost at API list prices: subscriptions are not billed per token.'],
    ['.ovhead .acts', 'Tool and period', 'Show one tool or all of them, for the last 24 hours, 7 days or 30 days.'],
    ['[data-testid=routes]', 'Live routes', 'Each account, the tool it runs and where its requests go. Pulses show the tokens of the last five minutes; red is a paused provider or an account not logged in.'],
    ['[data-testid=burn]', 'Limit burn-down', 'Every open limit, tightest first. The mark is where it ends up at the reset at the pace so far.'],
    ['[data-testid=request-log]', 'Every model call', 'Newest first, with its account, model, tokens, cost and reply timing.'],
    ['[data-nav=home] .count', 'Today', 'Today\'s usage at API prices, always in view.'],
  ],
  accounts: [
    ['[data-testid=add-account]', 'Add an account', 'Each account gets its own folder and login. Sign in once; Harness Manager keeps the accounts apart.'],
    ['.tray', 'This week', 'Totals for all accounts, and when each account\'s limits free up again.'],
    ['[data-testid=accounts-table]', 'Your accounts, by tool', 'One card per tool, one row per account. "main" is the tool\'s own login on this computer. Each limit shows how much is left and when it resets.'],
    ['[data-testid=accounts-table] .acr .lims', 'Limits', 'Every limit of the plan: 5-hour, weekly and per model. The ↻ button beside a Claude account reads its limits from Anthropic right now.'],
    ['[data-testid=accounts-table] .go .btn', 'Start', 'Opens a terminal running the tool as this account. Log in appears instead when the account has not signed in yet.'],
    ['[data-testid=accounts-table] [data-act=account-menu]', 'More', 'Log in again, open the account\'s folder, its settings, or remove it.'],
    ['[data-testid=best-note]', 'The account with the most room', 'The account furthest from its limits says so: start your next session there.'],
  ],
  account: [
    ['.tabs2', 'Overview, Sessions, Projects, Settings', 'Plan, limits and usage; this account\'s sessions and projects; and its email, key or endpoint.'],
    ['[data-act=start-in-folder]', 'Start in a folder', 'Opens the account in the project folder you pick.'],
    ['.trow [data-act=account-menu]', 'More', 'Log in again, open its folder, or remove the account.'],
  ],
  sessions: [
    ['[data-testid=sessions-search]', 'Find a session', 'Search titles, folders and session ids across every account and tool.'],
    ['[data-testid=group]', 'Grouped by project', 'Sessions of the same repository sit together, whichever account or tool ran them. Open one to read it.'],
  ],
  session: [
    ['[data-testid=steps-panel]', 'Steps', 'Every tool call, folded: click one for its output or diff. All, Changes (the files it edited) or Failed.'],
    ['[data-testid=chat]', 'Conversation', 'Your prompts on the right, the answers on the left. An open session follows as its tool writes.'],
    ['[data-testid=resume]', 'Resume', 'Continues this session in a terminal, with the same account.'],
    ['[data-testid=export-md]', 'Export', 'Save the session as a Markdown file.'],
  ],
  usage: [
    ['[data-testid=usage-ledger]', 'Usage by account', 'Tokens per account for each of the last seven days, with the split by model and by project below.'],
  ],
  routing: [
    ['[data-testid=provider-add]', 'Providers', 'Add OpenRouter, Ollama or another provider with its key. Keys stay in Windows Credential Manager, never in a file.'],
    ['[data-testid=route-add]', 'Routes', 'A route lists providers: failover tries them in order, aggregate offers all their models. Add an account of kind Routed to use one.'],
    ['[data-testid=router-state]', 'The router', 'Runs on this computer only (127.0.0.1) and starts by itself when a routed account starts.'],
  ],
  mcp: [
    ['[data-testid=mcp-add]', 'Add an MCP server', 'Then tick the tools that should have it: it is written into every account of those tools. Servers you added by hand in a tool are never overwritten.'],
  ],
  skills: [
    ['[data-testid=skill-install]', 'Install a skill', 'From a folder or a GitHub link, then tick the tools that should have it. Skills are linked, so one update reaches every tool.'],
  ],
  prompts: [
    ['.tabs2', 'One list per tool', 'The instructions file of each tool: CLAUDE.md for Claude Code, AGENTS.md for Codex, and so on.'],
    ['[data-testid=prompt-new]', 'Versions', 'Keep several prompts and switch with Use this prompt. The file\'s current text is saved in the list before it is replaced.'],
  ],
  settings: [
    ['[data-testid=tip-term]', 'What each setting means', 'Point at ⓘ next to a setting for what it does and what its default means.'],
    ['[data-testid=settings-save]', 'Save', 'These settings belong to this computer.'],
    ['[data-testid=guides-reset]', 'Tours', 'Shows every page\'s tour again. The ? at the top replays the tour of the page you are on.'],
  ],
};

const key = (v) => 'hm.tour.' + v;
const OFF = 'hm.tours.off';
const seen = (v) => { try { return localStorage.getItem(OFF) === '1' || localStorage.getItem(key(v)) === '1'; } catch { return false; } };
const remember = (k) => { try { localStorage.setItem(k, '1'); } catch {} };

let tour = null; // { view, steps, i }

function visibleSteps(view) {
  return (TOURS[view] || []).filter(([sel]) => !sel || document.querySelector(sel));
}

function end(done = true, all = false) {
  if (tour && done) remember(all ? OFF : key(tour.view));
  tour = null;
  document.querySelectorAll('.tour-spot, .tour-pop').forEach((e) => e.remove());
}

function show() {
  document.querySelectorAll('.tour-spot, .tour-pop').forEach((e) => e.remove());
  const [sel, title, text] = tour.steps[tour.i];
  const target = sel && document.querySelector(sel);
  const last = tour.i === tour.steps.length - 1;
  const pop = document.createElement('div');
  pop.className = 'tour-pop';
  pop.setAttribute('role', 'dialog');
  pop.setAttribute('aria-label', title);
  pop.dataset.testid = 'tour';
  pop.innerHTML = `<div class="tour-step">${tour.i + 1} of ${tour.steps.length}</div><b>${esc(title)}</b><p>${esc(text)}</p>
    <div class="tour-acts"><button class="btn sm" data-act="tour-skip" data-testid="tour-skip" ${last ? '' : 'title="No more tours on any page; Settings can turn them back on"'}>${last ? 'Close' : 'Skip all tours'}</button><span>
    ${tour.i ? '<button class="btn outline sm" data-act="tour-back">Back</button>' : ''}
    <button class="btn primary sm" data-act="tour-next" data-testid="tour-next">${last ? 'Done' : 'Next'}</button></span></div>`;
  document.body.appendChild(pop);
  const W = window.innerWidth, H = window.innerHeight, pw = pop.offsetWidth, ph = pop.offsetHeight;
  if (target) {
    target.scrollIntoView({ block: 'nearest' });
    const r = target.getBoundingClientRect();
    const spot = document.createElement('div');
    spot.className = 'tour-spot';
    Object.assign(spot.style, { left: r.left - 6 + 'px', top: r.top - 6 + 'px', width: r.width + 12 + 'px', height: r.height + 12 + 'px' });
    document.body.appendChild(spot);
    // beside the part: below, else above, else to the right; always inside the window
    let top = r.bottom + 12 + ph <= H - 8 ? r.bottom + 12 : r.top - 12 - ph >= 8 ? r.top - 12 - ph : Math.min(H - ph - 8, Math.max(8, r.top));
    let left = r.bottom + 12 + ph <= H - 8 || r.top - 12 - ph >= 8 ? r.left : r.right + 16;
    pop.style.top = Math.max(8, top) + 'px';
    pop.style.left = Math.min(W - pw - 8, Math.max(8, left)) + 'px';
  } else {
    const spot = document.createElement('div');
    spot.className = 'tour-spot none';
    Object.assign(spot.style, { left: W / 2 + 'px', top: H / 2 + 'px', width: '0px', height: '0px' });
    document.body.appendChild(spot);
    pop.style.top = Math.max(8, (H - ph) / 2) + 'px';
    pop.style.left = Math.max(8, (W - pw) / 2) + 'px';
  }
  pop.querySelector('[data-act=tour-next]').focus();
}

/** Start the tour of a page (from the first visit, or the ? button). */
export function startTour(view) {
  const steps = visibleSteps(view);
  if (!steps.length) return false;
  tour = { view, steps, i: 0 };
  show();
  return true;
}

document.addEventListener('rendered', (e) => {
  const { view } = e.detail;
  if (tour && tour.view !== view) end(false); // left the page: its tour waits for the next visit
  if (tour) { tour.steps = visibleSteps(view); tour.i = Math.min(tour.i, tour.steps.length - 1); if (tour.i < 0) end(false); else show(); return; }
  if (!seen(view) && !document.getElementById('dialog-root').innerHTML) startTour(view);
});

action('tour-next', () => { if (!tour) return; if (tour.i < tour.steps.length - 1) { tour.i++; show(); } else end(); });
action('tour-back', () => { if (tour && tour.i > 0) { tour.i--; show(); } });
// Skip before the last step: the user does not want tours, on this page or any other
action('tour-skip', () => end(true, !!tour && tour.i < tour.steps.length - 1));
action('tour-start', () => { end(false); if (!startTour(S.route.view)) end(false); });

document.addEventListener('keydown', (e) => {
  if (!tour) return;
  if (e.key === 'Escape') { e.preventDefault(); e.stopImmediatePropagation(); document.querySelector('[data-act=tour-skip]')?.click(); }
  else if (e.key === 'ArrowRight') { e.preventDefault(); document.querySelector('[data-act=tour-next]')?.click(); }
  else if (e.key === 'ArrowLeft' && tour.i > 0) { e.preventDefault(); tour.i--; show(); }
}, true);
window.addEventListener('resize', () => { if (tour) show(); });

/** Show every page's tour again. */
export function resetGuides() {
  try { localStorage.removeItem(OFF); for (const v of Object.keys(TOURS)) localStorage.removeItem(key(v)); } catch {}
}
