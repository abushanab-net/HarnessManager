// Option B "Workbench": the rail and the top bar (the same on every screen) and the small line icons.
// Load after kit.js. In a page: <div class="app b"><script>shell('now')</script> ... ; icons: <i data-ic="copy"></i>
const IC = {
  now: '<path d="M22 12h-4l-3 9L9 3l-3 9H2"/>',
  sessions: '<path d="m4 17 6-6-6-6"/><path d="M12 19h8"/>',
  projects: '<path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.7-.9l-.8-1.2A2 2 0 0 0 7.9 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>',
  accounts: '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
  computers: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>',
  setup: '<path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>',
  cli: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 10 3 2-3 2M13 15h4"/>',
  settings: '<path d="M4 6h10M18 6h2M4 12h2M10 12h10M4 18h8M16 18h4"/><circle cx="16" cy="6" r="2"/><circle cx="8" cy="12" r="2"/><circle cx="14" cy="18" r="2"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
  refresh: '<path d="M3 12a9 9 0 0 1 15.5-6.3L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-15.5 6.3L3 16"/><path d="M3 21v-5h5"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  copy: '<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/>',
  open: '<path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>',
  play: '<path d="M7 4v16l13-8z" fill="currentColor"/>',
  more: '<circle cx="5" cy="12" r="1.3" fill="currentColor"/><circle cx="12" cy="12" r="1.3" fill="currentColor"/><circle cx="19" cy="12" r="1.3" fill="currentColor"/>',
  tag: '<path d="M3 12V4a1 1 0 0 1 1-1h8l9 9-9 9z"/><circle cx="8" cy="8" r="1.2" fill="currentColor"/>',
  chev: '<path d="m9 6 6 6-6 6"/>',
  down: '<path d="m6 9 6 6 6-6"/>',
  ok: '<path d="m5 12 5 5 9-10"/>',
  fail: '<path d="M6 6l12 12M18 6 6 18"/>',
  spin: '<path d="M12 3a9 9 0 1 0 9 9"/>',
};
IC.folder = IC.projects;
const ic = (n, s = 16) => `<svg width="${s}" height="${s}" viewBox="0 0 24 24" aria-hidden="true">${IC[n] || ''}</svg>`;

function shell(active) {
  const nav = [
    ['now', 'Now', 'b-now.html'],
    ['sessions', 'Sessions', 'b-sessions.html', '<span class="bdg ok"><span class="dot live"></span>4 live</span>'],
    ['projects', 'Projects'],
    ['accounts', 'Accounts', 'b-accounts.html', '<span class="bdg warn">2 alerts</span>'],
    ['computers', 'Computers'],
    ['setup', 'Setup', '', '<span class="bdg">1 update</span>'],
  ];
  const foot = [['cli', 'CLI'], ['settings', 'Settings']];
  const item = ([id, label, href, badge]) => `<a${href ? ` href="${href}${location.search}"` : ''} class="${id === active ? 'on' : ''}"><span class="ic">${ic(id, 20)}</span>${label}${badge || ''}</a>`;
  document.write(`
<nav class="b-rail" aria-label="Main">
  <div class="logo"><i data-logo data-size="26"></i></div>
  <div class="b-nav">${nav.map(item).join('')}</div>
  <div class="b-nav foot">${foot.map(item).join('')}</div>
</nav>
<header class="b-top">
  <div class="row"><span class="app-name">Harness Manager</span><span class="row small muted" style="gap:6px"><span class="dot"></span>WORKSTATION</span></div>
  <div class="b-cmd"><span class="ic">${ic('search')}</span><span class="grow">Search or run a command</span><span class="b-kbd">Ctrl K</span></div>
  <div class="row end">
    <span class="b-chip ok"><span class="dot live"></span>4 live</span>
    <span class="b-chip"><span class="dots"><span class="dot"></span><span class="dot pc2"></span><span class="dot pc3"></span></span>3 computers</span>
    <span class="small muted">Limits read 1 m ago</span>
    <a class="btn icon sm" title="Refresh" style="width:28px"><span class="ic">${ic('refresh', 15)}</span></a>
  </div>
</header>`);
}
addEventListener('DOMContentLoaded', () => {
  for (const el of document.querySelectorAll('[data-ic]')) { el.classList.add('ic'); el.innerHTML = ic(el.dataset.ic, +(el.dataset.size || 16)); }
});
