// Option A "Tighten": the shell every A screen shares (sidebar + top bar), written once. <body data-page="overview|accounts|computers|settings">.
// Include as the first child of .app, before kit.js paints the logo.
{
  const ico = (d) => `<svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;
  const I = {
    overview: ico('<rect x="2" y="2" width="5" height="5" rx="1"/><rect x="9" y="2" width="5" height="5" rx="1"/><rect x="2" y="9" width="5" height="5" rx="1"/><rect x="9" y="9" width="5" height="5" rx="1"/>'),
    sessions: ico('<path d="M3 4l4 4-4 4M9 12h4"/>'),
    projects: ico('<path d="M2 4.5a1 1 0 011-1h3l1.5 1.5H13a1 1 0 011 1V12a1 1 0 01-1 1H3a1 1 0 01-1-1z"/>'),
    accounts: ico('<circle cx="8" cy="5.5" r="2.5"/><path d="M3 13.5c.6-2.4 2.6-3.5 5-3.5s4.4 1.1 5 3.5"/>'),
    computers: ico('<rect x="2" y="3" width="12" height="8" rx="1"/><path d="M6 13.5h4M8 11v2.5"/>'),
    setup: ico('<path d="M8 2l6 3-6 3-6-3zM2 8l6 3 6-3M2 11l6 3 6-3"/>'),
    cli: ico('<rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="M5 7l2 1.5L5 10M8.5 10.5H11"/>'),
    settings: ico('<path d="M2 5h7M12 5h2M2 11h2M7 11h7"/><circle cx="10.5" cy="5" r="1.5"/><circle cx="5.5" cy="11" r="1.5"/>'),
    search: ico('<circle cx="7" cy="7" r="4"/><path d="M10 10l3.5 3.5"/>'),
    refresh: ico('<path d="M13 8a5 5 0 11-1.5-3.5M13 3v2.5h-2.5"/>'),
    alert: '<svg width="11" height="11" viewBox="0 0 12 12" aria-hidden="true"><path d="M6 1l5.2 9.5H.8z" fill="currentColor"/></svg>',
  };
  const page = document.body.dataset.page;
  const item = (id, label, badge = '') => `<a class="${id === page ? 'on' : ''}"${id === page ? ' aria-current="page"' : ''}>${I[id]}<span>${label}</span>${badge}</a>`;
  document.currentScript.insertAdjacentHTML('afterend', `
<aside class="side">
  <div class="brand"><i data-logo data-size="26"></i><div><b>Harness Manager</b><span>WORKSTATION</span></div></div>
  <nav class="nav">
    ${item('overview', 'Overview')}
    ${item('sessions', 'Sessions', '<span class="pill ok badge"><span class="dot live"></span>4 live</span>')}
    ${item('projects', 'Projects')}
    ${item('accounts', 'Accounts', `<span class="pill warn badge" title="2 accounts need attention">${I.alert}2</span>`)}
    ${item('computers', 'Computers')}
    ${item('setup', 'Setup', '<span class="pill badge">1 update</span>')}
    <div class="sub"><a>MCP servers</a><a>Skills</a><a>System prompts</a><a>API keys &amp; routing</a><a>Apps</a></div>
  </nav>
  <nav class="nav foot">
    ${item('cli', 'CLI reference')}
    ${item('settings', 'Settings')}
    <div class="ver mono">v1.9.14</div>
  </nav>
</aside>
<header class="top">
  <div class="cmd">${I.search}<span class="grow trunc">Search or run a command</span><span class="kbd">Ctrl K</span></div>
  <span class="grow"></span>
  <a class="chip"><span class="dot live"></span>4 live</a>
  <a class="chip"><span class="dot"></span><span class="dot pc2"></span><span class="dot pc3"></span>3 computers</a>
  <span class="small muted">limits read 1 m ago</span>
  <a class="btn icon" title="Refresh limits" aria-label="Refresh limits">${I.refresh}</a>
</header>`);

  // 7-day sparkline: <i data-spark="3,5,4,8,6,7,5"></i>, each scaled to its own peak (a shape, not a comparison)
  addEventListener('DOMContentLoaded', () => {
    for (const el of document.querySelectorAll('[data-spark]')) {
      const v = el.dataset.spark.split(',').map(Number), m = Math.max(...v, 1), w = 96, h = 20;
      const pts = v.map((x, i) => `${(i * w / (v.length - 1)).toFixed(1)},${(h - 2 - x / m * (h - 4)).toFixed(1)}`).join(' ');
      el.innerHTML = `<svg width="${w}" height="${h}" role="img" aria-label="tokens per day, last 7 days"><polyline points="${pts}" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round"/></svg>`;
    }
  });
}
