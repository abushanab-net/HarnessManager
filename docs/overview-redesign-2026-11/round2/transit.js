// Transit Orbit: six racetracks, one per account. The top straight is the route, read left to right; the bottom
// straight is that account's capacity; beads circulate the loop in proportion to traffic.
(function () {
  const { el, graph, limitArcs, labelLayer, beads, selectable, edgeCol, nodeCol, LEGEND, when } = H2;
  const { esc, pcCol, dur, accounts, providers, tools, router } = HM;
  const LANES = ['claude-max', 'claude-team', 'codex-pro', 'opencode-routed', 'pi-openrouter', 'antigravity'];
  const ST = { pc: 0.03, account: 0.1, tool: 0.42, router: 0.62, provider: 0.83 };

  function draw(host, o) {
    const scene = host.parentElement, W = host.clientWidth, Hh = host.clientHeight;
    const ovl = scene.querySelector('.ov.l').offsetWidth, ovr = scene.querySelector('.ov.r').offsetWidth;
    const x0 = ovl + 12, x1 = W - ovr - 12, gap = Math.max(26, Math.min(34, (Hh - 120) / 12)), r0 = 30, rOut = r0 + 5 * gap;
    // the ends are squashed (a stadium), so the straights keep their room on a narrow sheet
    const K = 0.55, cy = Hh / 2 + 10, cL = x0 + K * rOut, cR = x1 - K * rOut, L = cR - cL;
    const svg = el('svg', { viewBox: `0 0 ${W} ${Hh}` }); host.append(svg);
    const { N, E, by } = graph();
    const laneOf = {}, ry = {};
    LANES.forEach((id, i) => { laneOf[id] = i; ry[id] = rOut - i * gap; });
    const sx = (f) => cL + f * L, top = (id) => cy - ry[id], bot = (id) => cy + ry[id];
    const loop = (r) => `M${cL} ${cy - r} H${cR} A${(K * r).toFixed(1)} ${r} 0 0 1 ${cR} ${cy + r} H${cL} A${(K * r).toFixed(1)} ${r} 0 0 1 ${cL} ${cy - r} Z`;
    // which lanes each shared body serves
    const lanesOf = (id) => { const s = new Set(); for (const e of E) { if (e.a === id && laneOf[e.b] != null) s.add(e.b); if (e.b === id) { if (laneOf[e.a] != null) s.add(e.a); else for (const e2 of E) if (e2.b === e.a && laneOf[e2.a] != null) s.add(e2.a); } } if (laneOf[id] != null) s.add(id); return [...s]; };
    const laneSet = {}; for (const n of N) laneSet[n.id] = lanesOf(n.id);
    // the router serves routed lanes only; providers behind it inherit those lanes
    laneSet.router = LANES.filter((id) => by[id].k === 'a');
    for (const p of providers.filter((p) => p.via === 'router')) laneSet[p.id] = LANES.filter((id) => E.some((e) => e.a === 'router' && e.b === p.id) && by[id].k === 'a' && E.some((e) => e.a === by[id].tool && e.b === 'router') && ((p.id === 'deepseek' && id === 'opencode-routed') || (p.id === 'openrouter' && id === 'pi-openrouter')));
    const lab = labelLayer(host);
    const trackEls = {};
    // the finding's lane is tinted
    const fl = 'codex-pro';
    el('path', { d: loop(ry[fl] + gap / 2 - 2) + ' ' + loop(ry[fl] - gap / 2 + 2), fill: 'var(--warnS)', 'fill-rule': 'evenodd', opacity: .9, class: 'band' }, svg);
    // stations, named once above the outer track
    for (const [k, f] of Object.entries(ST)) if (k !== 'account') el('text', { x: sx(f), y: cy - rOut - 22, 'text-anchor': k === 'pc' ? 'start' : 'middle', fill: 'var(--m)', 'font-size': 10.5, 'font-family': 'var(--mono)' }, svg).textContent = { pc: 'computers · account', tool: 'tool', router: 'router', provider: 'provider' }[k];
    const items = [];
    for (const id of LANES) {
      const a = by[id], acct = a.acct, broken = a.k === 'r', routed = a.k === 'a', r = ry[id], y = top(id), yb = bot(id);
      const col = edgeCol(a.k);
      // the track
      const t = el('path', { d: loop(r), class: 'e', fill: 'none', stroke: col, 'stroke-width': broken ? 1.2 : a.rate ? 1.6 : 1, 'stroke-dasharray': broken ? '3 4' : '', opacity: broken ? .5 : a.rate ? .5 : .25, 'data-a': id, 'data-b': id }, svg);
      trackEls[id] = t;
      // beads run the real path only, computer to provider along the top straight; the return half is the track
      const run = el('path', { d: `M${sx(ST.pc)} ${y} H${sx(ST.provider)}`, fill: 'none', stroke: 'none' }, svg);
      items.push({ e: { a: id, b: id, k: a.k, rate: a.rate }, path: run, rate: a.rate, color: col });
      // the computers it runs on
      a.on.forEach((pc, i) => { const g = el('g', { class: 'n', 'data-node': pc }, svg); el('rect', { x: sx(ST.pc) + i * 14 - 5, y: y - 5, width: 10, height: 10, rx: 2.5, fill: pcCol(pc) }, g); g.setAttribute('tabindex', 0); g.setAttribute('role', 'button'); g.setAttribute('aria-label', HM.pcName(pc)); });
      // the account, with its limit arcs
      const ga = el('g', { class: 'n', 'data-node': id, tabindex: 0, role: 'button', 'aria-label': `${a.label}, ${a.sub}` }, svg);
      el('circle', { cx: sx(ST.account), cy: y, r: 15, fill: 'var(--s)' }, ga); limitArcs(ga, sx(ST.account), y, acct, 12); el('circle', { cx: sx(ST.account), cy: y, r: 4.5, fill: nodeCol(a) }, ga);
      el('circle', { cx: sx(ST.account), cy: y, r: 18, fill: 'transparent' }, ga);
      lab.add(a, sx(ST.account) + 20, y - 1, 'r', 'tag', `<b>${esc(a.label)}<span class="pc">${a.on.map((p) => `<i style="background:${pcCol(p)}"></i>`).join('')}</span></b>`);
      // the capacity gauge on the bottom straight
      const gx0 = sx(0.06), gx1 = sx(0.94), gw = gx1 - gx0, wk = acct.windows && acct.windows[1];
      el('line', { x1: gx0, x2: gx1, y1: yb, y2: yb, stroke: 'var(--line)', 'stroke-width': 4, 'stroke-linecap': 'round' }, svg);
      let note = '';
      if (acct.needsLogin) { note = '— limits not read: sign in'; }
      else if (acct.cap) { const u = acct.cap.spent / acct.cap.cap; el('line', { x1: gx0, x2: gx0 + gw * u, y1: yb, y2: yb, stroke: 'var(--r2)', 'stroke-width': 4, 'stroke-linecap': 'round' }, svg); note = `$${acct.cap.spent.toFixed(2)} of $${acct.cap.cap} today · resets 00:00`; }
      else if (!wk) { note = 'API key, no limit'; }
      else {
        const u = wk.used / 100, f = Math.min(1, (wk.at || wk.used) / 100), tone = wk.out ? 'var(--warn)' : col;
        el('line', { x1: gx0 + gw * u, x2: gx0 + gw * f, y1: yb, y2: yb, stroke: tone, 'stroke-width': 4, 'stroke-dasharray': '2 3', opacity: .6 }, svg);
        el('line', { x1: gx0, x2: gx0 + gw * u, y1: yb, y2: yb, stroke: tone, 'stroke-width': 4, 'stroke-linecap': 'round' }, svg);
        el('path', { d: `M${gx1} ${yb - 7} v14`, stroke: 'var(--m)', 'stroke-width': 1 }, svg);
        if (wk.out) { const X = gx0 + gw * 0.985; el('path', { d: `M${X - 4} ${yb - 4} l8 8 M${X + 4} ${yb - 4} l-8 8`, stroke: 'var(--neg)', 'stroke-width': 2 }, svg); }
        note = `week ${wk.used}% used · ≈ ${Math.min(100, wk.at)}% at the reset ${when(wk.reset)}${wk.out ? ' · ≈ out ' + when(wk.out) : ''} · 5-h ${acct.windows[0].used}%`;
      }
      lab.add(a, gx0, yb + 12, 'r', 'sm gauge', `<b class="${acct.windows && acct.windows[1] && acct.windows[1].out ? 'warn' : ''}"><i style="background:${col}"></i>${esc(a.label)} · ${esc(note)}</b>`);
    }
    // shared bodies: one tall rounded rect spanning the lanes it serves, labelled once
    const span = (id, f, kind) => {
      const n = by[id], ls = laneSet[id].filter((l) => laneOf[l] != null); if (!ls.length) return;
      const ys = ls.map(top), ya = Math.min(...ys) - 9, yb2 = Math.max(...ys) + 9, x = sx(f), col = nodeCol(n);
      const g = el('g', { class: 'n', 'data-node': id, tabindex: 0, role: 'button', 'aria-label': `${n.label}, ${n.sub}` }, svg);
      el('rect', { x: x - 8, y: ya, width: 16, height: yb2 - ya, rx: 5, fill: 'var(--s)', stroke: kind === 'router' ? 'var(--r2)' : n.k === 'r' ? 'var(--neg)' : n.rate ? col : 'var(--m)', 'stroke-width': 1.5, 'stroke-dasharray': n.k === 'r' && kind === 'provider' ? '2 2' : '' }, g);
      for (const l of ls) el('circle', { cx: x, cy: top(l), r: 3, fill: kind === 'tool' ? 'var(--t2)' : kind === 'router' ? 'var(--r2)' : col }, g);
      lab.add(n, x + 14, (ya + yb2) / 2, 'r', 'tag sm', `<b>${esc(n.label)}</b><span class="${n.tone || ''}">${esc(n.sub)}</span>`);
      return g;
    };
    for (const t of tools) span(t.id, ST.tool, 'tool');
    span('router', ST.router, 'router');
    for (const p of providers.filter((p) => !p.paused)) span(p.id, ST.provider, 'provider');
    // Groq hangs off the router: no lane reaches it
    const groq = by.groq, gy = (top('opencode-routed') + top('pi-openrouter')) / 2, gx = sx(0.74);
    el('path', { d: `M${sx(ST.router) + 8} ${gy} H${gx - 7}`, class: 'e', stroke: 'var(--neg)', 'stroke-dasharray': '3 4', fill: 'none', 'data-a': 'router', 'data-b': 'groq', opacity: .7 }, svg);
    const gg = el('g', { class: 'n', 'data-node': 'groq', tabindex: 0, role: 'button', 'aria-label': `Groq, ${groq.sub}` }, svg);
    el('circle', { cx: gx, cy: gy, r: 5, fill: 'var(--s)', stroke: 'var(--neg)', 'stroke-width': 1.5, 'stroke-dasharray': '2 2' }, gg);
    lab.add(groq, gx + 10, gy, 'r', 'tag sm', `<b>Groq</b><span class="neg">${esc(groq.sub)}</span>`);
    lab.place(2);
    const bd = beads(svg, items, o.motion);
    // selection by lane: a body lights every lane it serves
    const E2 = [];
    for (const id of Object.keys(laneSet)) for (const l of laneSet[id]) if (id !== l) E2.push({ a: id, b: l });
    for (const l of LANES) E2.push({ a: l, b: l });
    E2.push({ a: 'router', b: 'groq' });
    const apply = (lit) => {
      host.querySelectorAll('[data-node]').forEach((g) => g.classList.toggle('dim', !!lit && !lit.has(g.dataset.node)));
      host.querySelectorAll('.lb[data-for]').forEach((d) => d.classList.toggle('dim', !!lit && !lit.has(d.dataset.for)));
      host.querySelectorAll('path.e').forEach((p) => p.classList.toggle('dim', !!lit && !(lit.has(p.dataset.a) && lit.has(p.dataset.b))));
      bd.filter(lit ? (e) => lit.has(e.a) : null);
    };
    selectable(host, E2, apply);
    return { stop: bd.stop };
  }

  H2.maps.transit = { id: 'transit', legend: LEGEND + '<span><i class="sq" style="background:var(--warnS);box-shadow:inset 0 0 0 1px var(--warn)"></i>the finding’s lane</span>', hint: 'each lane reads left to right · the bottom straight is its capacity · hover a lane', draw };
})();
