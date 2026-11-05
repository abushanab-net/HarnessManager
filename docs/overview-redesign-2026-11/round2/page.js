// Round two: the shell with the real marks, the scene every map shares (finding, readouts, attention), the graph,
// label placement with collision nudging, beads along routes, selection, the screen simulator and the feedback form.
(function () {
  const { NOW, MEASURED, DAY, H, computers, accounts, tools, providers, router, edges, running, kpi, series, fmt, money, dur, at, clock, esc, pct, pcName, pcCol, burn, icon, reduced } = HM;
  const SVGNS = 'http://www.w3.org/2000/svg';
  const el = (tag, attrs = {}, parent) => { const e = document.createElementNS(SVGNS, tag); for (const k in attrs) e.setAttribute(k, attrs[k]); if (parent) parent.append(e); return e; };
  const when = (ms) => at(ms);

  // ---------- the shell ----------
  const NAV = [['Overview', 'dash', money(kpi.cost.v)], ['Sessions', 'term', String(running.length)], ['Projects', 'folder', '23'], ['Accounts', 'users', String(accounts.length)]];
  const NAV2 = [['MCP servers', '14'], ['Skills', '61'], ['System prompts', ''], ['API keys & routing', ''], ['Apps', '1']];
  function shell(main) {
    return `<aside class="side"><div class="brand"><span class="mark hm">${LOGOS.hm}</span><span class="mark vgc">${LOGOS.vgColor}</span><span class="mark vgg">${LOGOS.vgGold}</span><div><b>Harness Manager</b><small>Studio · 3 computers</small></div></div>
      <nav class="nav">${NAV.map(([t, i, c], n) => `<a class="${n ? '' : 'on'}" href="#" onclick="return false">${icon(i)}<span>${t}</span><em>${c}</em></a>`).join('')}
        <div class="gap"></div><a href="#" onclick="return false">${icon('grid')}<span>Setup</span><em class="upd">1</em></a>
        ${NAV2.map(([t, c]) => `<a class="sub" href="#" onclick="return false"><span>${t}</span><em>${c}</em></a>`).join('')}</nav>
      <nav class="nav bottom"><a href="#" onclick="return false">${icon('book')}<span>CLI reference</span></a><a href="#" onclick="return false">${icon('gear')}<span>Settings</span></a><span class="ver">1.6.1</span></nav></aside>
    <header class="top"><label class="find">${icon('search', 15)}<input placeholder="Find an account, session or setting" aria-label="Find"><kbd>Ctrl K</kbd></label>
      <div class="right"><span class="pcs">${computers.map((c) => `<i style="background:${c.col}" title="${c.name}"></i>`).join('')}<span>3 computers</span></span><span class="fresh">limits read ${Math.round((NOW - MEASURED) / 1e3)} s ago</span><button class="ib" aria-label="Refresh">${icon('refresh', 15)}</button></div></header>
    <main class="sheet">${main}</main>`;
  }

  // ---------- the graph: every hop a token takes, nothing invented ----------
  const TOOL_ORDER = ['claude', 'codex', 'opencode', 'pi', 'agy'];
  function graph() {
    const N = [], E = [];
    const tight = (a) => (a.windows ? a.windows.slice().sort((x, y) => x.used - y.used).pop() : null);
    for (const c of computers) N.push({ id: c.id, kind: 'pc', col: c.col, label: c.name, sub: c.me ? 'this computer' : '', on: accounts.filter((a) => a.on.includes(c.id)).map((a) => a.id) });
    const acc = accounts.slice().sort((a, b) => TOOL_ORDER.indexOf(a.tool) - TOOL_ORDER.indexOf(b.tool) || a.label.localeCompare(b.label));
    for (const a of acc) {
      const w = tight(a), wk = a.windows && a.windows[1], fh = a.windows && a.windows[0];
      const sub = a.needsLogin ? 'login expired' : a.cap ? `$${a.cap.spent.toFixed(2)} of $${a.cap.cap} today` : a.windows ? `5-h ${fh.used}% · week ${wk.used}%${wk.out ? ', ≈ out ' + when(wk.out) : ''}` : 'API key, no limit';
      N.push({ id: a.id, kind: 'account', label: a.label, sub, tone: a.needsLogin ? 'neg' : w && w.out ? 'warn' : '', k: a.needsLogin ? 'r' : a.kind === 'routed' ? 'a' : 'c', rate: a.tps, on: a.on, acct: a, tool: a.tool });
      for (const pc of a.on) E.push({ a: pc, b: a.id, k: 'pc:' + pc, rate: a.tps / a.on.length });
    }
    for (const t of tools) N.push({ id: t.id, kind: 'tool', label: t.label, sub: `${t.ver}${running.filter((r) => r.tool === t.label).length ? ' · ' + running.filter((r) => r.tool === t.label).length + ' open' : ''}`, rate: edges.filter((e) => e[1] === t.id).reduce((s, e) => s + e[3], 0) });
    N.push({ id: 'router', kind: 'router', label: 'Local router', sub: `:${router.port} · ${router.routes} routes · ${router.errors} errors`, k: 'a', rate: edges.filter((e) => e[1] === 'router').reduce((s, e) => s + e[3], 0) });
    for (const p of providers) N.push({ id: p.id, kind: 'provider', label: p.label, sub: p.paused ? `paused, retry ${dur(p.retry)}` : p.tps ? `${p.first} s first reply` : 'no traffic', tone: p.paused ? 'neg' : p.tps ? 'ok' : '', k: p.paused ? 'r' : p.via === 'router' ? 'a' : 'c', rate: p.tps, via: p.via });
    for (const [a, b, k, rate] of edges) E.push({ a, b, k, rate });
    return { N, E, by: Object.fromEntries(N.map((n) => [n.id, n])) };
  }
  // what a body touches: everything upstream and downstream of it
  function chain(E, id) {
    const lit = new Set([id]);
    const walk = (dir) => { const q = [id]; while (q.length) { const x = q.shift(); for (const e of E) { const [a, b] = dir ? [e.a, e.b] : [e.b, e.a]; if (a === x && !lit.has(b)) { lit.add(b); q.push(b); } } } };
    walk(true); walk(false);
    return lit;
  }
  const edgeCol = (k) => (k.startsWith('pc:') ? pcCol(k.slice(3)) : k === 'a' ? 'var(--r2)' : k === 'r' ? 'var(--neg)' : 'var(--ac)');
  const nodeCol = (n) => (n.kind === 'pc' ? n.col : n.kind === 'tool' ? 'var(--m)' : n.kind === 'router' ? 'var(--r2)' : n.k === 'r' ? 'var(--neg)' : n.kind === 'provider' && !n.rate ? 'var(--m)' : edgeCol(n.k || 'c'));

  // ---------- drawing ----------
  const arcPath = (cx, cy, r, f0, f1) => { if (f1 - f0 >= 0.999) f1 = f0 + 0.999; const a0 = f0 * 2 * Math.PI - Math.PI / 2, a1 = f1 * 2 * Math.PI - Math.PI / 2; return `M${cx + r * Math.cos(a0)} ${cy + r * Math.sin(a0)} A${r} ${r} 0 ${f1 - f0 > 0.5 ? 1 : 0} 1 ${cx + r * Math.cos(a1)} ${cy + r * Math.sin(a1)}`; };
  /** Two named limit arcs around an account: inner 5-hour, outer weekly; solid is used, faint is the estimate at the reset. */
  function limitArcs(g, x, y, a, R = 13) {
    const tone = (w) => (w.out ? 'var(--warn)' : a.kind === 'routed' ? 'var(--r2)' : 'var(--ac)');
    if (a.needsLogin) { el('circle', { cx: x, cy: y, r: R, fill: 'none', stroke: 'var(--neg)', 'stroke-dasharray': '2 4', opacity: .8 }, g); return; }
    if (a.cap) { const u = a.cap.spent / a.cap.cap; el('circle', { cx: x, cy: y, r: R, fill: 'none', stroke: 'var(--line)', 'stroke-width': 2.4 }, g); el('path', { d: arcPath(x, y, R, 0, u), fill: 'none', stroke: 'var(--r2)', 'stroke-width': 2.4, 'stroke-linecap': 'round' }, g); return; }
    if (!a.windows) { el('circle', { cx: x, cy: y, r: R, fill: 'none', stroke: 'var(--line)', 'stroke-dasharray': '1 3' }, g); return; }
    a.windows.forEach((w, i) => {
      const r = i ? R : R - 4, u = w.used / 100, f = Math.min(1, (w.at || w.used) / 100);
      el('circle', { cx: x, cy: y, r, fill: 'none', stroke: 'var(--line)', 'stroke-width': 2.2 }, g);
      el('path', { d: arcPath(x, y, r, u, f), fill: 'none', stroke: tone(w), 'stroke-width': 2.2, 'stroke-dasharray': '1.5 2.5', opacity: .6 }, g);
      el('path', { d: arcPath(x, y, r, 0, u), fill: 'none', stroke: tone(w), 'stroke-width': 2.2, 'stroke-linecap': 'round' }, g);
    });
  }
  /** A body on the map: a dot sized by kind, limit arcs for an account, a ring for a tool, a square for a computer. */
  function body(svg, n, x, y) {
    const g = el('g', { class: 'n', 'data-node': n.id, tabindex: 0, role: 'button', 'aria-label': `${n.label}, ${n.sub}` }, svg);
    n.x = x; n.y = y;
    const col = nodeCol(n);
    if (n.kind === 'pc') { el('rect', { x: x - 6, y: y - 6, width: 12, height: 12, rx: 3, fill: col }, g); if (n.id === 'studio') el('rect', { x: x - 10, y: y - 10, width: 20, height: 20, rx: 5, fill: 'none', stroke: col, opacity: .5 }, g); }
    else if (n.kind === 'tool') { el('circle', { cx: x, cy: y, r: 5.5, fill: 'var(--s)', stroke: 'var(--t2)', 'stroke-width': 1.5 }, g); }
    else if (n.kind === 'router') { el('rect', { x: x - 9, y: y - 9, width: 18, height: 18, rx: 4, fill: 'var(--s)', stroke: 'var(--r2)', 'stroke-width': 1.6 }, g); el('circle', { cx: x, cy: y, r: 3, fill: 'var(--r2)' }, g); }
    else if (n.kind === 'account') { el('circle', { cx: x, cy: y, r: 16, fill: 'var(--s)', opacity: .9 }, g); limitArcs(g, x, y, n.acct); el('circle', { cx: x, cy: y, r: 5, fill: col }, g); }
    else { const r = n.rate ? 7 : 5; el('circle', { cx: x, cy: y, r: r + 5, fill: col, opacity: n.rate ? .18 : 0 }, g); el('circle', { cx: x, cy: y, r, fill: n.rate || n.k === 'r' ? col : 'var(--s)', stroke: col, 'stroke-width': 1.5, 'stroke-dasharray': n.k === 'r' ? '2 2' : '' }, g); }
    g.hit = el('circle', { cx: x, cy: y, r: 18, fill: 'transparent' }, g);
    return g;
  }
  /** A route line; broken ones are dashed and still. */
  const route = (svg, e, d) => el('path', { class: 'e', d, fill: 'none', stroke: edgeCol(e.k), 'stroke-width': e.k === 'r' ? 1.2 : e.rate ? 1.6 : 1, 'stroke-dasharray': e.k === 'r' ? '3 4' : '', opacity: e.rate || e.k === 'r' ? .55 : .25, 'data-a': e.a, 'data-b': e.b }, svg);

  /** HTML labels over the map, measured and pushed apart until none overlap. side: l (text ends at x), r (starts at x), c (centred). */
  function labelLayer(host) {
    const L = [];
    return {
      add(n, x, y, side, cls = '', html) {
        const pc = n && n.on && n.kind === 'account' ? `<span class="pc">${n.on.map((id) => `<i style="background:${pcCol(id)}"></i>`).join('')}</span>` : '';
        const d = HM.h(`<div class="lb ${cls}" data-for="${n ? n.id : ''}">${html || `<b>${esc(n.label)}${pc}</b><span class="${n.tone || ''}">${esc(n.sub)}</span>`}</div>`);
        host.append(d); L.push({ d, x, y, side, n }); return d;
      },
      place(gap = 3) {
        for (const l of L) { l.w = l.d.offsetWidth; l.h = l.d.offsetHeight; l.left = l.side === 'r' ? l.x : l.side === 'l' ? l.x - l.w : l.x - l.w / 2; l.top = l.y - l.h / 2; l.top0 = l.top; }
        const s = L.slice().sort((a, b) => a.top - b.top);
        for (let pass = 0; pass < 3; pass++) for (let i = 0; i < s.length; i++) for (let j = 0; j < i; j++) { const a = s[j], b = s[i]; if (b.left < a.left + a.w + 6 && a.left < b.left + b.w + 6 && b.top < a.top + a.h + gap) b.top = a.top + a.h + gap; }
        for (const l of L) { l.d.style.transform = `translate(${l.left.toFixed(1)}px, ${l.top.toFixed(1)}px)`; l.d.style.textAlign = l.side === 'l' ? 'right' : l.side === 'c' ? 'center' : 'left'; }
        return L;
      },
      list: L,
    };
  }
  /** Beads along routes, as many and as fast as the traffic on them. */
  function beads(svg, items, motion) {
    const g = el('g', { class: 'beads' }, svg), B = [];
    for (const it of items) {
      if (!it.rate) continue;
      const len = it.path.getTotalLength(), n = Math.max(1, Math.round(Math.log10(it.rate + 1) * 1.3)), v = 26 + Math.log10(it.rate + 1) * 18;
      for (let i = 0; i < n; i++) B.push({ c: el('circle', { r: 2.6, fill: it.color || edgeCol(it.e.k) }, g), path: it.path, len, o: i / n, v, e: it.e });
    }
    const place = (t) => { for (const b of B) { const u = ((t * b.v) / b.len + b.o) % 1, p = b.path.getPointAtLength(u * b.len); b.c.setAttribute('cx', p.x.toFixed(1)); b.c.setAttribute('cy', p.y.toFixed(1)); } };
    let raf = 0, vis = true;
    const io = new IntersectionObserver((en) => { vis = en[0].isIntersecting; }, { threshold: 0.05 }); io.observe(svg);
    if (motion) { const t0 = performance.now(); const loop = (n) => { if (vis) place((n - t0) / 1000); raf = requestAnimationFrame(loop); }; raf = requestAnimationFrame(loop); } else place(3.1);
    return { g, stop: () => { cancelAnimationFrame(raf); io.disconnect(); }, filter(f) { g.style.opacity = f ? 1 : .55; for (const b of B) b.c.style.display = !f || f(b.e) ? '' : 'none'; } };
  }
  /** Hover, focus, click to keep, Escape to clear; apply(litSet|null) redraws. */
  function selectable(map, E, apply, defaultId = null) {
    let sticky = defaultId, hover = null;
    const upd = () => apply(hover ? chain(E, hover) : sticky ? chain(E, sticky) : null, hover || sticky);
    map.querySelectorAll('[data-node]').forEach((g) => {
      const id = g.dataset.node;
      g.addEventListener('mouseenter', () => { hover = id; upd(); }); g.addEventListener('mouseleave', () => { hover = null; upd(); });
      g.addEventListener('focus', () => { hover = id; upd(); }); g.addEventListener('blur', () => { hover = null; upd(); });
      g.addEventListener('click', (ev) => { sticky = sticky === id ? defaultId : id; ev.stopPropagation(); upd(); });
      g.addEventListener('keydown', (ev) => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); sticky = sticky === id ? defaultId : id; upd(); } });
    });
    map.addEventListener('click', () => { sticky = defaultId; upd(); });
    map.addEventListener('keydown', (ev) => { if (ev.key === 'Escape') { sticky = defaultId; hover = null; upd(); } });
    upd();
  }
  /** The standard dimming: bodies, labels and routes outside the lit set fade. */
  const dimmer = (map, bd) => (lit) => {
    map.querySelectorAll('[data-node]').forEach((g) => g.classList.toggle('dim', !!lit && !lit.has(g.dataset.node)));
    map.querySelectorAll('.lb[data-for]').forEach((d) => d.classList.toggle('dim', !!lit && !lit.has(d.dataset.for)));
    map.querySelectorAll('path.e').forEach((p) => p.classList.toggle('dim', !!lit && !(lit.has(p.dataset.a) && lit.has(p.dataset.b))));
    if (bd) bd.filter(lit ? (e) => lit.has(e.a) && lit.has(e.b) : null);
  };
  // the circular mean of some angles
  // the circular mean; when the angles cancel (two opposite accounts) the first one stands in
  const meanAng = (as) => { const y = as.reduce((s, a) => s + Math.sin(a), 0), x = as.reduce((s, a) => s + Math.cos(a), 0); return Math.hypot(x, y) < 1e-6 ? (as[0] || 0) : Math.atan2(y, x); };
  // push angles apart until each pair is at least `min` apart (on a circle when wrap is true)
  function spread(items, min, wrap) {
    for (let pass = 0; pass < 12; pass++) {
      items.sort((a, b) => a.ang - b.ang);
      for (let i = 1; i < items.length; i++) { const d = items[i].ang - items[i - 1].ang; if (d < min) { items[i].ang += (min - d) / 2; items[i - 1].ang -= (min - d) / 2; } }
      if (wrap && items.length > 1) { const d = items[0].ang + 2 * Math.PI - items[items.length - 1].ang; if (d < min) { items[0].ang += (min - d) / 2; items[items.length - 1].ang -= (min - d) / 2; } }
    }
  }
  /** A corridor between two rings: radial to a corridor radius, an arc along it, radial in. */
  function corridor(cx, cy, rA, tA, rB, tB, rc) {
    const p = (r, t) => `${(cx + r * Math.cos(t)).toFixed(1)} ${(cy + r * Math.sin(t)).toFixed(1)}`;
    let d = tB - tA; while (d > Math.PI) d -= 2 * Math.PI; while (d < -Math.PI) d += 2 * Math.PI;
    if (Math.abs(d) < 0.02) return `M${p(rA, tA)} L${p(rB, tB)}`;
    return `M${p(rA, tA)} L${p(rc, tA)} A${rc} ${rc} 0 ${Math.abs(d) > Math.PI ? 1 : 0} ${d > 0 ? 1 : 0} ${p(rc, tB)} L${p(rB, tB)}`;
  }

  // ---------- the scene every map shares ----------
  function dots(vals, color, cols = 12, rows = 5) {
    const max = Math.max(...vals), w = cols * 6, hh = rows * 6;
    let s = `<svg width="${w}" height="${hh}" viewBox="0 0 ${w} ${hh}" aria-hidden="true">`;
    vals.slice(-cols).forEach((v, c) => { const lit = Math.max(1, Math.round((v / max) * rows)); for (let r = 0; r < rows; r++) s += `<circle cx="${c * 6 + 3}" cy="${hh - r * 6 - 3}" r="1.7" fill="${r < lit ? color : 'var(--line)'}"/>`; });
    return s + '</svg>';
  }
  function scene(map) {
    const cw = burn().find((b) => b.out);
    return `<section class="scene ${map.id}" aria-label="Live routes"><div class="map" data-map="${map.id}"></div>
      <div class="ov l">
        <div><div class="crumb"><i class="dot"></i>Overview · live on 3 computers · ${running.length} sessions open</div>
          <h1><span class="w">${esc(cw.name)}</span> will likely run out of its weekly limit on ${when(cw.out).replace(/ /, ' at about ')}, ${dur(cw.reset - cw.out)} before it resets.</h1>
          <div class="basis">Measured <b>${cw.used}% used</b>, resets <b>${when(cw.reset)}</b>, read ${Math.round((NOW - MEASURED) / 1e3)} s ago. The run-out is an estimate from today's pace.</div></div>
        <div class="rds">
        <div class="rd"><label>Tokens<b class="d">${pct(kpi.tokens.v, kpi.tokens.prev)}</b></label><strong>${fmt(kpi.tokens.v)}</strong>${dots(series.tokens, 'var(--ac)')}</div>
        <div class="rd"><label>Cost<b class="d warn">+${money(kpi.cost.v - kpi.cost.prev, 0)}</b></label><strong>${money(kpi.cost.v, 0)}</strong>${dots(series.cost, 'var(--warn)')}</div>
        <div class="rd"><label>Cached<b class="d">+${Math.round((kpi.hit.v - kpi.hit.prev) * 100)} pt</b></label><strong>${Math.round(kpi.hit.v * 100)}<small>%</small></strong>${dots(series.hit.map((x) => x - 0.8), 'var(--ac)')}</div>
        </div>
      </div>
      <div class="ov r">
        <div class="inc hd">Needs attention<span>3 open</span></div>
        <div class="inc"><div class="t"><i></i>${esc(cw.name)} weekly<em>≈ ${when(cw.out)}</em></div><p>Runs out ${dur(cw.reset - cw.out)} before its reset at today's pace. Move work to Claude Team.</p></div>
        <div class="inc neg"><div class="t"><i></i>Antigravity<em>login</em></div><p>The login expired, so its limits cannot be read.</p><button>Sign in</button></div>
        <div class="inc neg"><div class="t"><i></i>Groq<em>retry ${dur(4 * 60e3)}</em></div><p>Paused by the router after 3 errors in a row.</p></div>
      </div>
      <div class="ov b">${map.legend}${computers.map((c) => `<span><i class="sq" style="background:${c.col}"></i>${c.name}</span>`).join('')}</div>
      <div class="ov h">${map.hint}</div></section>`;
  }
  const LEGEND = `<span><i style="background:var(--ac)"></i>signed-in plan</span><span><i style="background:var(--r2)"></i>through the router</span><span><i style="background:var(--neg)"></i>paused or logged out</span>`;

  // ---------- the simulator: one theme for the whole page, a screen per stage ----------
  const SCREENS = [['1920x1080 at 100%', 1920, 1080], ['1920x1080 at 125%', 1536, 864], ['1920x1080 at 150%', 1280, 720], ['2560x1440 at 150%', 1707, 960], ['3840x2160 at 200%', 1920, 1080], ['1366x768 at 100%', 1366, 768]];
  const maps = {};
  const theme = { brand: 'hm', mode: 'light' };
  function mount(sec, key) {
    const map = maps[key], st = { screen: 1, motion: !reduced() };
    const stage = sec.querySelector('.stage'), screen = HM.h('<div class="screen"></div>'), dims = HM.h('<span class="dims"></span>');
    stage.append(screen, dims);
    let inst = null;
    const fit = () => { const W = SCREENS[st.screen][1]; screen.style.transform = `scale(${stage.clientWidth / W})`; };
    const draw = () => {
      if (inst && inst.stop) inst.stop();
      const [name, W, Hh] = SCREENS[st.screen];
      screen.style.width = W + 'px'; screen.style.height = Hh + 'px'; stage.style.aspectRatio = W + ' / ' + Hh;
      dims.textContent = `${name} → laid out at ${W} × ${Hh}`;
      screen.innerHTML = '';
      const app = HM.h(`<div class="app" data-brand="${theme.brand}" data-theme="${theme.mode}"></div>`);
      screen.append(app); fit();
      app.innerHTML = shell((H2.scene || scene)(map));
      inst = map.draw(app.querySelector('.map'), { motion: st.motion });
    };
    new ResizeObserver(fit).observe(stage);
    sec.querySelectorAll('[data-motion-btn]').forEach((b) => b.addEventListener('click', () => { st.motion = b.dataset.motionBtn === 'on'; sec.querySelectorAll('[data-motion-btn]').forEach((x) => x.classList.toggle('on', x === b)); draw(); }));
    const sel = sec.querySelector('select.scr');
    sel.innerHTML = SCREENS.map(([n], i) => `<option value="${i}">${n}</option>`).join(''); sel.value = st.screen;
    sel.addEventListener('change', () => { st.screen = +sel.value; draw(); });
    sec.querySelector('[data-full]').addEventListener('click', () => stage.requestFullscreen && stage.requestFullscreen());
    document.addEventListener('fullscreenchange', fit);
    if (!st.motion) sec.querySelectorAll('[data-motion-btn]').forEach((x) => x.classList.toggle('on', x.dataset.motionBtn === 'off'));
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

  // ---------- feedback, kept in this browser ----------
  const KEY = document.body.dataset.fbkey || 'hm-overview-redesign-2026-11-round2';
  function feedback(sections) {
    let draft = {};
    try { draft = JSON.parse(localStorage.getItem(KEY) || '{}'); } catch (e) { draft = {}; }
    const saved = document.getElementById('saved');
    const save = () => { try { localStorage.setItem(KEY, JSON.stringify(draft)); } catch (e) {} if (saved) saved.textContent = 'Saved in this browser ' + new Date().toLocaleTimeString(); };
    document.querySelectorAll('[data-k]').forEach((x) => {
      const k = x.dataset.k;
      if (x.type === 'radio') { x.checked = draft[k] === x.value; x.addEventListener('change', () => { draft[k] = x.value; save(); }); }
      else if (x.type === 'checkbox') { x.checked = !!draft[k + ':' + x.value]; x.addEventListener('change', () => { draft[k + ':' + x.value] = x.checked; save(); }); }
      else { x.value = draft[k] || ''; x.addEventListener('input', () => { draft[k] = x.value; save(); }); }
    });
    const text = () => {
      const out = ['# Orbital route map, round two: my answers', ''];
      for (const [key, name] of sections) {
        out.push(`## ${name}`, `Verdict: ${draft['v' + key] || '(none)'}`);
        const parts = [...document.querySelectorAll(`[data-k="p${key}"]`)].filter((x) => x.checked).map((x) => x.value);
        out.push('Keep: ' + (parts.join('; ') || '(none)'));
        if (draft['c' + key]) out.push('Comments: ' + draft['c' + key]);
        out.push('');
      }
      out.push('## Decisions', `Map for the Overview: ${draft.pick || '(none)'}`);
      if (draft.themes) out.push(`Themes: ${draft.themes}`);
      if (draft.vgname) out.push(`VektraGrid mark with the Harness Manager name: ${draft.vgname}`);
      if (draft.general) out.push('Anything else: ' + draft.general);
      return out.join('\n');
    };
    const box = document.getElementById('out');
    document.getElementById('copy').addEventListener('click', async () => { box.style.display = 'block'; box.value = text(); box.select(); try { await navigator.clipboard.writeText(box.value); saved.textContent = 'Copied. Paste it into the chat with Claude.'; } catch (e) { document.execCommand('copy'); saved.textContent = 'Copied (or select the text below and copy it).'; } });
    document.getElementById('dl').addEventListener('click', () => { const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([text()], { type: 'text/markdown' })); a.download = 'orbit-round2-answers.md'; a.click(); });
    document.getElementById('clear').addEventListener('click', () => { if (!window.confirm('Clear every answer on this page?')) return; draft = {}; save(); location.reload(); });
  }

  window.H2 = { el, graph, chain, edgeCol, nodeCol, body, route, limitArcs, arcPath, labelLayer, beads, selectable, dimmer, meanAng, spread, corridor, LEGEND, maps, mount, themePicker, feedback, when };
})();
