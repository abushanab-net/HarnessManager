// The detail pane Sessions and Accounts share (docs/project-review-2026-11/mockups/b-sessions.html and b-accounts.html,
// zone 3): a row of the list is selected and shown in a 380 px pane at the right, and the full page of the item stays
// one step away. The selection is in the address (`sel=<id>`), so Back, a reload and a link keep it; whether the pane
// is open is remembered per page on this computer. A page marks its rows and draws the pane's inside; everything a
// selection does is here.
//
// A row: `rowAttrs(id, open, on)` on the row element, and `data-sel-title` on the control inside it that stands for
// the row (its title button), when the row itself takes no focus. The pane: `paneHtml(...)` as the last child of the
// sheet, drawn through `drawWithPane()`; app.css lays the two out by that child being there.

import { S, patch, action } from './core.js';
import { esc } from './lib.js';

const pages = {};
/** A page with a pane: `show()` draws the page again from what it last read, without asking the backend. */
export function paneOf(view, show) { pages[view] = show; }

const KEY = 'hm.pane.';
/** The id selected on the page shown ('' for none). */
export const selected = () => S.route.params.sel || '';
/** Whether the page's pane shows a selection: open until closed with Esc or ✕, and opened again by a click on a row. */
export const paneOpen = (view) => { try { return localStorage.getItem(KEY + view) !== '0'; } catch { return true; } };
const setOpen = (view, on) => { try { localStorage.setItem(KEY + view, on ? '1' : '0'); } catch {} };

/** What a row carries: its id, the address of its full page, and whether it is the selected one. */
export const rowAttrs = (id, open, on) => `data-act="sel" data-sel="${esc(id)}" data-open="${esc(open)}" ${on ? 'aria-current="true"' : ''}`;

/** The pane: a region of its own named by the item shown. `head` is the item's icon, name and line of facts (markup),
 * `body` the sections under it; the ✕ that closes the pane is the pane's own. */
const inside = (head, body) => `<div class="pane-sec pane-top">${head}<button class="btn sm icon quiet" data-act="pane-close" aria-label="Close the pane" title="Close (Esc)" data-testid="pane-close">✕</button></div>${body}`;
export const paneHtml = (name, head, body) => `<aside class="pane" id="pane" role="complementary" aria-label="${esc(name)}" data-testid="pane">${inside(head, body)}</aside>`;
/** Draw the pane on screen again, and nothing else: what it shows changed (a live session wrote a step), the list did not. */
export function patchPane(name, head, body) {
  const el = document.getElementById('pane');
  if (!el) return;
  el.setAttribute('aria-label', name);
  patch(el, inside(head, body));
}

// the list scrolls in the sheet without the pane, and in its own column beside it: its place is carried across
const listOf = (sheet) => (sheet.querySelector(':scope > .pane') && sheet.querySelector(':scope > .page-body')) || sheet;
let keepTop = null, drawnKey = '', focusRow = false;
/** Draw a page that may carry the pane (`html` ends with paneHtml() when it does). `key` names the list drawn: the
 * same list keeps its scroll, whether the pane opens, closes or stays; another list starts at its top. */
export function drawWithPane(sheet, html, key) {
  const from = listOf(sheet), top = keepTop ?? (key === drawnKey ? from.scrollTop : 0);
  keepTop = null; drawnKey = key;
  patch(sheet, html);
  const to = listOf(sheet);
  if (to.scrollTop !== top) to.scrollTop = top;
  if (focusRow) { focusRow = false; focusSelected(sheet); }
}
/** The keyboard focus on the selected row, in view: after the arrows moved the selection, and when the pane closes
 * with the focus inside it. */
function focusSelected(sheet) {
  const row = sheet.querySelector('[data-sel][aria-current]'), t = row && (row.querySelector('[data-sel-title]') || row);
  if (!t) return;
  t.focus({ preventScroll: true });
  row.scrollIntoView({ block: 'nearest' });
}

/** Select a row. A click also opens the pane; the arrows move the selection and leave the pane as it is. The first
 * selection is a step Back undoes; moving it replaces that step, so Back still returns to the list as it was. */
function select(id, { open = false, focus = false } = {}) {
  const view = S.route.view, sheet = document.getElementById('sheet');
  keepTop = listOf(sheet).scrollTop;
  focusRow = focus;
  if (open) setOpen(view, true);
  if (id === selected()) return pages[view]();
  const hash = '#' + view + '?' + new URLSearchParams({ ...S.route.params, sel: id });
  if (selected()) location.replace(hash); else location.hash = hash;
}
function close() {
  const view = S.route.view, pane = document.getElementById('pane');
  if (!pane) return;
  focusRow = pane.contains(document.activeElement);
  setOpen(view, false);
  pages[view]();
}
// a click on a row selects it; a control inside the row (its own button, a tag) acts for itself
// (the focus goes to the row, so the arrows and Enter go on from it)
action('sel', (el, e) => { if (!e.target.closest('button:not([data-sel-title]), a, input, select')) select(el.dataset.sel, { open: true, focus: true }); });
action('pane-close', close);
// a double click on a row opens the item's full page
document.addEventListener('dblclick', (e) => {
  const row = pages[S.route.view] && e.target.closest && e.target.closest('[data-sel]');
  if (row && !e.target.closest('button:not([data-sel-title]), a, input, select')) location.hash = row.dataset.open;
});
// The keys, asked before the shell's own (capture): Esc closes the pane, Enter on the selected row opens its full page,
// Up and Down move the selection while the focus is on a row. A dialog, a menu, the tour or a field being typed in keeps its keys.
document.addEventListener('keydown', (e) => {
  if (!pages[S.route.view] || e.altKey || e.ctrlKey || e.metaKey || document.querySelector('.menu, [role=dialog], .tour-pop')) return;
  const t = e.target, typing = t.matches && t.matches('input, textarea, select, [contenteditable]');
  if (e.key === 'Escape') {
    if (!typing && document.getElementById('pane')) { e.stopPropagation(); close(); }
    return;
  }
  const sheet = document.getElementById('sheet'), row = t.closest && t.closest('#sheet [data-sel]');
  if (e.key === 'Enter' && row && row.hasAttribute('aria-current') && (t === row || t.matches('[data-sel-title]'))) {
    e.preventDefault(); e.stopPropagation();
    location.hash = row.dataset.open;
  } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && row && !typing) {
    // from the selected row; with none selected yet, the row the focus is on becomes it
    const rows = [...sheet.querySelectorAll('[data-sel]')], at = rows.findIndex((r) => r.hasAttribute('aria-current'));
    const to = at < 0 ? rows.indexOf(row) : Math.max(0, Math.min(rows.length - 1, at + (e.key === 'ArrowDown' ? 1 : -1)));
    e.preventDefault(); e.stopPropagation();
    if (to !== at) select(rows[to].dataset.sel, { focus: true });
  }
}, true);
