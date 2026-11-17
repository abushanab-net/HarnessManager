// Round five: the whole Overview around the Horizon Dial. The scene carries the finding and small readouts; below it the
// attention strip, the limits as rings, running now, the year clock, and the latest replies.
(function () {
  const { NOW, MEASURED, computers, accounts, running, kpi, series, days, replies, fmt, money, dur, clock, esc, pct, pcName, pcCol, burn } = HM;
  const when = H2.when, PI = Math.PI;
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
  function dots(vals, color, cols = 10, rows = 3) {
    const max = Math.max(...vals), w = cols * 6, hh = rows * 6;
    let s = `<svg width="${w}" height="${hh}" viewBox="0 0 ${w} ${hh}" aria-hidden="true">`;
    vals.slice(-cols).forEach((v, c) => { const lit = Math.max(1, Math.round((v / max) * rows)); for (let r = 0; r < rows; r++) s += `<circle cx="${c * 6 + 3}" cy="${hh - r * 6 - 3}" r="1.7" fill="${r < lit ? color : 'var(--line)'}"/>`; });
    return s + '</svg>';
  }
  const arc = (cx, cy, r, f0, f1) => H2.arcPath(cx, cy, r, f0, f1);
  /** Every account's limits as rings: inner 5-hour, outer weekly; dashed is the estimate at the reset. */
  function rings() {
    return accounts.map((a) => {
      const S = 84, c = 42, tone = (u) => (u >= 80 ? 'var(--neg)' : u >= 55 ? 'var(--warn)' : 'var(--ac)');
      let g = '', note = '', mid = '', cls = '';
      if (a.needsLogin) { g = `<circle cx="${c}" cy="${c}" r="34" fill="none" stroke="var(--neg)" stroke-dasharray="3 5" opacity=".7"/>`; mid = '—'; note = 'sign in to read'; cls = 'neg'; }
      else if (a.cap) { const u = a.cap.spent / a.cap.cap; g = `<circle cx="${c}" cy="${c}" r="34" fill="none" stroke="var(--line)" stroke-width="6"/><path d="${arc(c, c, 34, 0, u)}" fill="none" stroke="var(--r2)" stroke-width="6" stroke-linecap="round"/>`; mid = Math.round((1 - u) * 100) + '%'; note = 'daily cap, resets 00:00'; }
      else if (!a.windows) { g = `<circle cx="${c}" cy="${c}" r="34" fill="none" stroke="var(--line)" stroke-dasharray="2 4"/>`; mid = '∞'; note = 'API key, no limit'; }
      else {
        a.windows.forEach((w, i) => { const r = i ? 34 : 25, u = w.used / 100, f = Math.min(1, (w.at || w.used) / 100);
          g += `<circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="var(--line)" stroke-width="6"/><path d="${arc(c, c, r, u, f)}" fill="none" stroke="${tone(w.at)}" stroke-width="6" stroke-dasharray="2 3" opacity=".55"/><path d="${arc(c, c, r, 0, u)}" fill="none" stroke="${tone(w.used)}" stroke-width="6" stroke-linecap="round"/>`; });
        const wk = a.windows[1]; mid = 100 - wk.used + '%';
        note = wk.out ? `≈ out ${when(wk.out)}` : `week resets ${when(wk.reset)}`; cls = wk.out ? 'warn' : '';
      }
      return `<div class="ring"><svg width="${S}" height="${S}" viewBox="0 0 ${S} ${S}" role="img" aria-label="${esc(a.label)}: ${esc(note)}">${g}<text x="${c}" y="${c + 5}" text-anchor="middle" font-size="14" font-weight="600" fill="var(--ink)" font-family="var(--sans)">${mid}</text></svg><b>${esc(a.label)}</b><small class="${cls}">${esc(note)}</small></div>`;
    }).join('');
  }
  /** The year clock: 53 weeks around, Monday inside to Sunday outside. */
  function yearClock() {
    const S = 300, c = 150, r0 = 58, r1 = 138, band = (r1 - r0) / 7;
    const nz = days.map((d) => d.tokens).filter(Boolean).sort((a, b) => a - b), q = (p) => nz[Math.floor(p * (nz.length - 1))], cuts = [q(0.25), q(0.5), q(0.75)], top = Math.max(...nz);
    const lv = (v) => (!v ? 0 : v >= top ? 4 : v <= cuts[0] ? 1 : v <= cuts[1] ? 2 : v <= cuts[2] ? 3 : 4), op = [0, 0.28, 0.5, 0.75, 1];
    const off = (days[0].day.getDay() + 6) % 7, weeks = Math.ceil((days.length + off) / 7);
    const P = (r, a) => `${(c + r * Math.cos(a)).toFixed(2)} ${(c + r * Math.sin(a)).toFixed(2)}`;
    let s = `<svg viewBox="0 0 ${S} ${S}" role="img" aria-label="Daily tokens over 53 weeks, as a year clock">`;
    let lastM = -1; const best = days.reduce((a, d) => (d.tokens > a.tokens ? d : a));
    days.forEach((d, i) => {
      const w = Math.floor((i + off) / 7), r = (i + off) % 7, a0 = (w / weeks) * 2 * PI - PI / 2 + 0.004, a1 = ((w + 1) / weeks) * 2 * PI - PI / 2 - 0.004;
      const ra = r0 + r * band + 0.6, rb = r0 + (r + 1) * band - 0.6, l = lv(d.tokens);
      s += `<path d="M${P(ra, a0)} A${ra} ${ra} 0 0 1 ${P(ra, a1)} L${P(rb, a1)} A${rb} ${rb} 0 0 0 ${P(rb, a0)} Z" fill="${l ? 'var(--ac)' : 'var(--hm0)'}" fill-opacity="${l ? op[l] : 1}"${d === best ? ' stroke="var(--ink)" stroke-width="1.2"' : ''}><title>${d.day.toDateString()}: ${fmt(d.tokens)} tokens</title></path>`;
      if (r === 0 && d.day.getMonth() !== lastM) { lastM = d.day.getMonth(); const am = (w / weeks) * 2 * PI - PI / 2; s += `<text x="${(c + (r1 + 9) * Math.cos(am)).toFixed(1)}" y="${(c + (r1 + 9) * Math.sin(am) + 3).toFixed(1)}" font-size="8.5" text-anchor="middle" fill="var(--m)" font-family="var(--mono)">${d.day.toLocaleDateString('en-GB', { month: 'short' })[0]}</text>`; }
    });
    s += `<text x="${c}" y="${c - 8}" text-anchor="middle" font-size="11" fill="var(--m)" font-family="var(--sans)">busiest day</text><text x="${c}" y="${c + 10}" text-anchor="middle" font-size="15" font-weight="600" fill="var(--ink)" font-family="var(--sans)">${best.day.toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })}</text><text x="${c}" y="${c + 25}" text-anchor="middle" font-size="10.5" fill="var(--m)" font-family="var(--mono)">${fmt(best.tokens)}</text>`;
    return s + '</svg>';
  }
  H2.scene = (map) => {
    const cw = burn().find((b) => b.out);
    return `<section class="scene ${map.id}" aria-label="Live routes"><div class="map" data-map="${map.id}"></div>
      <div class="ov l"><div>
        <div class="crumb"><i class="dot"></i>Overview · live on 3 computers · ${running.length} sessions open</div>
        <h1><span class="w">${esc(cw.name)}</span> will likely run out of its weekly limit on ${when(cw.out).replace(/ /, ' at about ')}, ${dur(cw.reset - cw.out)} before it resets.</h1>
        <div class="basis">Measured <b>${cw.used}% used</b>, resets <b>${when(cw.reset)}</b>, read ${Math.round((NOW - MEASURED) / 1e3)} s ago. The run-out is an estimate from today's pace.</div>
        </div>
        <div class="rds">
          <div class="rd"><label>Tokens</label><strong>${fmt(kpi.tokens.v)}</strong>${dots(series.tokens, 'var(--ac)')}<b class="d">${pct(kpi.tokens.v, kpi.tokens.prev)}</b></div>
          <div class="rd"><label>Cost</label><strong>${money(kpi.cost.v, 0)}</strong>${dots(series.cost, 'var(--warn)')}<b class="d warn">+${money(kpi.cost.v - kpi.cost.prev, 0)}</b></div>
          <div class="rd"><label>Cached</label><strong>${Math.round(kpi.hit.v * 100)}<small>%</small></strong>${dots(series.hit.map((x) => x - 0.8), 'var(--ac)')}<b class="d">+${Math.round((kpi.hit.v - kpi.hit.prev) * 100)} pt</b></div>
        </div></div>
      <div class="ov b">${map.legend}${computers.map((c) => `<span><i class="sq" style="background:${c.col}"></i>${c.name}</span>`).join('')}</div>
      <div class="ov h">${map.hint}</div></section>
    <section class="attn" aria-label="Needs attention"><div class="inc hd">Needs attention<span>3 open</span></div>
      <div class="inc"><div class="t"><i></i>${esc(cw.name)} weekly<em>≈ ${when(cw.out)}</em></div><p>Runs out ${dur(cw.reset - cw.out)} before its reset at today's pace. Move work to Claude Team.</p></div>
      <div class="inc neg"><div class="t"><i></i>Antigravity<em>login</em></div><p>The login expired, so its limits cannot be read.</p><button>Sign in</button></div>
      <div class="inc neg"><div class="t"><i></i>Groq<em>retry ${dur(4 * 60e3)}</em></div><p>Paused by the router after 3 errors in a row.</p></div></section>
    <div class="row3">
      <section class="pn"><div class="ph"><h2>Limits</h2><span>inner 5-hour, outer weekly · dashed is the estimate at the reset</span></div><div class="rings">${rings()}</div></section>
      <section class="pn"><div class="ph"><h2>Running now</h2><span class="r">${running.length} sessions</span></div><div class="runs">${running.map((r) => `<div><i style="background:${pcCol(r.device)}"></i><span><b>${esc(r.project)}</b> <small>${esc(r.account)}, ${pcName(r.device)}</small></span><time>${dur(r.since)}</time></div>`).join('')}</div></section>
      <section class="pn"><div class="ph"><h2>Activity</h2><span class="r">53 weeks, Mon inside</span></div><div class="clock">${yearClock()}</div></section>
    </div>
    <section class="pn wide"><div class="ph"><h2>Latest replies</h2><span>${kpi.requests.v.toLocaleString()} today · speed and first reply are approximate</span><a class="r lnk">Request log</a></div>
      <table class="rep"><thead><tr><th>Time</th><th>Account</th><th>Model</th><th>Computer</th><th class="n">Tokens</th><th class="n">Cost</th><th class="n">≈ Speed</th><th class="n">≈ First reply</th></tr></thead><tbody>
      ${replies.slice(0, 8).map((r) => `<tr><td class="tm">${clock(r.ts)}</td><td>${esc(r.account)}</td><td class="mono" style="font-size:var(--fs-xs)">${r.model}</td><td><span class="chip"><i style="background:${pcCol(r.device)}"></i>${pcName(r.device)}</span></td><td class="n">${fmt(r.input + r.output + r.cacheRead + r.cacheWrite)}</td><td class="n">${money(r.cost, 3)}</td><td class="n">${r.tps} tok/s</td><td class="n" style="${r.first > 4 ? 'color:var(--warn)' : ''}">${r.first} s</td></tr>`).join('')}</tbody></table></section>`;
  };
})();
