// The page tour, started from the ? in the top bar: it highlights one part of the page at a time with
// a note beside it (Next / Back, or the arrow keys). Finishing a tour marks its page; Skip or Esc turns every
// page's tour off. Remembered on this computer; the ? button replays the current page's tour, and
// Settings > Show the tours again turns them all back on.

import { S, action } from './core.js';
import { esc } from './lib.js';

// [selector of the part to highlight (null: the middle of the window), title, explanation]
const TOURS = {
  home: [
    [null, 'Welcome to Harness Manager', 'It keeps every account of Claude Code, Codex, OpenCode, Antigravity, Pi and Oh My Pi on this computer in one place. The ? at the top shows this tour of a page.'],
    ['aside.rail nav', 'Every page', 'Accounts to add and start accounts (with their usage), Projects and Sessions to read past work, routing to other providers, and the MCP servers, skills and system prompts shared between accounts.'],
    ['[data-testid=use-next]', 'Use next', 'The account to start on now: the most room, or the 5-hour window that resets soonest. Start opens a terminal on it; Folder picks where.'],
    ['[data-testid=finding]', 'Runs out first', 'The account on pace to run out before its reset, with the measured basis under it. Measured numbers are plain; estimates carry ≈.'],
    ['[data-testid=running-now]', 'Running now', 'Every console open, on which computer; Follow opens its session as it writes.'],
    ['[data-testid=horizon]', 'Reset Horizon', 'Each account as a lane: its week ring, the capacity left in each window, and the time to each reset on a 7-day scale (the 5-hour window on its own zoom). Start appears on hover.'],
    ['[data-testid=kpi-tokens]', 'Key numbers', 'Tokens, cost and the share read from cache, each with its trend and its change against the period before: today, 7 days, 30 days or all time. Cost is what the tokens would cost at API list prices.'],
    ['[data-testid=attention]', 'Needs attention', 'A limit that runs out before its reset at today\'s pace, a login to renew, a provider the router paused.'],
    ['[data-testid=activity]', 'Activity, 53 weeks', 'A year clock of daily tokens: 53 weeks around, Monday inside, the busiest day in the centre.'],
    ['[data-testid=latest-replies]', 'Latest replies', 'Newest first, with the account, model, tokens and the time to the first reply; the request log has every call with its cost.'],
    ['[data-nav=home] .nav-badge', 'Today', 'Today\'s usage at API prices, always in view.'],
  ],
  accounts: [
    ['[data-testid=add-account]', 'Add an account', 'Each account gets its own folder and login. Sign in once; Harness Manager keeps the accounts apart.'],
    ['[data-testid=usage-panel]', 'Tokens per day', 'Every account\'s tokens for each day of the range, priced at API list prices in the rows below. Subscriptions are not billed per token.'],
    ['[data-testid=accounts-table]', 'Your accounts, by tool', 'One card per tool, one row per account. "main" is the tool\'s own login on this computer. Each limit shows how much is left and when it resets.'],
    ['[data-testid=accounts-table] .acct-windows-col', 'Limits', 'Every limit of the plan: 5-hour, weekly and per model. The ↻ button beside a Claude account reads its limits from Anthropic right now.'],
    ['[data-testid=accounts-table] .btn-lane-start', 'Start', 'Opens a terminal running the tool as this account. Log in appears instead when the account has not signed in yet.'],
    ['[data-testid=accounts-table] [data-act=account-menu]', 'More', 'Log in again, open the account\'s folder, its settings, or remove it.'],
    ['[data-testid=best-note]', 'The account with the most room', 'The account furthest from its limits says so: start your next session there.'],
  ],
  account: [
    ['.page-tabs', 'Overview, Sessions, Projects, Settings', 'Plan, limits and usage; this account\'s sessions and projects; and its email, key or endpoint.'],
    ['[data-act=start-in-folder]', 'Start in a folder', 'Opens the account in the project folder you pick.'],
    ['.page-actions [data-act=account-menu]', 'More', 'Log in again, open its folder, or remove the account.'],
  ],
  projects: [
    ['[data-testid=project-tiles]', 'Every project at a glance', 'Tokens, cost at API list prices, how many run now, and the busiest one. A project is a folder your tools ran in.'],
    ['[data-testid=project-cards]', 'One card per project', 'Open its sessions, start a new one, pin it to the sidebar, rename or merge it from the card\'s menu.'],
  ],
  sessions: [
    ['[data-testid=rail]', 'Projects, pinned and recent', 'What runs now, every project, the ones you pinned, and your tags. Pick one to see only its sessions.'],
    ['[data-act=tag-edit]', 'Tags', 'Your own labels on a session or a project, shared across tools and accounts. Group by Tag, or click a tag to see what carries it.'],
    ['[data-testid=find]', 'Find a session', 'The search above narrows this list by title, folder, account and tool as you type.'],
    ['[data-testid=group]', 'Grouped by project', 'Sessions of the same repository sit together, whichever account or tool ran them. Open one to read it.'],
  ],
  session: [
    ['[data-tour=modes]', 'Changes and Failed', 'List only the steps that edited files, or only the failed commands.'],
    ['[data-testid=chat]', 'Conversation', 'Your prompts on the right, the answers on the left, and every tool call (folded) where it happened: click one for its output or diff. An open session follows as its tool writes.'],
    ['[data-testid=resume]', 'Resume', 'Continues this session in a terminal, with the same account.'],
    ['[data-testid=export-md]', 'Export', 'Save the session as a Markdown file.'],
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
    ['[data-testid=skill-install]', 'Install a skill', 'From a folder or a GitHub link, then tick the tools that should have it. The tick in a tool\'s column header links every skill shown in one go. Skills are linked, so one update reaches every tool.'],
  ],
  prompts: [
    ['[data-testid=prompt-tabs]', 'One list per tool', 'The instructions file of each tool: CLAUDE.md for Claude Code, AGENTS.md for Codex, and so on.'],
    ['[data-testid=prompt-new]', 'Versions', 'Keep several prompts and switch with Use this prompt. The file\'s current text is saved in the list before it is replaced.'],
  ],
  settings: [
    ['[data-testid=tip-term]', 'What each setting means', 'Point at ⓘ next to a setting for what it does and what its default means.'],
    ['[data-testid=computers]', 'Computers', 'Name this computer and pick a shared folder your other computers also sync, or join a hub you host with a code. Each computer writes its own history there and reads the others\', so the Overview, Sessions and Projects add them all up. Keys and logins never go in either; the hub only ever holds sealed data.'],
    ['[data-testid=st-direct]', 'Direct connection', 'With this on, your computers find each other on this network or at an address you give and talk directly: what runs there within ten seconds, their history the moment it is written. The folder keeps working underneath.'],
    ['[data-testid=pair]', 'Pair a computer', 'Once per pair of computers: one shows a six-digit code, the other types it. Each card then says how that computer is reached, and Forget drops it again.'],
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
