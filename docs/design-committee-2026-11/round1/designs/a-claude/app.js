// Design A, "The board": the prototype. Classic script; needs data.js first.
(function () {
  const D = window.A;
  const { MIN, H, DAY } = D;
  let NOW = D.NOW, LIMITS_READ = D.LIMITS_READ, USAGE_READ = D.USAGE_READ;

  // ---------- assets (inlined; see angle.md) ----------
  const LOGO = '<svg viewBox="60 10 410 500" role="img" aria-label="Harness Manager"><defs><linearGradient id="hm-lu" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#006D79"/><stop offset="1" stop-color="#009C99"/></linearGradient><linearGradient id="hm-ll" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#393F91"/><stop offset="1" stop-color="#7F82E8"/></linearGradient><linearGradient id="hm-bc" x1="0" y1=".1" x2="1" y2=".8"><stop stop-color="#00616C"/><stop offset=".58" stop-color="#097D8C"/><stop offset="1" stop-color="#57CAC9"/></linearGradient><linearGradient id="hm-ru" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#6588DD"/><stop offset=".6" stop-color="#4455AF"/><stop offset="1" stop-color="#283875"/></linearGradient><linearGradient id="hm-rl" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#00A3A8"/><stop offset="1" stop-color="#00676D"/></linearGradient><linearGradient id="hm-fc" x1="0" y1="1" x2="1" y2="0"><stop stop-color="#65DBD8"/><stop offset=".4" stop-color="#36C8C5"/><stop offset=".76" stop-color="#078B98"/><stop offset="1" stop-color="#087680"/></linearGradient><linearGradient id="hm-rm" x1="0" y1="0" x2="1" y2="0"><stop stop-color="#B0E8E4"/><stop offset="1" stop-color="#5BCAD1"/></linearGradient></defs><path d="M84 259 L84 115 C84 90 96 77 121 63 L179 31 C194 23 207 25 207 44 L207 211 C173 229 122 255 84 280 Z" fill="url(#hm-lu)"/><path d="M188 292 C231 302 284 329 333 356 L333 425 C286 402 236 376 184 347 L153 330 Z" fill="url(#hm-bc)"/><path d="M84 342 L84 381 C84 402 96 417 113 428 L176 473 C196 487 207 482 207 465 L207 305 C170 320 119 340 84 342 Z" fill="url(#hm-ll)"/><path d="M333 41 C333 24 342 20 358 28 L418 62 C439 74 447 90 447 110 L447 192 C408 218 372 236 333 251 Z" fill="url(#hm-ru)"/><path d="M333 244 L447 190 L447 287 L333 344 Z" fill="url(#hm-rm)"/><path d="M333 340 L447 283 L447 413 C447 435 437 450 416 464 L365 494 C347 506 333 496 333 479 Z" fill="url(#hm-rl)"/><path d="M84 270 C84 242 106 229 149 208 L333 121 L333 244 L151 332 C111 353 91 368 84 393 Z" fill="url(#hm-fc)"/></svg>';
  const TOOL = {
    claude: '<svg viewBox="0 0 24 24" aria-label="Claude Code"><path clip-rule="evenodd" d="M20.998 10.949H24v3.102h-3v3.028h-1.487V20H18v-2.921h-1.487V20H15v-2.921H9V20H7.488v-2.921H6V20H4.487v-2.921H3V14.05H0V10.95h3V5h17.998v5.949zM6 10.949h1.488V8.102H6v2.847zm10.51 0H18V8.102h-1.49v2.847z" fill="#D97757" fill-rule="evenodd"/></svg>',
    codex: '<svg viewBox="0 0 24 24" aria-label="Codex"><path d="M19.503 0H4.496A4.496 4.496 0 000 4.496v15.007A4.496 4.496 0 004.496 24h15.007A4.496 4.496 0 0024 19.503V4.496A4.496 4.496 0 0019.503 0z" fill="#fff"/><path d="M9.064 3.344a4.578 4.578 0 012.285-.312c1 .115 1.891.54 2.673 1.275.01.01.024.017.037.021a.09.09 0 00.043 0 4.55 4.55 0 013.046.275l.047.022.116.057a4.581 4.581 0 012.188 2.399c.209.51.313 1.041.315 1.595a4.24 4.24 0 01-.134 1.223.123.123 0 00.03.115c.594.607.988 1.33 1.183 2.17.289 1.425-.007 2.71-.887 3.854l-.136.166a4.548 4.548 0 01-2.201 1.388.123.123 0 00-.081.076c-.191.551-.383 1.023-.74 1.494-.9 1.187-2.222 1.846-3.711 1.838-1.187-.006-2.239-.44-3.157-1.302a.107.107 0 00-.105-.024c-.388.125-.78.143-1.204.138a4.441 4.441 0 01-1.945-.466 4.544 4.544 0 01-1.61-1.335c-.152-.202-.303-.392-.414-.617a5.81 5.81 0 01-.37-.961 4.582 4.582 0 01-.014-2.298.124.124 0 00.006-.056.085.085 0 00-.027-.048 4.467 4.467 0 01-1.034-1.651 3.896 3.896 0 01-.251-1.192 5.189 5.189 0 01.141-1.6c.337-1.112.982-1.985 1.933-2.618.212-.141.413-.251.601-.33.215-.089.43-.164.646-.227a.098.098 0 00.065-.066 4.51 4.51 0 01.829-1.615 4.535 4.535 0 011.837-1.388zm3.482 10.565a.637.637 0 000 1.272h3.636a.637.637 0 100-1.272h-3.636zM8.462 9.23a.637.637 0 00-1.106.631l1.272 2.224-1.266 2.136a.636.636 0 101.095.649l1.454-2.455a.636.636 0 00.005-.64L8.462 9.23z" fill="url(#cdx-g)"/><defs><linearGradient gradientUnits="userSpaceOnUse" id="cdx-g" x1="12" x2="12" y1="3" y2="21"><stop stop-color="#B1A7FF"/><stop offset=".5" stop-color="#7A9DFF"/><stop offset="1" stop-color="#3941FF"/></linearGradient></defs></svg>',
    opencode: '<svg fill="currentColor" fill-rule="evenodd" viewBox="0 0 24 24" aria-label="OpenCode"><path d="M16 6H8v12h8V6zm4 16H4V2h16v20z"/></svg>',
    agy: '<svg viewBox="-1.5 -1.5 27 27" aria-label="Antigravity"><mask id="agy-m" maskUnits="userSpaceOnUse" x="0" y="1" width="24" height="23"><path d="M21.751 22.607c1.34 1.005 3.35.335 1.508-1.508C17.73 15.74 18.904 1 12.037 1 5.17 1 6.342 15.74.815 21.1c-2.01 2.009.167 2.511 1.507 1.506 5.192-3.517 4.857-9.714 9.715-9.714 4.857 0 4.522 6.197 9.714 9.715z" fill="#fff"/></mask><g mask="url(#agy-m)"><g filter="url(#agy-f1)"><path d="M-1.018-3.992c-.408 3.591 2.686 6.89 6.91 7.37 4.225.48 7.98-2.043 8.387-5.633.408-3.59-2.686-6.89-6.91-7.37-4.225-.479-7.98 2.043-8.387 5.633z" fill="#FFE432"/></g><g filter="url(#agy-f2)"><path d="M15.269 7.747c1.058 4.557 5.691 7.374 10.348 6.293 4.657-1.082 7.575-5.653 6.516-10.21-1.058-4.556-5.691-7.374-10.348-6.292-4.657 1.082-7.575 5.653-6.516 10.21z" fill="#FC413D"/></g><g filter="url(#agy-f3)"><path d="M-12.443 10.804c1.338 4.703 7.36 7.11 13.453 5.378 6.092-1.733 9.947-6.95 8.61-11.652C8.282-.173 2.26-2.58-3.833-.848-9.925.884-13.78 6.1-12.443 10.804z" fill="#00B95C"/></g><g filter="url(#agy-f3)"><path d="M-12.443 10.804c1.338 4.703 7.36 7.11 13.453 5.378 6.092-1.733 9.947-6.95 8.61-11.652C8.282-.173 2.26-2.58-3.833-.848-9.925.884-13.78 6.1-12.443 10.804z" fill="#00B95C"/></g><g filter="url(#agy-f5)"><path d="M-7.608 14.703c3.352 3.424 9.126 3.208 12.896-.483 3.77-3.69 4.108-9.459.756-12.883C2.69-2.087-3.083-1.871-6.853 1.82c-3.77 3.69-4.108 9.458-.755 12.883z" fill="#00B95C"/></g><g filter="url(#agy-f6)"><path d="M9.932 27.617c1.04 4.482 5.384 7.303 9.7 6.3 4.316-1.002 6.971-5.448 5.93-9.93-1.04-4.483-5.384-7.304-9.7-6.301-4.316 1.002-6.971 5.448-5.93 9.93z" fill="#3186FF"/></g><g filter="url(#agy-f7)"><path d="M2.572-8.185C.392-3.329 2.778 2.472 7.9 4.771c5.122 2.3 11.042.227 13.222-4.63 2.18-4.855-.205-10.656-5.327-12.955-5.122-2.3-11.042-.227-13.222 4.63z" fill="#FBBC04"/></g><g filter="url(#agy-f8)"><path d="M-3.267 38.686c-5.277-2.072 3.742-19.117 5.984-24.83 2.243-5.712 8.34-8.664 13.616-6.592 5.278 2.071 11.533 13.482 9.29 19.195-2.242 5.713-23.613 14.298-28.89 12.227z" fill="#3186FF"/></g><g filter="url(#agy-f9)"><path d="M28.71 17.471c-1.413 1.649-5.1.808-8.236-1.878-3.135-2.687-4.531-6.201-3.118-7.85 1.412-1.649 5.1-.808 8.235 1.878s4.532 6.2 3.119 7.85z" fill="#749BFF"/></g><g filter="url(#agy-f10)"><path d="M18.163 9.077c5.81 3.93 12.502 4.19 14.946.577 2.443-3.612-.287-9.727-6.098-13.658-5.81-3.931-12.502-4.19-14.946-.577-2.443 3.612.287 9.727 6.098 13.658z" fill="#FC413D"/></g><g filter="url(#agy-f11)"><path d="M-.915 2.684c-1.44 3.473-.97 6.967 1.05 7.804 2.02.837 4.824-1.3 6.264-4.772 1.44-3.473.97-6.967-1.05-7.804-2.02-.837-4.824 1.3-6.264 4.772z" fill="#FFEE48"/></g></g><defs><filter id="agy-f1" color-interpolation-filters="sRGB" filterUnits="userSpaceOnUse" x="-3.288" y="-11.917" width="19.838" height="17.587"><feGaussianBlur stdDeviation="1.117"/></filter><filter id="agy-f2" color-interpolation-filters="sRGB" filterUnits="userSpaceOnUse" x="4.251" y="-13.493" width="38.9" height="38.565"><feGaussianBlur stdDeviation="5.4"/></filter><filter id="agy-f3" color-interpolation-filters="sRGB" filterUnits="userSpaceOnUse" x="-21.889" y="-10.592" width="40.955" height="36.517"><feGaussianBlur stdDeviation="4.591"/></filter><filter id="agy-f5" color-interpolation-filters="sRGB" filterUnits="userSpaceOnUse" x="-19.099" y="-10.278" width="36.632" height="36.595"><feGaussianBlur stdDeviation="4.591"/></filter><filter id="agy-f6" color-interpolation-filters="sRGB" filterUnits="userSpaceOnUse" x=".981" y="8.758" width="33.533" height="34.087"><feGaussianBlur stdDeviation="4.363"/></filter><filter id="agy-f7" color-interpolation-filters="sRGB" filterUnits="userSpaceOnUse" x="-6.143" y="-21.659" width="35.978" height="35.276"><feGaussianBlur stdDeviation="3.954"/></filter><filter id="agy-f8" color-interpolation-filters="sRGB" filterUnits="userSpaceOnUse" x="-11.96" y="-.46" width="45.114" height="46.523"><feGaussianBlur stdDeviation="3.531"/></filter><filter id="agy-f9" color-interpolation-filters="sRGB" filterUnits="userSpaceOnUse" x="10.485" y=".58" width="25.094" height="24.054"><feGaussianBlur stdDeviation="3.159"/></filter><filter id="agy-f10" color-interpolation-filters="sRGB" filterUnits="userSpaceOnUse" x="5.833" y="-12.467" width="33.508" height="30.007"><feGaussianBlur stdDeviation="2.669"/></filter><filter id="agy-f11" color-interpolation-filters="sRGB" filterUnits="userSpaceOnUse" x="-8.355" y="-8.876" width="22.194" height="26.151"><feGaussianBlur stdDeviation="3.303"/></filter></defs></svg>',
    pi: '<svg viewBox="-1.5 -1.5 27 27" aria-label="Pi"><rect x="0" y="0" width="24" height="24" rx="5" fill="#1F2937"/><path d="M5.5 7.5h13M8.5 7.5v10M15.5 7.5v10" fill="none" stroke="#F5F5F4" stroke-width="2.2" stroke-linecap="round"/></svg>',
    omp: '<svg viewBox="-1.5 -1.5 27 27" aria-label="Oh My Pi"><rect x="0" y="0" width="24" height="24" rx="5" fill="#7C3AED"/><path d="M5.5 7.5h13M8.5 7.5v10M15.5 7.5v10" fill="none" stroke="#FFFFFF" stroke-width="2.2" stroke-linecap="round"/><circle cx="19" cy="5" r="2.2" fill="#FDE68A"/></svg>',
  };
  const UI = {
    search: '<circle cx="11" cy="11" r="7"/><path d="M21 21l-4.3-4.3"/>', refresh: '<path d="M21 12a9 9 0 1 1-2.6-6.4M21 4v5h-5"/>', help: '<circle cx="12" cy="12" r="9"/><path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.7.3-1 .8-1 1.5V14M12 17h.01"/>',
    gear: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>',
    play: '<path d="M7 4l13 8-13 8z" fill="currentColor" stroke="none"/>', x: '<path d="M6 6l12 12M18 6L6 18"/>', chev: '<path d="M9 6l6 6-6 6"/>', down: '<path d="M6 9l6 6 6-6"/>', more: '<circle cx="5" cy="12" r="1.6" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="1.6" fill="currentColor" stroke="none"/><circle cx="19" cy="12" r="1.6" fill="currentColor" stroke="none"/>',
    folder: '<path d="M3 6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>', window: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18"/>', plus: '<path d="M12 5v14M5 12h14"/>', back: '<path d="M15 6l-6 6 6 6"/>', check: '<path d="M5 12l5 5L20 7"/>', link: '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>',
  };
  const ic = (k, s = 16) => `<svg width="${s}" height="${s}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${UI[k]}</svg>`;
  const ti = (id) => TOOL[id] || '';

  // ---------- helpers ----------
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
  const h = (html) => { const t = document.createElement('template'); t.innerHTML = html.trim(); return t.content.firstElementChild; };
  const fmt = (n) => (n == null ? '—' : n >= 1e9 ? (n / 1e9).toFixed(2).replace(/\.?0+$/, '') + 'B' : n >= 1e6 ? (n / 1e6).toFixed(n >= 1e7 ? 1 : 2).replace(/\.?0+$/, '') + 'M' : n >= 1e3 ? Math.round(n / 1e3) + 'k' : String(Math.round(n)));
  const money = (c, d = 2) => '$' + c.toLocaleString('en-US', { minimumFractionDigits: d, maximumFractionDigits: d });
  const dur = (ms) => { const m = Math.max(0, Math.round(ms / MIN)); if (m < 60) return m + ' m'; const hh = Math.floor(m / 60); if (hh < 24) return hh + ' h' + (m % 60 ? ' ' + String(m % 60).padStart(2, '0') + ' m' : ''); const d = Math.floor(hh / 24); return d + ' d' + (hh % 24 ? ' ' + (hh % 24) + ' h' : ''); };
  const ago = (ms) => (ms < 60e3 ? Math.max(1, Math.round(ms / 1e3)) + ' s ago' : ms < 2 * MIN ? '1 m ago' : dur(ms) + ' ago');
  const at = (ms) => { const d = new Date(NOW + ms), t = d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit', hour12: false }); return (d.toDateString() === new Date(NOW).toDateString() ? '' : d.toLocaleDateString('en-GB', { weekday: 'short' }) + ' ') + t; };
  const clock = (t) => new Date(t).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false });
  const acct = (id) => D.accounts.concat(D.extra, st.added).find((a) => a.id === id);
  const tool = (id) => D.tools.find((t) => t.id === id);
  const pc = (id) => D.computers.find((c) => c.id === id);
  const alive = (c) => c.seen < 3 * MIN;
  const me = D.computers.find((c) => c.me);
  const left = (w) => 100 - w.used;
  const tone = (l) => (l == null ? '' : l < 15 ? 'neg' : l < 40 ? 'warn' : '');
  const weekWin = (a) => a.windows && (a.windows.find((w) => w.label === 'Week') || a.windows[0]);
  const room = (a) => (a.windows ? Math.min(...a.windows.map(left)) : -1);
  const tightest = (a) => a.windows && a.windows.slice().sort((x, y) => left(x) - left(y) || x.reset - y.reset)[0];
  // one colour rule everywhere: red = cannot be used now (login expired, a window exhausted); amber = a window runs out before its reset at today's pace; else teal
  const acctTone = (a) => (a.needsLogin || (a.windows || []).some((w) => left(w) <= 0) ? 'neg' : (a.windows || []).some((w) => w.out) ? 'warn' : '');
  const noQuota = (a) => !a.windows && tool(a.tool) && !tool(a.tool).quota;
  const pcCss = (c) => `--c:${c.col}`;

  // ---------- state ----------
  const KEY = 'hm-design-a';
  let st = { theme: 'dark', w: 1536, still: false, data: 'full', used: false, page: 'overview', toolF: 'all', pcF: 'all', range: 'today', arange: '7d', log: false, yc: 'all', follow: null, detail: null, dtab: 'overview', q: '', sync: null, added: [], more: {}, opener: null };
  try { Object.assign(st, JSON.parse(localStorage.getItem(KEY) || '{}')); } catch (e) {}
  st.follow = null; st.detail = null; st.added = []; st.more = {};
  const save = () => { try { localStorage.setItem(KEY, JSON.stringify({ theme: st.theme, w: st.w, still: st.still, data: st.data, used: st.used })); } catch (e) {} };
  const reduced = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // the accounts, sessions and computers the prototype shows in the chosen data mode (plus accounts added in this session)
  const full = () => st.data === 'full' || st.data === 'twelve';
  const accounts = () => (full() ? D.accounts.concat(st.data === 'twelve' ? D.extra : []) : D.firstRun[st.data].tools.map((t) => ({ ...acct(t + '-main'), windows: null, open: 0, tokens7: 0, cost7: 0, spark: [0, 0, 0, 0, 0, 0, 0], read: null, needsLogin: false, advice: null }))).concat(st.added);
  const sessions = () => (full() ? D.sessions : []);
  const pcs = () => (full() ? D.computers : [me]);
  const shownSessions = () => sessions().filter((s) => (st.toolF === 'all' || s.tool === st.toolF) && (st.pcF === 'all' || s.computer === me.id) && (!st.q || (s.project + ' ' + s.account).toLowerCase().includes(st.q)));
  const sorted = () => accounts().slice().sort((x, y) => room(y) - room(x));
  const useNext = () => sorted().find((a) => a.windows && !a.needsLogin) || null;
  // exhausted now beats a forecast: a window at 0% left is the answer to "which account is exhausted"
  const outFirst = () => { let now = null, best = null; for (const a of accounts()) { if (a.needsLogin) continue; for (const w of a.windows || []) { if (left(w) <= 0 && (!now || w.reset < now.w.reset)) now = { a, w, now: true }; if (w.out && (!best || w.out < best.w.out)) best = { a, w }; } } return now || best; };
  const resetsSoon = () => { let best = null; for (const a of accounts()) for (const w of a.windows || []) if (w.used >= 50 && (!best || w.reset < best.w.reset)) best = { a, w }; return best; };

  // ---------- pieces ----------
  function ring(a, size = 'md', value) {
    const w = weekWin(a), l = value != null ? value : w ? left(w) : null, shown = l == null ? null : st.used ? 100 - l : l;
    const r = size === 'lg' ? 22 : size === 'sm' ? 12 : size === 'ld' ? 15 : 17, px = size === 'lg' ? 52 : size === 'sm' ? 30 : size === 'ld' ? 36 : 40, sw = size === 'sm' ? 2.5 : size === 'ld' ? 3 : 3.5, C = 2 * Math.PI * r;
    return `<span class="ring ${size} ${l == null ? 'none' : ''}" data-ring="${esc(a.id)}" title="${l == null ? (noQuota(a) ? 'no limits on this tool' : 'limits not read') : `week: ${l}% left, ${100 - l}% used`}">
      <svg width="${px}" height="${px}" viewBox="0 0 ${px} ${px}"><circle class="tr" cx="${px / 2}" cy="${px / 2}" r="${r}" fill="none" stroke-width="${sw}"/>
      ${l == null ? '' : `<circle class="ar ${acctTone(a)}" cx="${px / 2}" cy="${px / 2}" r="${r}" fill="none" stroke-width="${sw}" stroke-dasharray="${C}" stroke-dashoffset="${C}" data-off="${C * (1 - shown / 100)}"/>`}</svg>
      <b>${l == null ? '—' : shown + '%'}</b></span>`;
  }
  // rings draw once: after insertion, set the offset on the next frame
  const drawRings = (root) => requestAnimationFrame(() => root.querySelectorAll('.ring .ar[data-off]').forEach((c) => { c.style.strokeDashoffset = c.dataset.off; }));
  const dotgrid = (vals) => `<span class="dg" aria-hidden="true">${vals.slice(-14).map((v) => `<i style="--o:${(0.15 + v * 0.85).toFixed(2)}"></i>`).join('')}</span>`;
  const livegrid = () => { const ss = sessions(); return `<span class="lgrid" aria-hidden="true">${Array.from({ length: 10 }, (_, i) => `<i class="${i < ss.length ? (ss[i].computer === me.id ? 'on' : 'el') : ''}"></i>`).join('')}</span>`; };
  const pcdot = (c, cls = '') => `<i class="${cls} ${alive(c) ? 'on' : ''}" style="${pcCss(c)}" title="${esc(c.name)}${alive(c) ? '' : ' · last heard ' + ago(c.seen)}"></i>`;

  // ---------- shell ----------
  const app = document.getElementById('app');
  function shell() {
    const ss = sessions(), here = ss.filter((s) => s.computer === me.id).length, there = ss.length - here, ac = accounts(), cs = pcs(), f = full();
    app.innerHTML = `
      <header class="top">
        <div class="brand">${LOGO}<div><b>Harness Manager</b><small>${esc(me.name)} · ${ac.length} account${ac.length === 1 ? '' : 's'}</small></div></div>
        <label class="find" data-tour="find">${ic('search', 15)}<input id="q" placeholder="Find an account, session or setting" aria-label="Find an account, session or setting" value="${esc(st.q)}"><kbd>Ctrl K</kbd></label>
        <div class="status">
          <button class="live" data-act="live" title="Sessions running now. A click shows them.">${ss.length ? '<i></i>' : ''}<span>${here} live</span><em>· ${there} elsewhere</em></button>
          <button class="pcs" data-act="computers" title="Computers: teal when heard from within three minutes. Opens Settings › Computers."><span class="dots">${cs.map((c) => pcdot(c)).join('')}</span><span>${cs.length} computer${cs.length === 1 ? '' : 's'}</span></button>
          <span class="fresh" id="fresh"></span>
          <button class="ib" data-act="tour" aria-label="Tour of this page" title="Tour of this page">${ic('help', 16)}</button>
          <button class="ib" data-act="refresh" aria-label="Refresh usage and limits" title="Re-read usage and limits">${ic('refresh', 16)}</button>
          <button class="ib" data-act="gear" aria-label="Settings, CLI reference and version" title="Settings">${ic('gear', 16)}${f ? `<i class="dot" title="Harness Manager ${D.counts.newerHarness} is available"></i>` : ''}</button>
        </div>
      </header>
      <nav class="tabs" aria-label="Pages">
        <button class="tab" data-page="overview">Overview ${f ? `<span class="n">${money(D.readouts.today.cost.v, 0)}</span>` : ''}</button>
        <button class="tab" data-page="sessions">Sessions ${livegrid()}</button>
        <button class="tab" data-page="projects">Projects ${f ? `<span class="n">${D.projects.count}</span><span class="caret" role="button" tabindex="0" data-act="pins" aria-label="Pinned projects">${ic('chev', 12)}</span>` : ''}</button>
        <button class="tab" data-page="accounts">Accounts <span class="n">${ac.length}</span></button>
        <button class="tab" data-act="setup" aria-haspopup="menu">Setup ${f ? `<span class="n warn">${D.counts.apps} update</span>` : ''}${ic('down', 12)}</button>
      </nav>
      <div id="notice"></div>
      <main class="page" id="main"></main>
      <div id="overlay"></div>`;
    app.querySelector('#q').addEventListener('input', (e) => { st.q = e.target.value.trim().toLowerCase(); if (st.page === 'overview') patchBoard(); else if (st.page === 'accounts') page(); });
    fresh();
  }
  function fresh() { const el = document.getElementById('fresh'); if (!el) return; const never = !accounts().some((a) => a.read); const u = NOW - USAGE_READ < 60e3 ? 'just now' : ago(NOW - USAGE_READ); el.innerHTML = never ? `<span>Limits not read yet</span>` : `<span class="long">Limits read ${ago(NOW - LIMITS_READ)} · usage ${u}</span><span class="short" title="Limits read ${ago(NOW - LIMITS_READ)} · usage ${u}">limits ${ago(NOW - LIMITS_READ).replace(' ago', '')} · usage ${u.replace('just now', 'now').replace(' ago', '')}</span>`; }
  function tabs() { app.querySelectorAll('.tab[data-page]').forEach((t) => (t.dataset.page === st.page ? t.setAttribute('aria-current', 'page') : t.removeAttribute('aria-current'))); }

  // ---------- Overview ----------
  function overview() {
    const main = document.getElementById('main');
    if (!full()) return firstRun(main);
    main.innerHTML = `<div class="wrap">
      <section class="strip" aria-label="The answer" data-tour="strip">
        <div class="cell" id="c-run"></div>
        <div class="cell out" id="c-out"></div>
        <div class="cell use" id="c-use"><div class="act" id="c-act"></div></div>
      </section>
      <section class="ov">
        <div class="board" id="board" data-tour="board">
          <div class="bh"><div class="chips" role="group" aria-label="Tool filter"><button class="chip" data-tf="all" aria-pressed="${st.toolF === 'all'}">All</button>${D.tools.map((t) => `<button class="chip" data-tf="${t.id}" aria-pressed="${st.toolF === t.id}" title="${esc(t.label)}">${ti(t.id)}<span>${esc(t.label)}</span></button>`).join('')}</div>
            <div class="seg" role="group" aria-label="Computers"><button data-pf="all" aria-pressed="${st.pcF === 'all'}">All computers</button><button data-pf="me" aria-pressed="${st.pcF === 'me'}">This computer</button></div><span class="sub mute" id="bcount"></span></div>
          <div class="lanes" id="lanes"></div>
        </div>
        <aside class="panel ladder" id="ladder" data-tour="ladder" aria-label="Room on every account">
          <div class="ph"><h2>Room</h2><span class="sub">ring: week ${st.used ? 'used' : 'left'} · <span id="lread"></span></span></div>
          <ol id="lad"></ol>
          <div class="foot"><span class="lg">ring and date: the week</span><span class="lg">bar: the window with the least room</span><span class="lg"><i class="warn"></i> ≈ runs out before its reset</span><span class="lg"><i class="neg"></i> exhausted or logged out</span><span class="lg">— not read</span></div>
        </aside>
      </section>
      <section class="panel" data-tour="reads" aria-label="Readouts"><div class="reads" id="reads"></div></section>
      <section class="lower">
        <div class="col">
          <div class="panel"><div class="ph"><h2>Activity, 53 weeks</h2><div class="right"><div class="seg pcseg" role="group" aria-label="Per computer"><button data-yc="all" aria-pressed="${st.yc === 'all'}">All</button>${pcs().map((c) => `<button data-yc="${c.id}" aria-pressed="${st.yc === c.id}" title="${esc(c.name)}"><i style="${pcCss(c)}"></i></button>`).join('')}</div></div></div><div class="yc" id="yc"></div></div>
          <div class="panel"><div class="ph"><h2>Needs attention</h2><div class="right"><span class="sub">${D.attention.length} open</span></div></div>
            <ul class="att">${D.attention.length ? D.attention.map((x) => `<li><i class="${x.kind}"></i><b>${esc(x.who)}</b><span class="when">${esc(x.when)}</span><span class="why">${esc(x.why)}</span></li>`).join('') : '<li class="none">Nothing</li>'}</ul></div>
        </div>
        <div class="col"><div class="panel" id="replies"></div></div>
      </section>
    </div>`;
    patchStrip(); patchBoard(); patchLadder(); reads(); yearClock(); replies();
    main.querySelectorAll('[data-tf]').forEach((b) => b.addEventListener('click', () => { st.toolF = b.dataset.tf; main.querySelectorAll('[data-tf]').forEach((x) => x.setAttribute('aria-pressed', x === b)); patchBoard(); patchLadder(); }));
    main.querySelectorAll('[data-pf]').forEach((b) => b.addEventListener('click', () => { st.pcF = b.dataset.pf; main.querySelectorAll('[data-pf]').forEach((x) => x.setAttribute('aria-pressed', x === b)); patchBoard(); }));
    main.querySelectorAll('[data-yc]').forEach((b) => b.addEventListener('click', () => { st.yc = b.dataset.yc; main.querySelectorAll('[data-yc]').forEach((x) => x.setAttribute('aria-pressed', x === b)); yearClock(); }));
  }

  // the answer strip: three cells, written from the same snapshot as the board and ladder on every tick (the Start button is kept)
  const nw = (s) => `<span class="nw">${esc(s)}</span>`;
  function patchStrip() {
    const run = document.getElementById('c-run'), out = document.getElementById('c-out'), use = document.getElementById('c-use'); if (!run) return;
    const ss = sessions(), here = ss.filter((s) => s.computer === me.id).length, there = ss.length - here, cs = pcs(), al = cs.filter(alive).length;
    run.innerHTML = `<div class="k">Running now</div><div class="v"><span>${ss.length} session${ss.length === 1 ? '' : 's'}</span></div><div class="s"><b>${here} here</b> · ${there} elsewhere · ${al} of ${cs.length} computers alive</div>`;
    const o = outFirst(), read = `read ${ago(NOW - LIMITS_READ)}`;
    out.classList.toggle('neg', !!(o && o.now));
    out.innerHTML = !o ? '<div class="k">Runs out first</div><div class="v"><span>Nothing</span></div><div class="s">No window runs out before its reset at today\'s pace.</div>'
      : o.now ? `<div class="k">Exhausted now</div><div class="v">${ti(o.a.tool)}<span>${esc(o.a.id)}</span></div><div class="s"><b>${o.w.label} 100% used</b> · back in ${dur(o.w.reset)} (${at(o.w.reset)}) · ${read}</div>`
      : `<div class="k">Runs out first</div><div class="v">${ti(o.a.tool)}<span>${esc(o.a.id)}</span></div><div class="s"><b>${o.w.label} ${o.w.used}% used</b> · ≈ out ${at(o.w.out)} · resets ${at(o.w.reset)} · ${read}</div>`;
    const u = useNext(), soon = resetsSoon(), act = document.getElementById('c-act');
    use.querySelectorAll('.k,.v,.s').forEach((x) => x.remove());
    use.insertAdjacentHTML('afterbegin', u ? `<div class="k">Use next</div><div class="v">${ti(u.tool)}<span>${esc(u.id)}</span></div><div class="s"><b>${u.windows.map((w) => left(w) + '% of ' + w.label.toLowerCase()).slice(0, 2).join(', ')} left</b>${soon ? ` · ${nw(soon.a.id)}'s ${soon.w.label.toLowerCase()} resets in ${nw(dur(soon.w.reset))}` : ''}</div>`
      : '<div class="k">Use next</div><div class="v"><span>—</span></div><div class="s">No account with limits read.</div>');
    if (u && act.querySelector('button')?.dataset.id !== u.id) act.innerHTML = `<button class="btn primary sm" data-act="start" data-id="${esc(u.id)}">${ic('play', 12)} Start <span class="nm">${esc(u.id)}</span></button>`;
    if (!u) act.innerHTML = '';
  }

  // the board: lanes per computer, cards keyed by session id; patched in place every tick; four cards per lane, "+N more" opens the rest
  const CAP = 4;
  function cardHTML(s) {
    return `<button class="card" data-session="${s.id}" aria-pressed="${st.follow === s.id}" title="Follow ${esc(s.project)}">
      <div class="r1">${ti(s.tool)}<b>${esc(s.account)}</b><span class="w">writing</span></div>
      <div class="r2"><span class="p">${esc(s.project)}</span><span class="t" data-f="since"></span></div>
      <div class="r3"><span class="m">${esc(s.model)} · <span data-f="tokens"></span> · <span data-f="cost"></span></span><span class="ago" data-f="wrote"></span></div></button>`;
  }
  function patchCard(el, s) {
    el.classList.toggle('writing', s.wrote < 30e3);
    el.querySelector('[data-f=since]').textContent = dur(s.since);
    el.querySelector('[data-f=tokens]').textContent = fmt(s.tokens);
    el.querySelector('[data-f=cost]').textContent = money(s.cost);
    el.querySelector('[data-f=wrote]').textContent = 'wrote ' + ago(s.wrote);
  }
  function patchBoard() {
    const lanes = document.getElementById('lanes'); if (!lanes) return;
    const shown = shownSessions(), cs = pcs().filter((c) => st.pcF === 'all' || c.me);
    lanes.style.gridTemplateColumns = `repeat(${cs.length},minmax(0,1fr))`;
    for (const c of cs) {
      let lane = lanes.querySelector(`[data-lane="${c.id}"]`);
      if (!lane) {
        lane = h(`<div class="lane ${c.me ? 'me' : ''}" data-lane="${c.id}"><div class="lh"><i class="pc" style="${pcCss(c)}"></i><b>${esc(c.name)}</b>${c.me ? '<span class="me" title="this computer">here</span>' : ''}<span class="n"></span></div><div class="heard"></div><div class="cards"></div></div>`);
        lanes.append(lane);
      }
      lane.classList.toggle('quiet', !alive(c));
      lane.querySelector('.pc').classList.toggle('alive', alive(c));
      const mine = shown.filter((s) => s.computer === c.id).sort((x, y) => y.since - x.since); // stable: by start time, so a tick never reorders cards
      const open = !!st.more[c.id], vis = open ? mine : mine.slice(0, CAP), rest = mine.slice(vis.length);
      lane.querySelector('.lh .n').textContent = mine.length ? mine.length + (mine.length === 1 ? ' session' : ' sessions') : '';
      lane.querySelector('.heard').textContent = alive(c) ? '' : `last heard ${ago(c.seen)}`;
      const cards = lane.querySelector('.cards');
      for (const s of vis) {
        let el = cards.querySelector(`[data-session="${s.id}"]`);
        if (!el) { el = h(cardHTML(s)); cards.append(el); el.addEventListener('click', () => follow(s.id)); }
        patchCard(el, s);
      }
      cards.querySelectorAll('[data-session]').forEach((el) => { if (!vis.find((s) => s.id === el.dataset.session)) el.remove(); });
      let more = lane.querySelector('.more');
      if ((rest.length || open) && mine.length > CAP) {
        if (!more) { more = h('<button class="more"></button>'); lane.append(more); more.addEventListener('click', () => { st.more[c.id] = !st.more[c.id]; patchBoard(); }); }
        const wr = rest.filter((s) => s.wrote < 30e3).length;
        more.textContent = open ? 'Show fewer' : `+${rest.length} more${wr ? ` · ${wr} writing` : ''}`;
      } else if (more) more.remove();
      let empty = lane.querySelector('.empty');
      if (!mine.length && !empty) lane.append(h(`<div class="empty">${alive(c) ? 'Nothing running' : 'Quiet'}</div>`));
      if (mine.length && empty) empty.remove();
    }
    lanes.querySelectorAll('[data-lane]').forEach((l) => { if (!cs.find((c) => c.id === l.dataset.lane)) l.remove(); });
    const bc = document.getElementById('bcount'); if (bc) bc.textContent = shown.length === sessions().length ? '' : `showing ${shown.length} of ${sessions().length}`;
    if (st.follow) patchDrawer();
  }
  // the ladder: every account by room left, keyed by account id
  function ladHTML(a, use) {
    return `<li><button class="lad ${use ? 'use' : ''}" data-lad="${esc(a.id)}" title="Open ${esc(a.id)}"><span class="rc">${ring(a, 'ld')}<small>${a.windows ? 'week' : ''}</small></span>
      <span class="nm">${ti(a.tool)}<span>${esc(a.id)}</span></span><span class="rs" data-f="rs"></span><span class="wl" data-f="wl"></span></button></li>`;
  }
  function patchLad(el, a) {
    const rs = el.querySelector('[data-f=rs]'), wl = el.querySelector('[data-f=wl]');
    if (!a.windows) {
      rs.innerHTML = a.needsLogin ? '<span class="neg">login expired</span>' : `<span class="sta">${noQuota(a) ? 'no limits' : 'not read'}</span>`;
      wl.innerHTML = noQuota(a) ? `<span class="lbl">${esc(tool(a.tool).label)} has no limits · usage only</span>` : `<span class="lbl">limits never read</span><button class="readbtn" data-act="read" data-id="${esc(a.id)}">Read now</button>`;
      return;
    }
    // line 1: the week (the ring's window) and its reset; line 2: the window with the least room, its % left as the bar, its reset (and ≈ run-out)
    const w = weekWin(a), t = tightest(a), lt = Math.max(0, left(t)), tn = acctTone(a);
    rs.innerHTML = `<span title="the week resets ${at(w.reset)}">↻ <b>${at(w.reset)}</b></span>`;
    wl.innerHTML = `<span class="lbl ${tn}">${t.label} ${lt <= 0 ? 'exhausted' : lt + '% left'}</span><span class="bar" title="${t.label}: ${lt}% left"><i class="${tn}" style="width:${lt}%"></i></span><span class="sta" title="${t.label} resets ${at(t.reset)}${t.out ? ', runs out ≈ ' + at(t.out) + ' at today\'s pace' : ''}">${t.out ? `<span class="warn">≈ out ${at(t.out)}</span>${t.label.startsWith('Week') ? '' : ' · ↻ ' + dur(t.reset)}` : '↻ ' + dur(t.reset)}</span>`;
  }
  function patchLadder() {
    const ol = document.getElementById('lad'); if (!ol) return;
    const list = sorted().filter((a) => st.toolF === 'all' || a.tool === st.toolF), u = useNext();
    let prev = null;
    for (const a of list) {
      let li = ol.querySelector(`[data-lad="${a.id}"]`)?.closest('li');
      if (!li) { li = h(ladHTML(a, u && u.id === a.id)); li.querySelector('.lad').addEventListener('click', (e) => { if (e.target.closest('[data-act=read]')) return; detail(a.id); }); drawRings(li); }
      li.querySelector('.lad').classList.toggle('use', !!(u && u.id === a.id));
      patchLad(li, a);
      if (prev ? prev.nextElementSibling !== li : ol.firstElementChild !== li) ol.insertBefore(li, prev ? prev.nextElementSibling : ol.firstElementChild);
      prev = li;
    }
    ol.querySelectorAll('li').forEach((li) => { if (!list.find((a) => a.id === li.querySelector('.lad').dataset.lad)) li.remove(); });
    const lr = document.getElementById('lread'); if (lr) lr.textContent = `read ${ago(NOW - LIMITS_READ)}`;
  }

  function reads() {
    const el = document.getElementById('reads'); if (!el) return;
    const r = D.readouts[st.range];
    const d = (s) => `<span class="d ${s.startsWith('+') && !s.includes('$') ? 'up' : s.startsWith('−') ? 'dn' : ''}">${s ? s + ' vs before' : ''}</span>`;
    el.innerHTML = `<div class="lbl"><b>Readouts</b><div class="seg" role="group" aria-label="Range">${[['today', 'Today'], ['7d', '7 d'], ['30d', '30 d'], ['all', 'All']].map(([k, l]) => `<button data-range="${k}" aria-pressed="${st.range === k}">${l}</button>`).join('')}</div></div>
      <div class="ro"><span class="k">Tokens</span><span class="v">${fmt(r.tokens.v)}</span>${d(r.tokens.d)}${dotgrid(r.grid[0])}</div>
      <div class="ro"><span class="k">Cost</span><span class="v">${money(r.cost.v, 0)}<small>list</small></span>${d(r.cost.d)}${dotgrid(r.grid[1])}</div>
      <div class="ro"><span class="k">Cached</span><span class="v">${r.cached.v}<small>%</small></span>${d(r.cached.d)}${dotgrid(r.grid[2])}</div>`;
    el.querySelectorAll('[data-range]').forEach((b) => b.addEventListener('click', () => { st.range = b.dataset.range; reads(); }));
  }

  function yearClock() {
    const el = document.getElementById('yc'); if (!el) return;
    const val = (d) => (st.yc === 'all' ? d.tokens : d.by[st.yc]);
    const first = new Date(D.days[0].t); const mon = (first.getDay() + 6) % 7; const start = D.days[0].t - mon * DAY;
    const weeks = Math.ceil((D.days[D.days.length - 1].t - start) / DAY / 7) + 0;
    const max = Math.max(...D.days.map(val)) || 1, busiest = D.days.reduce((b, d) => (val(d) > val(b) ? d : b), D.days[0]);
    const S = 300, cx = 150, cy = 150, r0 = 58, rw = 11.5, gap = 1.2, step = (Math.PI * 2) / weeks;
    const arc = (r1, r2, a1, a2) => { const p = (r, a) => [cx + r * Math.cos(a), cy + r * Math.sin(a)]; const [x1, y1] = p(r1, a1), [x2, y2] = p(r1, a2), [x3, y3] = p(r2, a2), [x4, y4] = p(r2, a1); return `M${x1.toFixed(1)} ${y1.toFixed(1)}A${r1} ${r1} 0 0 1 ${x2.toFixed(1)} ${y2.toFixed(1)}L${x3.toFixed(1)} ${y3.toFixed(1)}A${r2} ${r2} 0 0 0 ${x4.toFixed(1)} ${y4.toFixed(1)}Z`; };
    let cells = '', months = '';
    for (const d of D.days) {
      const i = Math.round((d.t - start) / DAY), wk = Math.floor(i / 7), dw = i % 7, a1 = -Math.PI / 2 + wk * step, a2 = a1 + step - 0.012;
      const v = val(d) / max, r1 = r0 + dw * rw, r2 = r1 + rw - gap;
      cells += `<path d="${arc(r1, r2, a1, a2)}" fill="${v ? 'var(--teal)' : 'var(--line)'}" opacity="${v ? (0.12 + v * 0.88).toFixed(2) : 1}"><title>${new Date(d.t).toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })}: ${fmt(val(d))} tokens</title></path>`;
      const dt = new Date(d.t);
      if (dt.getDate() === 1) { const a = a1 + step / 2, R = r0 + 7 * rw + 8; months += `<text x="${(cx + R * Math.cos(a)).toFixed(1)}" y="${(cy + R * Math.sin(a)).toFixed(1)}" font-size="9" fill="var(--dim)" text-anchor="middle" dominant-baseline="middle">${'JFMAMJJASOND'[dt.getMonth()]}</text>`; }
    }
    el.innerHTML = `<svg viewBox="0 0 ${S} ${S}" role="img" aria-label="Daily tokens over 53 weeks">${cells}${months}
      <text x="${cx}" y="${cy - 12}" font-size="9" fill="var(--dim)" text-anchor="middle">busiest day</text>
      <text x="${cx}" y="${cy + 4}" font-size="13" font-weight="600" fill="var(--ink)" text-anchor="middle">${new Date(busiest.t).toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })}</text>
      <text x="${cx}" y="${cy + 19}" font-size="10" fill="var(--mute)" text-anchor="middle">${fmt(val(busiest))}</text></svg><span class="cap">Monday inside · daily tokens${st.yc === 'all' ? '' : ' · ' + esc(pc(st.yc).name)}</span>`;
  }

  function replies() {
    const el = document.getElementById('replies'); if (!el) return;
    const rows = st.log ? D.replies : D.replies.slice(0, 8);
    // no tokens-per-second on the Overview (brief rule 2): the speed column lives on the Sessions page's log only
    el.innerHTML = `<div class="ph"><h2>${st.log ? 'Request log' : 'Latest replies'}</h2><span class="sub">${D.repliesToday.toLocaleString('en-US')} today · first reply is approximate</span><div class="right"><button class="btn ghost sm" data-act="log">${st.log ? 'Latest replies' : 'Request log'}</button></div></div>
      <div class="tblwrap"><table class="rep ${st.log ? 'log' : ''}"><thead><tr><th>Time</th><th>Account</th><th>Model</th><th>Computer</th><th class="num">Tokens</th><th class="num">First reply</th></tr></thead><tbody>
      ${rows.map((r) => { const c = pc(r.computer); return `<tr><td>${clock(r.ts)}</td><td><span class="acc">${ti(r.tool)}${esc(r.account)}</span></td><td class="mute">${esc(r.model)}</td><td><span class="pc"><i style="${pcCss(c)}"></i>${esc(c.name)}</span></td><td class="num">${fmt(r.tokens)}</td><td class="num mute">≈ ${r.first} s</td></tr>`; }).join('')}</tbody></table></div>`;
    el.querySelector('[data-act=log]').addEventListener('click', () => { st.log = !st.log; replies(); });
  }

  // ---------- follow drawer ----------
  function follow(id) {
    st.follow = st.follow === id ? null : id;
    document.querySelectorAll('.card').forEach((c) => c.setAttribute('aria-pressed', c.dataset.session === st.follow));
    const ov = document.getElementById('overlay'); ov.querySelector('.drawer')?.remove();
    if (!st.follow) return;
    st.opener = document.activeElement;
    const s = D.sessions.find((x) => x.id === id), a = acct(s.account), c = pc(s.computer);
    ov.append(h(`<aside class="drawer" role="dialog" aria-label="Following ${esc(s.project)}" data-drawer="follow">
      <div class="dh"><h2>${ti(s.tool)}<span>${esc(s.project)}</span></h2><span class="tag teal" data-f="state"></span><button class="ib x" data-act="close" aria-label="Close">${ic('x', 16)}</button></div>
      <div class="db">
        <div class="sec"><h3>This session</h3><dl class="kv"><dt>Account</dt><dd>${esc(s.account)} · ${esc(tool(s.tool).label)}</dd><dt>Computer</dt><dd><span style="display:inline-flex;align-items:center;gap:6px"><i style="width:8px;height:8px;border-radius:50%;background:${c.col};display:inline-block"></i>${esc(c.name)}</span></dd><dt>Model</dt><dd>${esc(s.model)}</dd><dt>Running for</dt><dd data-f="since"></dd><dt>Used</dt><dd><span data-f="tokens"></span> tokens · <span data-f="cost"></span> at list prices</dd><dt>First reply</dt><dd>≈ ${s.first} s</dd><dt>Last wrote</dt><dd data-f="wrote"></dd></dl></div>
        <div class="sec"><h3>${esc(a.id)}'s limits</h3>${a.windows ? `<div class="wins">${a.windows.map(winHTML).join('')}</div>` : '<span class="dim">Limits never read.</span>'}</div>
        <div class="sec"><h3>Latest replies on this session</h3><table class="rep"><tbody>${D.replies.filter((r) => r.account === s.account && r.computer === s.computer).slice(0, 6).map((r) => `<tr><td>${clock(r.ts)}</td><td class="mute">${esc(r.model)}</td><td class="num">${fmt(r.tokens)}</td><td class="num mute">≈ ${r.first} s</td></tr>`).join('') || '<tr><td class="dim">No reply in the range yet.</td></tr>'}</tbody></table></div>
        <div class="sec" style="display:flex;gap:8px;flex-wrap:wrap"><button class="btn sm" data-act="toast" data-msg="Window of ${esc(s.project)} brought to front">${ic('window', 14)} Show window</button><button class="btn sm" data-act="toast" data-msg="Opened the project folder">${ic('folder', 14)} Open project folder <span class="tag proposed">proposed</span></button><button class="btn sm ghost" data-act="page" data-page="sessions">All sessions</button></div>
      </div></aside>`));
    patchDrawer(); drawRings(ov); trap(ov.querySelector('.drawer')); ov.querySelector('.drawer .x').focus();
  }
  function patchDrawer() {
    const d = document.querySelector('[data-drawer=follow]'); if (!d) return;
    const s = D.sessions.find((x) => x.id === st.follow); if (!s) return;
    d.querySelector('[data-f=state]').textContent = s.wrote < 30e3 ? 'writing' : 'idle';
    d.querySelector('[data-f=since]').textContent = dur(s.since);
    d.querySelector('[data-f=tokens]').textContent = fmt(s.tokens);
    d.querySelector('[data-f=cost]').textContent = money(s.cost);
    d.querySelector('[data-f=wrote]').textContent = ago(s.wrote);
  }
  // one window: % left as the bar, the reset always, the ≈ run-out beside it when the pace runs it out first
  const winHTML = (w) => { const l = Math.max(0, left(w)), tn = l <= 0 ? 'neg' : w.out ? 'warn' : ''; return `<div class="win"><span>${esc(w.label)}</span><span class="bar"><i class="${tn}" style="width:${l}%"></i></span><span class="pct ${tn}">${l <= 0 ? 'exhausted' : l + '% left'}</span><span class="to">↻ ${dur(w.reset)}${w.out ? ` · <span class="warn">≈ out ${at(w.out)}</span>` : ''}</span></div>`; };

  // ---------- Accounts ----------
  function accountsPage() {
    const main = document.getElementById('main'), ac = accounts(), u = useNext(), byTool = D.tools.filter((t) => ac.some((a) => a.tool === t.id));
    const q = st.q;
    const head = u ? `<span class="teal">${esc(u.id)}</span> has the most room: ${u.windows.map((w) => w.used + '% of its ' + w.label.toLowerCase()).slice(0, 2).join(' and ')} used.` : ac.length ? 'No limits read yet.' : 'No accounts yet.';
    main.innerHTML = `<div class="wrap">
      <div class="pagehead" data-tour="ahead"><div class="lead"><h1>${head}</h1><div class="basis">${ac.length} account${ac.length === 1 ? '' : 's'} across ${byTool.length} tool${byTool.length === 1 ? '' : 's'}, each in its own folder with its own login · sorted by room left · limits read ${ac.some((a) => a.read) ? ago(NOW - LIMITS_READ) : 'never'} · usage at API list prices (≈)</div></div>
        <div class="acts"><div class="seg" role="group" aria-label="Range">${['7d', '14d', '30d'].map((k) => `<button data-arange="${k}" aria-pressed="${st.arange === k}">${k.replace('d', ' d')}</button>`).join('')}</div><button class="btn" data-act="toast" data-msg="accounts-${st.arange}.csv saved to Downloads">Export CSV</button><button class="btn" data-act="startin">Start in a folder…</button><button class="btn primary" data-act="add">${ic('plus', 14)} Add account</button></div></div>
      ${ac.length ? `<details class="panel fold"><summary class="ph">${ic('chev', 16).replace('<svg', '<svg class="chev"')}<h2>Tokens per day, by account</h2><span class="sub">${st.arange.replace('d', ' days')}</span></summary><div class="body">${stackChart(ac)}</div></details>
      <div class="bh"><div class="chips" role="group" aria-label="Tool filter"><button class="chip" data-tf="all" aria-pressed="${st.toolF === 'all'}">All</button>${byTool.map((t) => `<button class="chip" data-tf="${t.id}" aria-pressed="${st.toolF === t.id}">${ti(t.id)}<span>${esc(t.label)}</span></button>`).join('')}</div><span class="mute" style="font-size:12px">Sorted by room left · limits read ${ac.some((a) => a.read) ? ago(NOW - LIMITS_READ) : 'never'}</span></div>
      ${byTool.filter((t) => st.toolF === 'all' || t.id === st.toolF).map((t) => group(t, sorted().filter((a) => a.tool === t.id && (!q || (a.id + ' ' + a.email).toLowerCase().includes(q))))).join('')}
      <details class="panel fold"><summary class="ph">${ic('chev', 16).replace('<svg', '<svg class="chev"')}<h2>By model</h2><span class="sub">${st.arange.replace('d', ' days')} · list prices</span></summary><div class="body"><div class="bym"><span class="h">Model</span><span class="h num">Tokens</span><span class="h num">Cost</span>${D.byModel.map(([m, t, c]) => `<span>${esc(m)}</span><span class="num">${fmt(t)}</span><span class="num">${money(c)}</span>`).join('')}</div></div></details>` : `<div class="placeholder"><h1>No accounts yet</h1><p>Add one, or install a tool and its main profile appears here by itself.</p><button class="btn primary" data-act="add">${ic('plus', 14)} Add account</button></div>`}
    </div>`;
    drawRings(main);
    main.querySelectorAll('[data-tf]').forEach((b) => b.addEventListener('click', () => { st.toolF = b.dataset.tf; accountsPage(); }));
    main.querySelectorAll('[data-arange]').forEach((b) => b.addEventListener('click', () => { st.arange = b.dataset.arange; accountsPage(); }));
    main.querySelectorAll('.row').forEach((r) => r.addEventListener('click', (e) => { if (e.target.closest('button')) return; detail(r.dataset.row); }));
  }
  const PAL = ['#3FD6CF', '#7F82E8', '#6FA8F5', '#C8A45A', '#E384B8', '#5ED39A', '#E0A84C', '#C792EA'];
  const acol = (id) => PAL[D.accounts.findIndex((a) => a.id === id) % PAL.length];
  function stackChart(ac) {
    const max = Math.max(...D.perDay.map((d) => ac.reduce((s, a) => s + (d.by[a.id] || 0), 0))) || 1;
    return `<div class="stack">${D.perDay.map((d, i) => `<div class="day">${ac.map((a) => `<i style="--c:${acol(a.id)};height:${((d.by[a.id] || 0) / max * 100).toFixed(1)}%" title="${esc(a.id)}: ${fmt(d.by[a.id] || 0)}"></i>`).join('')}${i % 2 === 0 ? `<small>${i === 6 ? 'today' : new Date(d.t).toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })}</small>` : ''}</div>`).join('')}</div><div class="legend">${ac.map((a) => `<span><i style="--c:${acol(a.id)}"></i>${esc(a.id)}</span>`).join('')}</div>`;
  }
  function group(t, list) {
    if (!list.length) return '';
    const open = list.reduce((s, a) => s + a.open, 0), tok = list.reduce((s, a) => s + a.tokens7, 0), cost = list.reduce((s, a) => s + a.cost7, 0);
    const next = list.flatMap((a) => a.windows || []).sort((x, y) => x.reset - y.reset)[0];
    return `<details class="panel grp fold" open><summary class="ph"><span class="ti">${ti(t.id)}</span><h2>${esc(t.label)}</h2><span class="sub">${list.length}</span><span class="facts"><span><b>${list.filter((a) => !a.needsLogin && a.read).length}</b> logged in</span><span><b>${open}</b> consoles open</span><span>next reset <b>${next ? at(next.reset) : '—'}</b></span><span><b>${fmt(tok)}</b> tokens</span><span><b>${money(cost)}</b></span></span>${ic('chev', 16).replace('<svg', '<svg class="chev"')}</summary><div class="body">${list.map(rowHTML).join('')}</div></details>`;
  }
  function rowHTML(a) {
    const u = useNext(), sp = Math.max(...a.spark, 0.01);
    return `<div class="row ${u && u.id === a.id ? 'use' : ''}" data-row="${esc(a.id)}" role="button" tabindex="0">
      ${ring(a, 'lg')}
      <div class="id"><b>${ti(a.tool)}${esc(a.id)}${a.label ? `<span class="lab">· ${esc(a.label)}</span>` : ''}</b><div class="em">${esc(a.email)}</div><div class="pl">${a.plan ? `<span class="tag">${esc(a.plan)}</span>` : a.needsLogin ? '<span class="tag neg">login expired</span>' : a.kind === 'api' ? '<span class="tag">API key</span>' : a.kind === 'routed' ? '<span class="tag">routed</span>' : '<span class="tag">plan unknown</span>'}<span>Added ${esc(a.added)}</span></div></div>
      <div class="wins">${a.windows ? a.windows.map(winHTML).join('') : `<div class="win"><span>Limits</span><span class="bar"><i class="dash" style="width:100%"></i></span><span class="pct">—</span><span class="to">${noQuota(a) ? 'none on ' + esc(tool(a.tool).label) : 'not read'}</span></div>`}</div>
      <div class="spark" aria-hidden="true">${a.spark.map((v) => `<i style="height:${Math.max(4, v / sp * 100)}%"></i>`).join('')}</div>
      <div class="num"><b>${fmt(a.tokens7)}</b><small>tokens · ${st.arange.replace('d', ' days')}</small></div>
      <div class="num"><b>${money(a.cost7)}</b><small>list price</small></div>
      <div class="acts"><span class="dim" style="font-size:11px;margin-right:4px">${a.read ? ago(NOW - a.read) : '—'}</span><button class="ib" data-act="read" data-id="${esc(a.id)}" aria-label="Read the limits now" title="Read the limits now">${ic('refresh', 14)}</button><button class="btn primary sm" data-act="start" data-id="${esc(a.id)}" ${tool(a.tool).installed && !a.needsLogin ? '' : 'disabled'}>Start</button><button class="ib" data-act="rowmenu" data-id="${esc(a.id)}" aria-label="More" aria-haspopup="menu">${ic('more', 16)}</button></div>
      <div class="adv">${a.advice ? `<span class="${/Most room/.test(a.advice) ? 'teal' : /exhausted/.test(a.advice) ? 'neg' : /out/.test(a.advice) ? 'warn' : ''}">${esc(a.advice)}</span>` : a.needsLogin ? '<span class="neg">Sign in again to read its limits.</span>' : !a.windows ? `<span class="dim">${noQuota(a) ? 'Usage is counted; this tool has no limit windows.' : 'Limits appear after the first read.'}</span>` : ''}${a.open ? `<span class="op">● ${a.open} open</span>` : ''}</div>
    </div>`;
  }

  function detail(id) {
    const a = acct(id); if (!a) return;
    st.detail = id; st.dtab = 'overview'; st.opener = document.activeElement;
    const ov = document.getElementById('overlay'); ov.querySelector('.drawer')?.remove();
    const ss = D.sessions.filter((s) => s.account === id);
    const d = h(`<aside class="drawer" role="dialog" aria-label="${esc(a.id)}" data-drawer="detail">
      <div class="dh"><h2>${ti(a.tool)}<span>${esc(a.id)}</span>${a.plan ? `<span class="tag">${esc(a.plan)}</span>` : ''}</h2><button class="ib x" data-act="close" aria-label="Close">${ic('x', 16)}</button></div>
      <div class="dtabs" role="tablist">${['overview', 'sessions', 'projects', 'settings'].map((t) => `<button role="tab" data-dtab="${t}" aria-selected="${t === st.dtab}">${t[0].toUpperCase() + t.slice(1)}</button>`).join('')}</div>
      <div class="db" id="dbody"></div></aside>`);
    ov.append(d);
    const body = () => {
      const b = d.querySelector('#dbody');
      if (st.dtab === 'overview') b.innerHTML = `<div class="sec"><h3>Limits</h3>${a.windows ? `<div class="wins">${a.windows.map(winHTML).join('')}</div>` : '<span class="dim">Never read.</span>'}</div>
        <div class="sec"><h3>Tokens per day, 30 days</h3><div class="spark" style="height:48px">${Array.from({ length: 30 }, (_, i) => `<i style="height:${Math.max(3, Math.round(20 + 70 * Math.abs(Math.sin(i * 1.7 + a.id.length))))}%"></i>`).join('')}</div></div>
        <div class="sec"><h3>By model</h3><div class="bym" style="padding:0">${D.byModel.filter(([m]) => (a.tool === 'claude' ? m.startsWith('claude') : a.tool === 'codex' ? m.startsWith('gpt') : !m.startsWith('claude') && !m.startsWith('gpt'))).map(([m, t, c]) => `<span>${esc(m)}</span><span class="num">${fmt(t)}</span><span class="num">${money(c)}</span>`).join('') || '<span class="dim">Nothing yet</span>'}</div></div>
        <div class="sec"><h3>Login and folder</h3><dl class="kv"><dt>Email</dt><dd>${esc(a.email)}</dd><dt>Kind</dt><dd>${esc(a.kind)}${a.main ? ' · main profile' : ''}</dd><dt>Folder</dt><dd class="mono">${a.main ? esc(tool(a.tool).main) : '~/.' + esc(a.tool) + '-' + esc(a.id)}</dd><dt>Added</dt><dd>${esc(a.added)}</dd><dt>Limits read</dt><dd>${a.read ? ago(NOW - a.read) : '—'}</dd></dl></div>
        <div class="sec"><h3>Open now</h3>${ss.length ? ss.map((s) => `<div class="kv" style="grid-template-columns:1fr auto"><dt><b style="color:var(--ink)">${esc(s.project)}</b> · ${esc(pc(s.computer).name)}</dt><dd>${dur(s.since)}</dd></div>`).join('') : '<span class="dim">Nothing</span>'}</div>
        <div class="sec"><h3>Recent sessions</h3><dl class="kv"><dt>Yesterday</dt><dd>${esc(a.tool === 'codex' ? 'sanduq-api, sanduq-ci' : 'HarnessManager, resal-wiki')}</dd><dt>Nov 4</dt><dd>${esc(a.tool === 'codex' ? 'sanduq-api' : 'model-adoption')}</dd></dl></div>`;
      else if (st.dtab === 'sessions') b.innerHTML = `<div class="sec"><h3>Sessions</h3>${ss.length ? `<table class="rep"><tbody>${ss.map((s) => `<tr><td><b>${esc(s.project)}</b></td><td>${esc(pc(s.computer).name)}</td><td class="num">${dur(s.since)}</td><td class="num">${fmt(s.tokens)}</td></tr>`).join('')}</tbody></table>` : '<span class="dim">No session running on this account.</span>'}</div>`;
      else if (st.dtab === 'projects') b.innerHTML = `<div class="sec"><h3>Projects</h3>${[...new Set(ss.map((s) => s.project))].map((p) => `<div class="kv" style="grid-template-columns:1fr auto;padding:4px 0"><dt><b style="color:var(--ink)">${esc(p)}</b></dt><dd>${ss.filter((s) => s.project === p).length} live</dd></div>`).join('') || '<span class="dim">None yet</span>'}</div>`;
      else b.innerHTML = `<form class="sec" onsubmit="return false"><h3>Settings</h3><div class="field"><label for="d-email">Email</label><input id="d-email" value="${esc(a.email)}"><label for="d-label">Label</label><input id="d-label" value="${esc(a.label)}"><label for="d-key">API key</label><input id="d-key" placeholder="${a.kind === 'api' ? 'sk-…' : 'not used by a subscription account'}" ${a.kind === 'api' ? '' : 'disabled'}><label for="d-ep">Endpoint</label><input id="d-ep" placeholder="default" ${a.kind === 'api' ? '' : 'disabled'}><label for="d-cap">Cost cap</label><input id="d-cap" placeholder="none"></div>
        <div style="display:flex;gap:8px;margin-top:14px;flex-wrap:wrap"><button class="btn primary sm" data-act="toast" data-msg="Saved">Save</button>${a.main ? `<button class="btn sm" data-act="toast" data-msg="${esc(a.id)} hidden from the list; the tool's own login stays">Hide from the list</button>` : `<button class="btn sm" data-act="toast" data-msg="${esc(a.id)} removed from the list; its folder stays">Remove from the list</button><button class="btn sm neg" data-act="toast" data-msg="${esc(a.id)} removed with its folder">Remove with its folder</button>`}</div>${a.main ? '<p class="dim" style="margin-top:10px;font-size:12px">A main profile is the tool\'s own login; it cannot be removed, only hidden.</p>' : ''}</form>`;
      drawRings(b);
    };
    d.querySelectorAll('[data-dtab]').forEach((t) => t.addEventListener('click', () => { st.dtab = t.dataset.dtab; d.querySelectorAll('[data-dtab]').forEach((x) => x.setAttribute('aria-selected', x === t)); body(); }));
    body(); trap(d); d.querySelector('[data-dtab]').focus();
  }

  // ---------- Add account ----------
  function addDialog(fromFirstRun) {
    const ov = document.getElementById('overlay'); ov.querySelector('.scrim')?.remove();
    st.opener = document.activeElement;
    const f = { step: 1, tool: 'claude', kind: 'subscription', name: '', email: '', key: '', endpoint: '', model: '', route: D.routes[0] };
    const KINDS = [['subscription', 'Subscription', 'Pro, Max, Team or Enterprise. You sign in in the browser or a terminal.'], ['api', 'API key', 'A key from the provider; usage is billed to it.'], ['routed', 'Routed', 'Requests go through the local router to a provider such as OpenRouter or DeepSeek.']];
    const scrim = h('<div class="scrim" data-dialog="add"></div>');
    const dlg = h('<div class="dialog" role="dialog" aria-modal="true" aria-labelledby="add-h"></div>');
    scrim.append(dlg); ov.append(scrim); trap(dlg);
    const draw = () => {
      const t = tool(f.tool), ok = t.kinds.includes(f.kind);
      dlg.innerHTML = `<div class="dh"><h2 id="add-h">Add account</h2><span class="steps" aria-label="Step ${f.step} of 3"><i class="${f.step >= 1 ? 'on' : ''}"></i><i class="${f.step >= 2 ? 'on' : ''}"></i><i class="${f.step >= 3 ? 'on' : ''}"></i></span><span class="mute">${['', 'Which tool', 'Which kind', f.kind === 'subscription' ? 'Name and email' : f.kind === 'api' ? 'Name and key' : 'Name and route', 'Signing in'][f.step]}</span><button class="ib x" data-act="close" aria-label="Close">${ic('x', 16)}</button></div>
      <form class="db" id="addf">${f.step === 1 ? `<div class="tiles" role="group" aria-label="Tool">${D.tools.map((x) => `<button type="button" class="tile" data-tool="${x.id}" aria-pressed="${f.tool === x.id}"><span class="t">${ti(x.id)}${esc(x.label)}<small>${x.installed ? 'v' + x.ver : 'not installed'}</small></span><span class="kinds">${KINDS.map(([k, l]) => `<i class="${x.kinds.includes(k) ? 'on' : ''}" title="${x.kinds.includes(k) ? esc(x.label) + ' takes a ' + l.toLowerCase() + ' account' : esc(x.label) + ' does not take a ' + l.toLowerCase() + ' account'}">${l}</i>`).join('')}</span></button>`).join('')}</div><p class="dim" style="font-size:12px">A tool that is not installed can still get an account prepared now; it starts once the tool is installed.</p>`
        : f.step === 2 ? `<div class="kcards" role="group" aria-label="Kind">${KINDS.map(([k, l, d]) => { const can = t.kinds.includes(k); return `<button type="button" class="kcard" data-kind="${k}" aria-pressed="${f.kind === k}" ${can ? '' : 'disabled'}><b>${l}</b><span>${d}</span>${can ? '' : `<span class="no">Not for ${esc(t.label)}</span>`}</button>`; }).join('')}</div>`
        : f.step === 3 ? `<div class="field"><label for="a-name">Name</label><input id="a-name" value="${esc(f.name)}" placeholder="work-design" required autofocus><span class="hint">Folder <code>~\\.${esc(f.tool)}-${esc(f.name || '<name>')}</code></span>
          ${f.kind === 'subscription' ? `<label for="a-email">Email</label><input id="a-email" type="email" value="${esc(f.email)}" placeholder="optional"><span class="hint">Optional. If given, it is checked after you log in, so a wrong account is caught.</span>` : ''}
          ${f.kind === 'api' ? `<label for="a-key">API key</label><input id="a-key" value="${esc(f.key)}" placeholder="sk-…" required><label for="a-ep">Endpoint</label><input id="a-ep" value="${esc(f.endpoint)}" placeholder="optional"><label for="a-model">Model</label><input id="a-model" value="${esc(f.model)}" placeholder="optional"></span>` : ''}
          ${f.kind === 'routed' ? `<label for="a-route">Route</label><select id="a-route">${D.routes.map((r) => `<option ${f.route === r ? 'selected' : ''}>${esc(r)}</option>`).join('')}</select><span class="hint">From the routes on the API keys &amp; routing page.</span><label for="a-model">Model</label><input id="a-model" value="${esc(f.model)}" placeholder="optional">` : ''}</div>
          <details class="facts"><summary>What this account shares with your main profile</summary><dl class="kv"><dt>Kept in sync</dt><dd>CLAUDE.md, rules, skills, plugins and MCP servers, from your main profile</dd><dt>Its own</dt><dd>Login, history and usage, so each account's numbers stay separate</dd><dt>CLI</dt><dd class="mono">hrm add ${esc(f.name || '<name>')} --tool ${esc(f.tool)} --kind ${esc(f.kind)}${f.kind === 'routed' ? ' --route ' + esc(f.route) : ''}</dd></dl></details>`
        : `<div class="signin">${f.kind !== 'subscription' ? `<div class="st">${ic('check', 16)} ${esc(f.name)} added as a${f.kind === 'api' ? 'n API-key' : ' routed'} account on ${esc(t.label)}.</div><p class="mute">It appears in the list; its usage is read as it is used.</p>` : f.tool === 'claude' ? `<div class="st"><i></i> Signing in to Claude Code as ${esc(f.name)}</div><p class="mute">1. Open the sign-in page in your browser. 2. Paste the code it gives you here.</p><button type="button" class="btn sm" data-act="toast" data-msg="Sign-in page opened in the browser">${ic('link', 14)} Open the sign-in page</button><div class="field" style="width:100%"><label for="a-code">Code</label><input id="a-code" placeholder="paste the code"></div>` : `<div class="st"><i></i> A terminal window is open: finish the sign-in to ${esc(t.label)} there.</div><p class="mute">When it is done, the plan and the limits appear on ${esc(f.name)}'s row.</p>`}</div>`}
      </form>
      <div class="df"><span class="sp"></span>
        ${f.step > 1 && f.step < 4 ? `<button class="btn" data-act="back">${ic('back', 14)} Back</button>` : '<button class="btn" data-act="close">Cancel</button>'}
        ${f.step < 3 ? `<button class="btn primary" data-act="next" ${f.step === 2 && !ok ? 'disabled' : ''}>Next</button>` : f.step === 3 ? `<button class="btn primary" data-act="next">${f.kind === 'subscription' ? 'Create and log in' : 'Add'}</button>` : `<button class="btn primary" data-act="done">${f.kind === 'subscription' && f.tool === 'claude' ? 'Finish sign-in' : 'Done'}</button>`}</div>`;
      dlg.querySelectorAll('[data-tool]').forEach((b) => b.addEventListener('click', () => { f.tool = b.dataset.tool; if (!tool(f.tool).kinds.includes(f.kind)) f.kind = tool(f.tool).kinds[0]; draw(); }));
      dlg.querySelectorAll('[data-kind]').forEach((b) => b.addEventListener('click', () => { f.kind = b.dataset.kind; draw(); }));
      const read = () => { for (const [k, id] of [['name', 'a-name'], ['email', 'a-email'], ['key', 'a-key'], ['endpoint', 'a-ep'], ['model', 'a-model'], ['route', 'a-route']]) { const el = dlg.querySelector('#' + id); if (el) f[k] = el.value.trim(); } };
      dlg.querySelector('#a-name')?.addEventListener('input', (e) => { f.name = e.target.value.trim(); dlg.querySelector('.hint code').textContent = `~\\.${f.tool}-${f.name || '<name>'}`; dlg.querySelector('.facts .mono').textContent = `hrm add ${f.name || '<name>'} --tool ${f.tool} --kind ${f.kind}${f.kind === 'routed' ? ' --route ' + f.route : ''}`; });
      dlg.querySelector('[data-act=next]')?.addEventListener('click', () => { read(); const nm = dlg.querySelector('#a-name'); if (nm) { nm.setCustomValidity(accounts().some((a) => a.id === f.name) ? 'An account with this name exists' : ''); if (!dlg.querySelector('#addf').reportValidity()) return; } f.step++; draw(); });
      dlg.querySelector('[data-act=back]')?.addEventListener('click', () => { read(); f.step--; draw(); });
      dlg.querySelector('#addf').addEventListener('submit', (e) => { e.preventDefault(); dlg.querySelector('[data-act=next],[data-act=done]')?.click(); });
      // the entered account joins the list with its limits not read (a subscription reads them after the sign-in; an API-key or routed account has usage only)
      dlg.querySelector('[data-act=done]')?.addEventListener('click', () => {
        st.added.push({ id: f.name, tool: f.tool, kind: f.kind, plan: null, email: f.email || '—', added: 'today', label: f.kind === 'routed' ? 'via ' + f.route : f.kind === 'api' ? 'API key' : '', windows: null, read: null, tokens7: 0, cost7: 0, spark: [0, 0, 0, 0, 0, 0, 0], open: 0, route: f.route, endpoint: f.endpoint, model: f.model });
        closeAll(); toast(`${f.name} added on ${tool(f.tool).label}${f.kind === 'subscription' ? ' · plan and limits appear on its row after the sign-in' : ''}`);
        st.q = ''; shell(); go('accounts'); document.querySelector(`[data-row="${CSS.escape(f.name)}"]`)?.scrollIntoView({ block: 'center' });
      });
      (dlg.querySelector('[autofocus]') || dlg.querySelector('[aria-pressed=true]') || dlg.querySelector('button')).focus();
    };
    draw();
  }
  function startIn() {
    const ov = document.getElementById('overlay'); ov.querySelector('.scrim')?.remove();
    st.opener = document.activeElement;
    const a = useNext();
    const scrim = h(`<div class="scrim" data-dialog="startin"><div class="dialog" role="dialog" aria-modal="true" aria-label="Start in a folder" style="width:min(520px,94%)"><div class="dh"><h2>Start in a folder</h2><button class="ib x" data-act="close" aria-label="Close">${ic('x', 16)}</button></div>
      <form class="db" onsubmit="return false"><div class="field"><label for="s-acc">Account</label><select id="s-acc">${sorted().filter((x) => tool(x.tool).installed).map((x) => `<option ${a && a.id === x.id ? 'selected' : ''}>${esc(x.id)}</option>`).join('')}</select><label for="s-dir">Folder</label><select id="s-dir"><option>D:\\Projects\\abushanab-net\\HarnessManager</option><option>D:\\Projects\\sanduq\\api</option><option>D:\\Projects\\resal\\wiki</option><option>Browse…</option></select><span class="hint">Recent folders of this account</span><label for="s-args">Extra arguments</label><input id="s-args" placeholder="optional"><label></label><label style="display:flex;gap:8px;align-items:center;color:var(--ink2)"><input type="checkbox" style="width:auto;height:auto"> Skip permission prompts</label></div></form>
      <div class="df"><span class="sp"></span><button class="btn" data-act="close">Cancel</button><button class="btn primary" data-act="toast" data-msg="Terminal opened in the folder" data-close="1">${ic('play', 12)} Start</button></div></div></div>`);
    ov.append(scrim); trap(scrim.querySelector('.dialog')); scrim.querySelector('select').focus();
  }
  // keep Tab inside an open dialog or drawer; closeAll() gives focus back to the control that opened it
  function trap(el) {
    el.addEventListener('keydown', (e) => {
      if (e.key !== 'Tab') return;
      const f = [...el.querySelectorAll('button:not([disabled]),input:not([disabled]),select:not([disabled]),summary,[tabindex="0"]')].filter((x) => x.offsetParent !== null);
      if (!f.length) return;
      const i = f.indexOf(document.activeElement);
      if (e.shiftKey && i <= 0) { e.preventDefault(); f[f.length - 1].focus(); } else if (!e.shiftKey && (i === -1 || i === f.length - 1)) { e.preventDefault(); f[0].focus(); }
    });
  }

  // ---------- first run ----------
  function firstRun(main) {
    const fr = D.firstRun[st.data], found = fr.tools.map(tool);
    main.innerHTML = `<div class="wrap">
    <section class="board ghostboard" aria-label="The board, empty"><div class="bh"><h2>Running now <span class="n">nothing yet</span></h2></div><div class="lanes"><div class="lane me"><div class="lh"><i class="pc alive" style="${pcCss(me)}"></i><b>${esc(me.name)}</b><span class="me" title="this computer">here</span></div><div class="empty">Sessions you start appear here, with their account and project.</div></div></div></section>
    <section class="first">
      <div class="panel hero"><h1>${esc(fr.message)}</h1>
        ${found.length ? `<div class="found">${found.map((t) => `<div class="fr">${ti(t.id)}<div><b>${esc(t.id)}-main</b><div class="path">${esc(t.main)} · v${esc(t.ver)}</div></div><span class="st">${ring({ id: t.id + '-main', tool: t.id, windows: null }, 'sm')}</span><button class="btn sm" data-act="toast" data-msg="Terminal opened: sign in to ${esc(t.label)} there">Sign in</button></div>`).join('')}</div>` : ''}
        <div class="acts"><button class="btn primary" data-act="add" data-first="1">${ic('plus', 14)} Add account</button><button class="btn" data-act="page" data-page="accounts">Accounts</button></div></div>
      <div class="panel sync"><h2>Sync with your other computers?</h2>
        ${[['shared', 'A shared folder', 'OneDrive, Dropbox or a network drive'], ['hub', 'A hub you host', 'Harness Manager on a server you own'], ['later', 'Not now', 'Settings can turn it on later']].map(([k, b, s]) => `<button class="opt" data-sync="${k}" aria-pressed="${st.sync === k}"><b>${b}</b><span>${s}</span></button>`).join('')}</div>
    </section></div>`;
    main.querySelectorAll('[data-sync]').forEach((b) => b.addEventListener('click', () => { st.sync = b.dataset.sync; main.querySelectorAll('[data-sync]').forEach((x) => x.setAttribute('aria-pressed', x === b)); toast(b.dataset.sync === 'later' ? 'You can turn sync on in Settings' : 'Sync set up: ' + b.querySelector('b').textContent); }));
  }

  // ---------- placeholder pages ----------
  const placeholder = (name) => { document.getElementById('main').innerHTML = `<div class="wrap"><div class="placeholder"><h1>${esc(name)}</h1><p>This page keeps what it does today; it is not part of this round.</p><span class="tag">not in this round</span></div></div>`; };

  // ---------- menus, tour, toasts ----------
  function menu(anchor, items) {
    closeMenus();
    const r = anchor.getBoundingClientRect(), ar = app.getBoundingClientRect(), k = ar.width / app.offsetWidth;
    const m = h(`<div class="menu" role="menu">${items.map((i) => (i === 'hr' ? '<hr>' : i.ver ? `<div class="ver">${i.ver}</div>` : `<button class="mi" role="menuitem" data-m="${i.id || ''}">${i.dot ? '<i class="dot"></i>' : ''}${i.icon ? ic(i.icon, 14) : ''}<span>${i.label}</span>${i.n != null ? `<span class="n ${i.warn ? 'warn' : ''}">${i.n}</span>` : ''}</button>`)).join('')}</div>`);
    m.style.left = Math.min((r.left - ar.left) / k, app.offsetWidth - 240) + 'px'; m.style.top = (r.bottom - ar.top) / k + 4 + 'px';
    app.append(m);
    m.querySelectorAll('[data-m]').forEach((b, i) => b.addEventListener('click', () => { const it = items.filter((x) => x !== 'hr' && !x.ver)[i]; closeMenus(); it.run && it.run(); }));
    m.querySelector('button')?.focus();
  }
  const closeMenus = () => app.querySelectorAll('.menu').forEach((m) => m.remove());
  function toast(msg, neg) { document.querySelectorAll('.toast').forEach((t) => t.remove()); const t = h(`<div class="toast ${neg ? 'neg' : ''}" role="status">${esc(msg)}</div>`); app.append(t); setTimeout(() => t.remove(), 2600); }
  function notice(msg) { const n = document.getElementById('notice'); n.innerHTML = `<div class="notice" role="alert">${esc(msg)}<button data-act="dismiss">Dismiss</button></div>`; }
  function tour() {
    const steps = st.page === 'accounts' ? [['ahead', 'The account with the most room', 'One line, no paragraph. Range, export, Start in a folder and Add account on the right.'], ['find', 'Find', 'Ctrl K. On this page it filters the rows.']]
      : [['strip', 'The sentence', 'Running now, which account runs out first and when, which to use next with its Start button.'], ['board', 'The board', 'One lane per computer; every running session is a card. A pulse at its edge means it wrote in the last 30 s. Click a card to follow it.'], ['ladder', 'Room', 'Every account by what is left of its week, with the tightest window and the time to its reset. Amber is a window the pace runs out before the reset.'], ['reads', 'Readouts', 'Tokens, cost at list prices and cached share for the range, with the trend as dots.']];
    let i = 0; const show = () => { app.querySelectorAll('.pop').forEach((p) => p.remove()); if (i >= steps.length) return; const [key, t, p] = steps[i], el = app.querySelector(`[data-tour=${key}]`); if (!el) { i++; return show(); } el.scrollIntoView({ block: 'nearest' }); const r = el.getBoundingClientRect(), ar = app.getBoundingClientRect(), k = ar.width / app.offsetWidth; const pop = h(`<div class="pop" role="dialog"><h4>${t}</h4><p>${p}</p><div class="row2"><button class="btn sm primary" data-n>${i < steps.length - 1 ? 'Next' : 'Done'}</button><button class="btn sm ghost" data-x>Skip</button><span class="n">${i + 1} of ${steps.length}</span></div></div>`); pop.style.left = Math.max(8, Math.min((r.left - ar.left) / k, app.offsetWidth - 320)) + 'px'; pop.style.top = Math.min((r.bottom - ar.top) / k + 6, app.offsetHeight - 160) + 'px'; app.append(pop); pop.querySelector('[data-n]').addEventListener('click', () => { i++; show(); }); pop.querySelector('[data-x]').addEventListener('click', () => { i = steps.length; show(); }); pop.querySelector('[data-n]').focus(); };
    show();
  }
  function trayPopup() {
    const ov = document.getElementById('overlay'); if (ov.querySelector('.tray')) return ov.querySelector('.tray').remove();
    const u = useNext();
    const t = h(`<div class="tray" role="dialog" aria-label="Limits (tray)"><div class="ph">${LOGO.replace('<svg', '<svg width="16" height="20"')}<h2>Limits</h2><span class="sub">${ago(NOW - LIMITS_READ)}</span></div><ol>${sorted().map((a) => ladHTML(a, u && u.id === a.id)).join('')}</ol><div class="foot"><button class="btn sm" data-act="trayclose" title="Open the window">Open</button>${u ? `<button class="btn sm primary" data-act="start" data-id="${esc(u.id)}">${ic('play', 11)} Start ${esc(u.id)}</button>` : ''}</div></div>`);
    ov.append(t); sorted().forEach((a) => patchLad(t.querySelector(`[data-lad="${a.id}"]`), a)); drawRings(t);
  }
  function closeAll() { const ov = document.getElementById('overlay'); const had = ov.querySelector('.scrim,.drawer,.tray') || app.querySelector('.menu,.pop'); ov.querySelectorAll('.scrim,.drawer,.tray').forEach((x) => x.remove()); app.querySelectorAll('.pop').forEach((p) => p.remove()); closeMenus(); if (st.follow) { st.follow = null; document.querySelectorAll('.card').forEach((c) => c.setAttribute('aria-pressed', 'false')); } st.detail = null; const op = st.opener; st.opener = null; if (had && op && document.contains(op)) op.focus(); }

  // ---------- routing and events ----------
  function page() { tabs(); closeAll(); const m = document.getElementById('main'); m.scrollTop = 0; if (st.page === 'overview') overview(); else if (st.page === 'accounts') accountsPage(); else placeholder(st.page); }
  function go(p) { st.page = p; page(); }
  app.addEventListener('click', (e) => {
    const t = e.target.closest('[data-act],[data-page]'); if (!t) { if (!e.target.closest('.menu')) closeMenus(); return; }
    const act = t.dataset.act, id = t.dataset.id;
    if (!act && t.dataset.page) return go(t.dataset.page);
    switch (act) {
      case 'page': return go(t.dataset.page);
      case 'close': return closeAll();
      case 'dismiss': return (document.getElementById('notice').innerHTML = '');
      case 'toast': toast(t.dataset.msg); if (t.dataset.close) closeAll(); return;
      case 'start': return toast(`Terminal opened on ${id}`);
      case 'read': { const a = acct(id); if (a) { a.read = NOW; if (!a.windows && !a.needsLogin && !noQuota(a)) a.windows = [{ label: '5-hour', used: 12, reset: 3 * H + 40 * MIN }, { label: 'Week', used: 9, reset: 4 * DAY + 2 * H }]; } toast(a && a.needsLogin ? `${id}: the login has expired; sign in again` : noQuota(a) ? `${tool(a.tool).label} has no limits to read; usage is counted` : `Limits of ${id} read`, a && a.needsLogin); page(); return; }
      case 'add': return addDialog(!!t.dataset.first);
      case 'startin': return startIn();
      case 'refresh': { LIMITS_READ = NOW; USAGE_READ = NOW; const n = accounts().filter((a) => a.windows).length; accounts().forEach((a) => { if (a.read) a.read = NOW; }); fresh(); if (st.page === 'overview') { patchStrip(); patchLadder(); } toast(n ? `Read the limits of ${n} account${n === 1 ? '' : 's'} · usage up to date` : 'Nothing to read yet: no account is signed in'); return; }
      case 'live': if (st.page !== 'overview') go('overview'); document.getElementById('board')?.scrollIntoView({ block: 'start' }); return;
      case 'computers': return go('Settings › Computers');
      case 'tour': return tour();
      case 'trayclose': return closeAll();
      case 'gear': return menu(t, [{ id: 'settings', label: 'Settings', icon: 'gear', dot: full(), n: full() ? `${D.counts.newerHarness} available` : null, run: () => go('Settings') }, { id: 'cli', label: 'CLI reference', run: () => go('CLI reference') }, { id: 'pcs', label: 'Computers', n: pcs().length, run: () => go('Settings › Computers') }, 'hr', { id: 'motion', label: `Motion: ${st.still ? 'still' : 'live'}`, run: () => setStill(!st.still) }, { id: 'rings', label: `Rings show: ${st.used ? 'used' : 'left'}`, run: () => { st.used = !st.used; save(); page(); } }, { id: 'theme', label: `Theme: Harness ${st.theme}`, run: () => setTheme(st.theme === 'dark' ? 'light' : 'dark') }, 'hr', { ver: 'Harness Manager v' + D.counts.version }]);
      case 'setup': return menu(t, [{ label: 'MCP servers', n: D.counts.mcp, run: () => go('MCP servers') }, { label: 'Skills', n: D.counts.skills, run: () => go('Skills') }, { label: 'System prompts', run: () => go('System prompts') }, { label: 'API keys & routing', run: () => go('API keys & routing') }, { label: 'Apps', n: `${D.counts.apps} update`, warn: true, run: () => go('Apps') }]);
      case 'pins': e.stopPropagation(); return menu(t, D.projects.pinned.map(([p, n]) => ({ label: p, n: `${n} live`, run: () => go('Projects') })));
      case 'rowmenu': { const a = acct(id); return menu(t, [{ label: 'Open folder', icon: 'folder', run: () => toast('Folder opened') }, { label: 'Rename', run: () => toast('Rename: type the new name') }, { label: 'Profile settings', run: () => { detail(id); st.dtab = 'settings'; document.querySelector('[data-dtab=settings]')?.click(); } }, { label: 'Sync now', run: () => toast(`${id} synced from the main profile`) }, { label: 'Show window', icon: 'window', run: () => toast('Window brought to front') }, 'hr', a.main ? { label: 'Hide from the list', run: () => toast(`${id} hidden; a main profile cannot be removed`) } : { label: 'Remove from the list', run: () => toast(`${id} removed from the list`) }, a.main ? null : { label: 'Remove with its folder', run: () => toast(`${id} removed with its folder`) }].filter(Boolean)); }
    }
  });
  app.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') { if (document.activeElement && document.activeElement.id === 'q') { document.activeElement.blur(); } closeAll(); }
    if (e.key === 'Enter' && e.target.classList?.contains('row')) detail(e.target.dataset.row);
    if (e.key === 'Enter' && e.target.dataset?.act === 'pins') { e.preventDefault(); e.target.click(); }
  });
  document.addEventListener('keydown', (e) => { if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); document.getElementById('q')?.focus(); } if (e.key === 'Escape' && !app.contains(e.target)) closeAll(); });

  // ---------- the redraw: every 4 s the sessions write and the clock moves; nodes are patched, never rebuilt ----------
  function tick() {
    const dt = 4e3; NOW += dt;
    for (const s of D.sessions) { s.since += dt; const c = pc(s.computer); if (alive(c) && Math.random() < (s.wrote < 30e3 ? 0.8 : 0.08)) { s.wrote = Math.round(Math.random() * 3e3); s.tokens += Math.round(2e3 + Math.random() * 9e3); s.cost += 0.004 + Math.random() * 0.02; } else s.wrote += dt; }
    for (const a of D.accounts) for (const w of a.windows || []) { w.reset -= dt; if (w.out) w.out -= dt; }
    fresh(); if (st.page === 'overview' && full()) { patchStrip(); patchBoard(); patchLadder(); }
  }
  setInterval(tick, 4000);

  // ---------- prototype chrome ----------
  const frame = document.getElementById('frame');
  const setTheme = (t) => { st.theme = t; app.dataset.theme = t; save(); proto(); };
  const setStill = (s) => { st.still = s; app.classList.toggle('still', s || reduced()); save(); proto(); };
  const setW = (w) => { st.w = w; app.dataset.w = w; frame.style.width = w + 'px'; frame.style.height = Math.round(w * 9 / 16) + 'px'; fit(); save(); proto(); };
  const setData = (d) => { st.data = d; st.added = []; st.more = {}; save(); boot(); proto(); };
  const fit = () => { const k = Math.min(1, (window.innerWidth - 36) / st.w); frame.style.transform = `scale(${k})`; frame.parentElement.style.height = Math.round(st.w * 9 / 16 * k) + 'px'; };
  window.addEventListener('resize', fit);
  function proto() { document.querySelectorAll('.proto [data-set]').forEach((b) => { const [k, v] = b.dataset.set.split(':'); b.setAttribute('aria-pressed', String(st[k]) === v); }); }
  document.querySelectorAll('.proto [data-set]').forEach((b) => b.addEventListener('click', () => { const [k, v] = b.dataset.set.split(':'); if (k === 'theme') setTheme(v); else if (k === 'still') setStill(v === 'true'); else if (k === 'w') setW(+v); else if (k === 'data') setData(v); }));
  document.querySelector('.proto [data-tray]').addEventListener('click', trayPopup);
  document.querySelector('.proto [data-notice]').addEventListener('click', () => notice('codex-main: the limits could not be read (401 from the API); the last read is 14 m old.'));

  function boot() { shell(); st.page = st.page === 'overview' || st.page === 'accounts' ? st.page : 'overview'; page(); }
  app.dataset.theme = st.theme; app.classList.toggle('still', st.still || reduced()); setW(st.w); boot(); proto();
})();
