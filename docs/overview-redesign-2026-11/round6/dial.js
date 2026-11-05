// Horizon Dial, round five: the readouts float small in the top-left, the dial shrinks only as far as that box needs,
// the docks are spread along the baseline, and labels are names only until a route is selected.
(function () {
  const { el, graph, body, route, labelLayer, beads, selectable, meanAng, spread, corridor, LEGEND } = H2;
  const RING = { account: 0.42, tool: 0.64, router: 0.82, provider: 1 };
  const deg = (d) => (d * Math.PI) / 180;

  function draw(host, o) {
    const scene = host.parentElement, W = host.clientWidth, Hh = host.clientHeight;
    // the dial takes the whole width, but shrinks until its label ring passes under the readouts and the attention cards
    const cx = W / 2, cy = Hh - 64;
    const ovl = scene.querySelector('.ov.l');
    const boxes = [{ x0: 0, x1: ovl.offsetWidth + 110, y1: ovl.offsetHeight + 14 }];
    const clear = (R) => boxes.every((b) => { const r = R + 22, dx = cx >= b.x0 && cx <= b.x1 ? 0 : Math.min(Math.abs(b.x0 - cx), Math.abs(b.x1 - cx)); return dx >= r || cy - Math.sqrt(r * r - dx * dx) >= b.y1; });
    let R = Math.min(W / 2 - 130, Hh - 96);
    while (R > 140 && !clear(R)) R -= 4;
    const svg = el('svg', { viewBox: `0 0 ${W} ${Hh}` }); host.append(svg);
    const { N, E, by } = graph();
    const acc = N.filter((n) => n.kind === 'account');
    acc.forEach((n, i) => { n.ang = deg(-165 + i * 30); });
    const of = (pred) => N.filter(pred);
    for (const t of of((n) => n.kind === 'tool')) t.ang = meanAng(acc.filter((a) => a.tool === t.id).map((a) => a.ang));
    by.router.ang = meanAng(of((n) => n.kind === 'tool' && E.some((e) => e.a === n.id && e.b === 'router')).map((t) => t.ang));
    const provs = of((n) => n.kind === 'provider');
    for (const p of provs) { const via = E.filter((e) => e.b === p.id && e.a !== 'router').map((e) => by[e.a].ang); p.ang = via.length ? meanAng(via) : by.router.ang; }
    const routedP = provs.filter((p) => p.via === 'router');
    routedP.forEach((p, i) => { p.ang = by.router.ang + deg((i - (routedP.length - 1) / 2) * 16); });
    spread(provs, deg(12), false);
    provs.forEach((p) => { p.ang = Math.max(deg(-168), Math.min(deg(-12), p.ang)); });
    // the dial: baseline and the hop arcs, named at their left ends
    el('line', { x1: cx - R - 20, x2: cx + R + 20, y1: cy, y2: cy, stroke: 'var(--line)' }, svg);
    for (const [k, r] of Object.entries(RING)) {
      if (k === 'router') continue;
      el('path', { d: `M${cx - r * R} ${cy} A${r * R} ${r * R} 0 0 1 ${cx + r * R} ${cy}`, fill: 'none', stroke: 'var(--line)', 'stroke-dasharray': k === 'tool' ? '2 5' : '' }, svg);
      el('text', { x: cx - r * R, y: cy + 13, 'text-anchor': 'middle', fill: 'var(--m)', 'font-size': 10.5, 'font-family': 'var(--mono)' }, svg).textContent = { account: 'accounts', tool: 'tools', provider: 'providers' }[k];
    }
    // the computers' docks on the baseline
    const pcs = of((n) => n.kind === 'pc'), dockX = { lab: cx - 0.42 * R, studio: cx, surface: cx + 0.42 * R };
    const P = (n) => (n.kind === 'pc' ? [dockX[n.id], cy] : [cx + RING[n.kind] * R * Math.cos(n.ang), cy + RING[n.kind] * R * Math.sin(n.ang)]);
    for (const c of pcs) { const [x] = P(c); el('rect', { x: x - 22, y: cy - 3, width: 44, height: 6, rx: 2, fill: c.col, opacity: .5 }, svg); }
    const paths = E.map((e, i) => {
      const A = by[e.a], B = by[e.b];
      if (A.kind === 'pc') { const [x1, y1] = P(A), [x2, y2] = P(B); return { e, path: route(svg, e, `M${x1} ${y1} C${x1} ${y1 - 0.2 * R} ${x2} ${y2 + 0.16 * R} ${x2} ${y2}`) }; }
      const rA = RING[A.kind] * R, rB = RING[B.kind] * R, rc = (rA + rB) / 2 + ((i % 3) - 1) * 5;
      return { e, path: route(svg, e, corridor(cx, cy, rA, A.ang, rB, B.ang, rc)) };
    });
    const bd = beads(svg, paths.map((p) => ({ e: p.e, path: p.path, rate: p.e.rate })), o.motion);
    const lab = labelLayer(host);
    for (const n of N) {
      const [x, y] = P(n); body(svg, n, x, y);
      const right = n.kind === 'pc' ? true : Math.cos(n.ang) >= 0;
      if (n.kind === 'pc') lab.add(n, x, cy + 16, 'c', 'sm', `<b>${n.label}</b>`);
      else if (n.kind === 'account') lab.add(n, x + (right ? 22 : -22), y, right ? 'r' : 'l');
      else if (n.kind === 'tool') lab.add(n, x, y - 14, 'c', 'sm', `<b>${ticon(n.id, 12)}${n.label}</b><span class="${n.tone || ''}">${n.sub}</span>`);
      else if (n.kind === 'router') lab.add(n, x + (right ? 14 : -14), y, right ? 'r' : 'l', 'sm');
      else { const r = R + 18; lab.add(n, cx + r * Math.cos(n.ang), cy + r * Math.sin(n.ang), right ? 'r' : 'l'); }
    }
    host.classList.add('measuring'); const placed = lab.place(); host.classList.remove('measuring');
    for (const l of placed) if (l.n.kind === 'provider' && Math.abs(l.top - l.top0) > 4) el('path', { d: `M${l.n.x} ${l.n.y} L${l.side === 'r' ? l.left - 4 : l.left + l.w + 4} ${l.top + l.h / 2}`, stroke: 'var(--m)', 'stroke-width': .8, fill: 'none', opacity: .7 }, svg);
    // the finding's route is the default selection
    selectable(host, E, H2.dimmer(host, bd), 'codex-pro');
    return { stop: bd.stop };
  }

  H2.maps.dial = { id: 'dial', legend: LEGEND, hint: 'the finding’s route is selected · hover, focus or click another · Esc returns', draw };
})();
