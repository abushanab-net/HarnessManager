// Route Atlas, round four: the circle has the whole scene; the headline and readouts sit above it, attention below.
(function () {
  const { el, graph, body, route, labelLayer, beads, selectable, meanAng, spread, corridor, LEGEND } = H2;
  const RING = { pc: 0.17, account: 0.46, tool: 0.66, router: 0.84, provider: 1 };
  const deg = (d) => (d * Math.PI) / 180;

  function draw(host, o) {
    const scene = host.parentElement, W = host.clientWidth, Hh = host.clientHeight;
    const cx = W / 2, cy = Hh / 2 - 4, R = Math.min(W / 2 - 150, Hh / 2 - 46);
    const svg = el('svg', { viewBox: `0 0 ${W} ${Hh}` }); host.append(svg);
    const { N, E, by } = graph();
    // angles: accounts in stable order every 60°, everything else at the mean of what it serves
    const acc = N.filter((n) => n.kind === 'account');
    acc.forEach((n, i) => { n.ang = deg(-90 + i * 60); });
    const of = (pred) => N.filter(pred);
    for (const t of of((n) => n.kind === 'tool')) t.ang = meanAng(acc.filter((a) => a.tool === t.id).map((a) => a.ang));
    const routedTools = of((n) => n.kind === 'tool' && E.some((e) => e.a === n.id && e.b === 'router'));
    by.router.ang = meanAng(routedTools.map((t) => t.ang));
    const provs = of((n) => n.kind === 'provider');
    for (const p of provs) {
      const viaTools = E.filter((e) => e.b === p.id && e.a !== 'router').map((e) => by[e.a].ang);
      p.ang = viaTools.length ? meanAng(viaTools) : by.router.ang;
    }
    const routedP = provs.filter((p) => p.via === 'router');
    routedP.forEach((p, i) => { p.ang = by.router.ang + deg((i - (routedP.length - 1) / 2) * 22); });
    spread(provs, deg(14), true);
    for (const c of of((n) => n.kind === 'pc')) c.ang = meanAng(c.on.map((id) => by[id].ang));
    spread(of((n) => n.kind === 'pc'), deg(40), true);
    // rings, named at the top
    for (const [k, r] of Object.entries(RING)) {
      if (k === 'router') continue;
      el('circle', { cx, cy, r: r * R, fill: 'none', stroke: 'var(--line)', 'stroke-dasharray': k === 'tool' ? '2 5' : '' }, svg);
      el('text', { x: cx + (r * R + 8) * Math.cos(deg(-120)), y: cy + (r * R + 8) * Math.sin(deg(-120)), 'text-anchor': 'end', class: 'ring', fill: 'var(--m)', 'font-size': 10.5, 'font-family': 'var(--mono)' }, svg).textContent = { pc: 'computers', account: 'accounts', tool: 'tools', provider: 'providers' }[k];
    }
    // routes as corridors, staggered so parallel ones read apart
    const P = (n) => [cx + RING[n.kind] * R * Math.cos(n.ang), cy + RING[n.kind] * R * Math.sin(n.ang)];
    const paths = E.map((e, i) => {
      const A = by[e.a], B = by[e.b], rA = RING[A.kind] * R, rB = RING[B.kind] * R, rc = (rA + rB) / 2 + ((i % 3) - 1) * 5;
      return { e, path: route(svg, e, corridor(cx, cy, rA, A.ang, rB, B.ang, rc)) };
    });
    const bd = beads(svg, paths.map((p) => ({ e: p.e, path: p.path, rate: p.e.rate })), o.motion);
    const lab = labelLayer(host);
    for (const n of N) {
      const [x, y] = P(n); body(svg, n, x, y);
      const right = Math.cos(n.ang) >= 0;
      if (n.kind === 'account') lab.add(n, x + (right ? 22 : -22), y, right ? 'r' : 'l');
      else if (n.kind === 'tool') lab.add(n, x, y + 16, 'c', 'sm');
      else if (n.kind === 'router') lab.add(n, x + (right ? 14 : -14), y, right ? 'r' : 'l', 'sm');
      else if (n.kind === 'pc') lab.add(n, x, y - 14, 'c', 'sm', `<b>${n.label}</b>`);
      else { const r = R + 18; lab.add(n, cx + r * Math.cos(n.ang), cy + r * Math.sin(n.ang), right ? 'r' : 'l'); }
    }
    host.classList.add('measuring'); const placed = lab.place(); host.classList.remove('measuring');
    // a leader to any provider label that had to move
    for (const l of placed) if (l.n.kind === 'provider' && Math.abs(l.top - l.top0) > 4) el('path', { d: `M${l.n.x} ${l.n.y} L${l.side === 'r' ? l.left - 4 : l.left + l.w + 4} ${l.top + l.h / 2}`, stroke: 'var(--m)', 'stroke-width': .8, fill: 'none', opacity: .7 }, svg);
    selectable(host, E, H2.dimmer(host, bd), 'codex-pro');
    return { stop: bd.stop };
  }

  H2.maps.atlas = { id: 'atlas', legend: LEGEND, hint: 'the finding’s route is selected · hover, focus or click another · Esc returns', draw };
})();
