// The pages review: the shell with the right place lit, one screen simulator per page, the theme for the whole page,
// and the form. Each page's content lives in content.js as P.pages[key] = (W, H) => html.
(function () {
  const { NOW, MEASURED, computers, accounts, running, kpi, money, icon, h, reduced } = HM;
  const NAV = [['home', 'Overview', 'dash', money(kpi.cost.v)], ['sessions', 'Sessions', 'term', String(running.length)], ['projects', 'Projects', 'folder', '23'], ['accounts', 'Accounts', 'users', String(accounts.length)]];
  const NAV2 = [['mcp', 'MCP servers', '14'], ['skills', 'Skills', '61'], ['prompts', 'System prompts', ''], ['routing', 'API keys & routing', ''], ['apps', 'Apps', '1']];
  const on = (k, cur) => (k === cur ? ' on' : '');
  function shell(main, cur, find = 'Find an account, session or setting') {
    return `<aside class="side"><div class="brand"><span class="mark hm">${LOGOS.hm}</span><div><b>Harness Manager</b><small>Studio · 3 computers</small></div></div>
      <nav class="nav">${NAV.map(([k, t, i, c]) => `<a class="${on(k, cur).trim()}" href="#" onclick="return false">${icon(i)}<span>${t}</span><em>${c}</em></a>`).join('')}
        <div class="gap"></div><a href="#" onclick="return false">${icon('grid')}<span>Setup</span><em class="upd">1</em></a>
        ${NAV2.map(([k, t, c]) => `<a class="sub${on(k, cur)}" href="#" onclick="return false"><span>${t}</span><em>${c}</em></a>`).join('')}</nav>
      <nav class="nav bottom"><a class="${on('cli', cur).trim()}" href="#" onclick="return false">${icon('book')}<span>CLI reference</span></a><a class="${on('settings', cur).trim()}" href="#" onclick="return false">${icon('gear')}<span>Settings</span>${cur === 'settings' ? '' : '<i class="udot" title="A newer Harness Manager is available"></i>'}</a><span class="ver">1.6.2</span></nav></aside>
    <header class="top"><label class="find">${icon('search', 15)}<input placeholder="${find}" aria-label="Find"><kbd>Ctrl K</kbd></label>
      <div class="right"><a class="livestrip" href="#" onclick="return false"><i></i>${running.length} running</a><span class="pcs">${computers.map((c) => `<i style="background:${c.col}" title="${c.name}"></i>`).join('')}<span>3 computers</span></span><span class="fresh">limits read ${Math.round((NOW - MEASURED) / 1e3)} s ago</span><button class="ib" aria-label="Tour of this page">?</button><button class="ib" aria-label="Refresh">${icon('refresh', 15)}</button></div></header>
    <main class="sheet">${main}</main>`;
  }

  const SCREENS = [['1920x1080 at 100%', 1920, 1080], ['1920x1080 at 125%', 1536, 864], ['1920x1080 at 150%', 1280, 720], ['2560x1440 at 150%', 1707, 960], ['3840x2160 at 200%', 1920, 1080], ['1366x768 at 100%', 1366, 768]];
  const theme = { brand: 'hm', mode: 'light' };
  const pages = {};
  function mount(sec, key) {
    const page = pages[key], st = { screen: 1, motion: !reduced() };
    const stage = sec.querySelector('.stage'), screen = h('<div class="screen"></div>'), dims = h('<span class="dims"></span>');
    stage.append(screen, dims);
    const fit = () => { screen.style.transform = `scale(${stage.clientWidth / SCREENS[st.screen][1]})`; };
    const draw = () => {
      const [name, W, Hh] = SCREENS[st.screen];
      screen.style.width = W + 'px'; screen.style.height = Hh + 'px'; stage.style.aspectRatio = W + ' / ' + Hh;
      dims.textContent = `${name} → laid out at ${W} × ${Hh}`;
      screen.innerHTML = '';
      const app = h(`<div class="app" data-brand="${theme.brand}" data-theme="${theme.mode}" data-page="${key}"></div>`);
      screen.append(app); fit();
      app.innerHTML = shell(page.html(W, Hh, st), page.nav || key, page.find);
      if (page.after) page.after(app, st);
    };
    new ResizeObserver(fit).observe(stage);
    const sel = sec.querySelector('select.scr');
    sel.innerHTML = SCREENS.map(([n], i) => `<option value="${i}">${n}</option>`).join(''); sel.value = st.screen;
    sel.addEventListener('change', () => { st.screen = +sel.value; draw(); });
    sec.querySelector('[data-full]').addEventListener('click', () => stage.requestFullscreen && stage.requestFullscreen());
    document.addEventListener('fullscreenchange', fit);
    sec.redraw = draw;
    draw();
  }
  function themePicker() {
    document.querySelectorAll('[data-brand-btn]').forEach((b) => b.addEventListener('click', () => {
      theme.brand = b.dataset.brandBtn; theme.mode = b.dataset.modeBtn;
      document.querySelectorAll('[data-brand-btn]').forEach((x) => x.classList.toggle('on', x === b));
      document.querySelectorAll('.app').forEach((a) => { a.dataset.brand = theme.brand; a.dataset.theme = theme.mode; });
    }));
  }

  // the form: a verdict and a comment per page, then the decision
  function feedback(names, title, file) {
    const KEY = document.body.dataset.fbkey; let draft = {};
    try { draft = JSON.parse(localStorage.getItem(KEY) || '{}'); } catch (e) {}
    const saved = document.getElementById('saved'), save = () => { localStorage.setItem(KEY, JSON.stringify(draft)); saved.textContent = 'Saved in this browser ' + new Date().toLocaleTimeString(); };
    document.querySelectorAll('[data-k]').forEach((x) => { const k = x.dataset.k; if (x.type === 'radio') { x.checked = draft[k] === x.value; x.addEventListener('change', () => { draft[k] = x.value; save(); }); } else { x.value = draft[k] || ''; x.addEventListener('input', () => { draft[k] = x.value; save(); }); } });
    const text = () => { const out = [`# ${title}: my answers`, '']; names.forEach(([k, n]) => { out.push(`## ${n}`, `Verdict: ${draft['v' + k] || '(none)'}`); if (draft['c' + k]) out.push('Comments: ' + draft['c' + k]); out.push(''); }); out.push('## Decision', draft.pick || '(none)'); if (draft.general) out.push('Anything else: ' + draft.general); return out.join('\n'); };
    const box = document.getElementById('out');
    document.getElementById('copy').addEventListener('click', async () => { box.style.display = 'block'; box.value = text(); box.select(); try { await navigator.clipboard.writeText(box.value); saved.textContent = 'Copied. Paste it into the chat with Claude.'; } catch (e) { document.execCommand('copy'); saved.textContent = 'Copied (or select the text below and copy it).'; } });
    document.getElementById('dl').addEventListener('click', () => { const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([text()], { type: 'text/markdown' })); a.download = file; a.click(); });
    document.getElementById('clear').addEventListener('click', () => { if (!window.confirm('Clear every answer on this page?')) return; draft = {}; save(); location.reload(); });
  }
  window.P = { shell, pages, mount, themePicker, feedback, SCREENS };
})();
