// CLI reference, as the reference draws it: the finding, a Start here card of examples, and the seven command groups
// as cards in two columns. Static: no state, no empty state.

import { view, patch } from './core.js';
import { esc } from './lib.js';

const CLI = [
  ['Accounts', [['list', 'Accounts with plan, limits, tokens and open consoles'], ['show', 'One account in detail'], ['add', 'Add an account (subscription login by default)'],
    ['login', 'Log an account in'], ['remove', 'Remove an account; --delete-home also deletes its folder'], ['key', 'API keys, kept in Windows Credential Manager (the Keychain on macOS)']]],
  ['Run', [['run', 'Run the tool as an account in this console'], ['start', 'Open a new terminal running the account'], ['resume', 'Reopen a session in its own account'], ['open', 'Open the desktop app']]],
  ['Usage and limits', [['usage', 'Token usage per account and day'], ['overview', 'Cost at list prices, requests, tokens, cache hits'], ['requests', 'Every model reply, newest first'], ['quota', 'Limits of one account; --live asks Anthropic now']]],
  ['Sessions and projects', [['sessions', 'Sessions of every tool, grouped by project, date, account, tool or tag; --open, --project, --tag'], ['session', 'Print one session as a transcript'], ['projects', 'List projects with tokens and cost, rename or merge them'], ['tag', 'Tag a session or a project'], ['tags', 'Every tag with its counts']]],
  ['Shared setup', [['sync', 'Bring every account in step'], ['mcp', 'MCP servers shared across tools'], ['skills', 'Skills shared across tools'], ['prompts', 'Prompt files per tool']]],
  ['Routing and setup', [['provider', 'Router providers: endpoints and keys'], ['route', 'Failover or aggregate routes'], ['router', 'Start, stop or check the local router'], ['settings', 'Read or change settings'],
    ['init', 'Find installed tools and add their main profiles'], ['import', 'Import from the old claude-accounts.ps1 launcher (Windows)']]],
  ['Computers', [['computers status', 'This computer, the shared folder and the others; --json'], ['computers now', 'Publish what is new here and merge what the others published'], ['computers set', 'The name, the folder, what is shared, --direct on|off, --port'],
    ['hub join <url> --code <c> | --admin <t>', 'Join the hub you host: with a code from a computer already on it, or the admin token for the first'], ['hub invite', 'A one-time six-digit code for another computer'], ['hub status', 'Joined, computers, the last push and pull; --json'], ['hub leave', 'Leave the hub; what was merged stays'],
    ['ring show | enter [key]', 'The recovery key: print it for your password manager, or enter it on a computer that is not paired'],
    ['computers pair', 'Show a six-digit code and wait for the other computer; --host for one on another network'], ['computers join <code>', 'Pair with the computer showing the code; --host name:port when not on this network'],
    ['computers peers', 'The paired computers, how each is reached and when it was last pulled'], ['computers forget <id or name>', 'Stop connecting to a computer; what it sent stays'], ['computers serve --seconds <n>', 'Run the listener without the app, for tests and headless use']]],
];
const EX = [['hrm list', 'Accounts, plans, limits, tokens and open consoles'], ['hrm start work-max --cwd D:\\Projects\\my-app', 'Start in that folder (a new terminal)'],
  ['hrm quota work-max --live', 'Limits straight from Anthropic'], ['hrm sessions --group project --json', "Every tool's sessions, for scripts"]];
const slug = (g) => g.toLowerCase().replace(/[^a-z]+/g, '-');

view('cli', async (sheet) => {
  sheet.className = 'sheet';
  patch(sheet, `<div class="page-body b-page" data-b-page="cli">
    <header class="page-heading"><div><h1>Every desktop action has an hrm command.</h1><div class="heading-facts">Add <span class="mono">--json</span> for scripts · static reference</div></div></header>
    <section class="panel-card"><div class="panel-head"><h2 class="panel-title">Start here</h2></div><div class="b-cli-examples">${EX.map(([c, d]) => `<div><code>${esc(c)}</code><span class="muted">${esc(d)}</span></div>`).join('')}</div></section>
    <div class="b-cli-grid">${CLI.map(([g, rows]) => `<section class="panel-card" id="cli-${slug(g)}"><div class="panel-head"><h2 class="panel-title">${esc(g)}</h2></div>${rows.map(([c, d]) => `<div class="b-cli-row"><code>hrm ${esc(c)}</code><span>${esc(d)}</span></div>`).join('')}</section>`).join('')}</div>
  </div>`);
});
