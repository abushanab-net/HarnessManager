// Round four on top of round two's page code: the headline and readouts in a band above the map, the attention cards in
// a strip below it, quiet labels, and less said on the selected route.
(function () {
  const { NOW, MEASURED, computers, running, kpi, series, fmt, money, dur, esc, pct, burn } = HM;
  const when = H2.when;
  // less on the selected route: an account says its week only (the 5-hour window is its inner arc); a tool says nothing
  const graph0 = H2.graph;
  H2.graph = () => {
    const G = graph0();
    for (const n of G.N) {
      if (n.kind === 'account' && n.acct.windows) { const wk = n.acct.windows[1]; n.sub = `week ${wk.used}%${wk.out ? ' · ≈ out ' + when(wk.out) : ''}`; }
      if (n.kind === 'tool') n.sub = '';
    }
    return G;
  };
  H2.dimmer = (map, bd) => (lit) => {
    map.querySelectorAll('[data-node]').forEach((g) => g.classList.toggle('dim', !!lit && !lit.has(g.dataset.node)));
    map.querySelectorAll('.lb[data-for]').forEach((d) => { const on = !!lit && lit.has(d.dataset.for); d.classList.toggle('dim', !!lit && !on); d.classList.toggle('lit', on); });
    map.querySelectorAll('path.e').forEach((p) => p.classList.toggle('dim', !!lit && !(lit.has(p.dataset.a) && lit.has(p.dataset.b))));
    if (bd) bd.filter(lit ? (e) => lit.has(e.a) && lit.has(e.b) : null);
  };
  function dots(vals, color, cols = 14, rows = 5) {
    const max = Math.max(...vals), w = cols * 6, hh = rows * 6;
    let s = `<svg width="${w}" height="${hh}" viewBox="0 0 ${w} ${hh}" aria-hidden="true">`;
    vals.slice(-cols).forEach((v, c) => { const lit = Math.max(1, Math.round((v / max) * rows)); for (let r = 0; r < rows; r++) s += `<circle cx="${c * 6 + 3}" cy="${hh - r * 6 - 3}" r="1.7" fill="${r < lit ? color : 'var(--line)'}"/>`; });
    return s + '</svg>';
  }
  H2.scene = (map) => {
    const cw = burn().find((b) => b.out);
    return `<div class="band"><div>
        <div class="crumb"><i class="dot"></i>Overview · live on 3 computers · ${running.length} sessions open</div>
        <h1><span class="w">${esc(cw.name)}</span> will likely run out of its weekly limit on ${when(cw.out).replace(/ /, ' at about ')}, ${dur(cw.reset - cw.out)} before it resets.</h1>
        <div class="basis">Measured <b>${cw.used}% used</b>, resets <b>${when(cw.reset)}</b>, read ${Math.round((NOW - MEASURED) / 1e3)} s ago. The run-out is an estimate from today's pace.</div></div>
      <div class="rds">
        <div class="rd"><label>Tokens<b class="d">${pct(kpi.tokens.v, kpi.tokens.prev)}</b></label><strong>${fmt(kpi.tokens.v)}</strong>${dots(series.tokens, 'var(--ac)')}</div>
        <div class="rd"><label>Cost<b class="d warn">+${money(kpi.cost.v - kpi.cost.prev, 0)}</b></label><strong>${money(kpi.cost.v, 0)}</strong>${dots(series.cost, 'var(--warn)')}</div>
        <div class="rd"><label>Cached<b class="d">+${Math.round((kpi.hit.v - kpi.hit.prev) * 100)} pt</b></label><strong>${Math.round(kpi.hit.v * 100)}<small>%</small></strong>${dots(series.hit.map((x) => x - 0.8), 'var(--ac)')}</div>
      </div></div>
      <section class="scene ${map.id}" aria-label="Live routes"><div class="map" data-map="${map.id}"></div>
        <div class="ov b">${map.legend}${computers.map((c) => `<span><i class="sq" style="background:${c.col}"></i>${c.name}</span>`).join('')}</div>
        <div class="ov h">${map.hint}</div></section>
      <section class="attn" aria-label="Needs attention"><div class="inc hd">Needs attention<span>3 open</span></div>
        <div class="inc"><div class="t"><i></i>${esc(cw.name)} weekly<em>≈ ${when(cw.out)}</em></div><p>Runs out ${dur(cw.reset - cw.out)} before its reset at today's pace. Move work to Claude Team.</p></div>
        <div class="inc neg"><div class="t"><i></i>Antigravity<em>login</em></div><p>The login expired, so its limits cannot be read.</p><button>Sign in</button></div>
        <div class="inc neg"><div class="t"><i></i>Groq<em>retry ${dur(4 * 60e3)}</em></div><p>Paused by the router after 3 errors in a row.</p></div></section>`;
  };
})();
