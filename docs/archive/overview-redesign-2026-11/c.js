// C, Runway (Vercel's request ledger, Linear's small multiples, Pulser's calm): the capacity you have left, over time.
(function () {
  const { NOW, MEASURED, H, DAY, computers, accounts, tools, providers, edges, running, kpi, series, days, replies, fmt, money, dur, at, clock, esc, pct, pcName, pcCol, shell, local } = HM;
  const T0 = NOW - DAY, T1 = NOW + 6 * DAY;
  const active = (t) => { const h = new Date(t).getHours(); return h >= 8 && h < 23 ? 1 : 0.08; };
  const act = (a, b) => { let s = 0; for (let t = a; t < b; t += 10 * 60e3) s += Math.min(10 * 60e3, b - t) * active(t); return s; };
  const when = (t) => new Date(t).toLocaleString('en-GB', { weekday: 'short', hour: '2-digit', minute: '2-digit' });

  // every window as a lane: measured up to now, then the same pace forward with an uncertainty that grows
  function lanes() {
    const out = [], rnd = HM.rng(11);
    for (const a of accounts) {
      const ws = (a.windows || []).map((w) => ({ ...w, period: w.label === '5-hour' ? 5 * H : 7 * DAY }));
      if (a.cap) { const mid = new Date(NOW); mid.setHours(24, 0, 0, 0); ws.push({ label: 'Daily cap', used: (a.cap.spent / a.cap.cap) * 100, reset: mid - NOW, period: DAY, cap: a.cap }); }
      if (a.needsLogin) { out.push({ acct: a, label: 'not read', none: true }); continue; }
      ws.forEach((w, i) => {
        const r0 = NOW + w.reset, resets = [];
        for (let t = r0 - Math.ceil((r0 - T0) / w.period) * w.period; t <= T1 + w.period; t += w.period) resets.push(t);
        const sF = w.out ? (100 - w.used) / act(NOW, NOW + w.out) : w.cap ? w.used / act(new Date(NOW).setHours(0, 0, 0, 0), NOW) : ((w.at || w.used) - w.used) / act(NOW, r0);
        const pts = [], outs = [];
        for (let k = 0; k < resets.length - 1; k++) {
          const cs = resets[k], ce = resets[k + 1];
          if (ce <= T0 || cs >= T1) continue;
          let s, lv;
          if (ce <= NOW) { s = (45 + rnd() * 45) / Math.max(1, act(cs, ce)); lv = 0; }
          else if (cs <= NOW) { s = cs >= T0 ? w.used / Math.max(1, act(cs, NOW)) : sF; lv = 0; }
          else { s = sF; lv = 0; }
          let t = Math.max(cs, T0);
          if (cs < T0) lv = Math.max(0, w.used - s * act(T0, NOW));
          let outStart = null;
          while (t < Math.min(ce, T1)) {
            if (cs <= NOW && t < NOW && t + 10 * 60e3 >= NOW) { pts.push([t, lv]); t = NOW; lv = w.used; s = sF; pts.push([t, lv, 'now']); continue; }
            const dt = Math.min(10 * 60e3, Math.min(ce, T1) - t);
            lv = Math.min(100, lv + s * dt * active(t));
            t += dt;
            if (lv >= 100 && outStart == null && t > NOW) outStart = t;
            pts.push([t, lv]);
          }
          if (outStart) outs.push([cs <= NOW && w.out ? NOW + w.out : outStart, Math.min(ce, T1)]); // the forecast's own run-out, not the 10-minute step's
          if (ce < T1) pts.push([ce, 0, 'reset']);
        }
        out.push({ acct: a, w, first: i === 0, label: w.label, pts, outs, resets: resets.filter((t) => t > NOW && t < T1), cap: w.cap });
      });
    }
    return out;
  }
  const levelAt = (L, t) => { if (L.none) return null; let p = L.pts[0]; for (const q of L.pts) { if (q[0] > t) break; p = q; } return p[1]; };
  const outAt = (L, t) => !L.none && L.outs.some(([a, b]) => t >= a && t < b);
  const band = (t) => 0.06 + 0.11 * Math.sqrt(Math.max(0, t - NOW) / DAY);

  function runway(W, ls) {
    const LW = 168, RW = 8, LH = 34, GAP = 6, GG = 14, TOP = 30;
    const X = (t) => LW + ((t - T0) / (T1 - T0)) * (W - LW - RW);
    let y = TOP, s = '';
    // days and nights
    for (let d = new Date(T0).setHours(0, 0, 0, 0); d < T1; d += DAY) {
      const n0 = Math.max(T0, d), n1 = Math.min(T1, d + 8 * H), n2 = Math.max(T0, d + 23 * H), n3 = Math.min(T1, d + DAY);
      if (n1 > n0) s += `<rect x="${X(n0)}" y="${TOP - 8}" width="${X(n1) - X(n0)}" height="9999" fill="var(--night)"/>`;
      if (n3 > n2) s += `<rect x="${X(n2)}" y="${TOP - 8}" width="${X(n3) - X(n2)}" height="9999" fill="var(--night)"/>`;
      if (d + 12 * H > T0 && d + 12 * H < T1) s += `<text class="ax ${new Date(d).getDate() === new Date(NOW).getDate() ? 'today' : ''}" x="${X(d + 12 * H)}" y="12" text-anchor="middle">${new Date(d).toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric' })}</text>`;
      if (d > T0) s += `<line x1="${X(d)}" x2="${X(d)}" y1="${TOP - 8}" y2="9999" stroke="var(--line)"/>`;
    }
    const rows = [];
    ls.forEach((L, i) => {
      if (i && L.acct !== ls[i - 1].acct) y += GG;
      const y0 = y, y1 = y + LH, Y = (v) => y1 - (v / 100) * (LH - 4);
      rows.push({ L, y0, y1, Y });
      if (L.first !== false || L.none) s += `<text class="lane-l" x="0" y="${y0 + 14}">${esc(L.acct.label)}</text>`;
      s += `<text class="lane-w" x="0" y="${y0 + (L.first !== false || L.none ? 28 : 18)}">${esc(L.none ? 'limits not read' : L.label)}</text>`;
      s += `<line x1="${LW}" x2="${W - RW}" y1="${y1}" y2="${y1}" stroke="var(--line)"/>`;
      if (L.none) { s += `<rect x="${LW + 4}" y="${y0 + 4}" width="${W - LW - RW - 8}" height="${LH - 8}" rx="4" fill="none" stroke="var(--neg)" stroke-dasharray="3 4" opacity=".5"/><text x="${LW + 16}" y="${y0 + LH / 2 + 4}" font-size="11.5" fill="var(--neg)">Sign in to Antigravity to read its limits. Nothing is drawn until then.</text>`; y = y1 + GAP; return; }
      const past = L.pts.filter((p) => p[0] <= NOW), fut = L.pts.filter((p) => p[0] >= NOW);
      const col = L.cap ? 'var(--iris)' : 'var(--ac)';
      const area = (pp) => `M${X(pp[0][0])} ${y1} ` + pp.map((p) => `L${X(p[0]).toFixed(1)} ${Y(p[1]).toFixed(1)}`).join(' ') + ` L${X(pp[pp.length - 1][0])} ${y1} Z`;
      s += `<path d="${area(past)}" fill="${col}" fill-opacity=".22"/><path d="${past.map((p, j) => (j ? 'L' : 'M') + X(p[0]).toFixed(1) + ' ' + Y(p[1]).toFixed(1)).join(' ')}" fill="none" stroke="${col}" stroke-width="1.5"/>`;
      // the estimate: a band, never a hard line
      const hi = fut.map((p) => [p[0], Math.min(100, p[1] * (1 + band(p[0])))]), lo = fut.map((p) => [p[0], p[1] * (1 - band(p[0]))]);
      s += `<path d="${hi.map((p, j) => (j ? 'L' : 'M') + X(p[0]).toFixed(1) + ' ' + Y(p[1]).toFixed(1)).join(' ')} ${lo.reverse().map((p) => 'L' + X(p[0]).toFixed(1) + ' ' + Y(p[1]).toFixed(1)).join(' ')} Z" fill="${col}" fill-opacity=".10"/>`;
      s += `<path d="${fut.map((p, j) => (j ? 'L' : 'M') + X(p[0]).toFixed(1) + ' ' + Y(p[1]).toFixed(1)).join(' ')}" fill="none" stroke="${col}" stroke-width="1.2" stroke-dasharray="3 3" opacity=".8"/>`;
      for (const [a, b] of L.outs) {
        s += `<rect x="${X(a)}" y="${y0 + 2}" width="${Math.max(2, X(b) - X(a))}" height="${LH - 4}" fill="url(#outC)"/><line x1="${X(a)}" x2="${X(a)}" y1="${y0}" y2="${y1}" stroke="var(--neg)" stroke-width="1.5"/>`;
        if (a < NOW + 3 * DAY) s += `<text class="outT" x="${X(a) - 6}" y="${y0 + 12}" text-anchor="end">≈ out ${when(a)}</text>`;
      }
      if (L.label !== '5-hour') for (const r of L.resets) s += `<path d="M${X(r)} ${y1} v-${LH - 2} l6 3 l-6 3" fill="var(--m)" stroke="var(--m)" stroke-width="1"/><text class="flag" x="${X(r) + 9}" y="${y0 + 10}">resets ${when(r)}</text>`;
      y = y1 + GAP;
    });
    const Ht = y + 22;
    s += `<line class="now" x1="${X(NOW)}" x2="${X(NOW)}" y1="${TOP - 10}" y2="${Ht}"/><text class="nowt" x="${X(NOW) - 6}" y="${Ht - 5}" text-anchor="end">measured</text><text x="${X(NOW) + 6}" y="${Ht - 5}" font-size="11" fill="var(--m)">estimated at today's pace</text>`;
    const svg = `<svg viewBox="0 0 ${W} ${Ht}" height="${Ht}" role="img" aria-label="Every limit over the next week, measured to now and estimated after">
      <defs><pattern id="outC" width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="5" height="5" fill="var(--negS)"/><line x1="0" y1="0" x2="0" y2="5" stroke="var(--neg)" stroke-width="1.2" opacity=".5"/></pattern></defs>
      ${s}<g class="scrub"></g><rect class="hit" x="${LW}" y="0" width="${W - LW - RW}" height="${Ht}"/></svg>`;
    return { svg, rows, X, LW, RW, Ht, inv: (x) => T0 + ((x - LW) / (W - LW - RW)) * (T1 - T0) };
  }

  // the setup as one flow, its widths the tokens per second of the last five minutes
  function flow(W) {
    const Hh = 236, nodeW = 6, cols = [0, 0.3, 0.56, 1], SLOT = 27; // a slot per node so small flows keep a readable label
    const val = {};
    for (const e of edges) { val[e[0]] = (val[e[0]] || 0); val[e[1]] = (val[e[1]] || 0) + e[3]; }
    for (const a of accounts) val[a.id] = a.tps;
    const colsOf = [accounts.map((a) => a.id), tools.map((t) => t.id), ['router'], providers.map((p) => p.id)];
    const k = (Hh - 6 * SLOT) / accounts.reduce((s, a) => s + a.tps, 0);
    const nodes = {};
    colsOf.forEach((ids, c) => {
      const hs = ids.map((id) => Math.max(3, (val[id] || 0) * k)), slots = hs.map((x) => Math.max(SLOT, x + 8)), total = slots.reduce((a, b) => a + b, 0);
      let y = (Hh - total) / 2;
      ids.forEach((id, i) => { nodes[id] = { x: cols[c] * (W - 150), y, h: hs[i], inO: 0, outO: 0, c }; y += slots[i]; });
    });
    let s = '';
    for (const [a, b, kind, rate] of edges) {
      if (!nodes[a] || !nodes[b]) continue;
      const A = nodes[a], B = nodes[b], th = Math.max(rate ? 1.5 : 0, rate * k);
      const x0 = A.x + nodeW, x1 = B.x, y0 = A.y + A.outO, y1 = B.y + B.inO, mx = (x0 + x1) / 2;
      A.outO += th; B.inO += th;
      const col = kind === 'a' ? 'var(--iris)' : kind === 'r' ? 'var(--neg)' : 'var(--ac)';
      if (!rate) { s += `<path d="M${x0} ${y0 + 1} C${mx} ${y0 + 1} ${mx} ${y1 + 1} ${x1} ${y1 + 1}" fill="none" stroke="${col}" stroke-dasharray="3 4" opacity=".6"/>`; continue; }
      s += `<path d="M${x0} ${y0} C${mx} ${y0} ${mx} ${y1} ${x1} ${y1} L${x1} ${y1 + th} C${mx} ${y1 + th} ${mx} ${y0 + th} ${x0} ${y0 + th} Z" fill="${col}" fill-opacity=".18"/>`;
      s += `<path class="fl" d="M${x0} ${y0 + th / 2} C${mx} ${y0 + th / 2} ${mx} ${y1 + th / 2} ${x1} ${y1 + th / 2}" fill="none" stroke="${col}" stroke-width="${Math.min(2, th)}" stroke-dasharray="1.5 9" stroke-linecap="round" data-v="${(0.4 + Math.log10(rate) * 0.5).toFixed(2)}"/>`;
    }
    for (const [id, n] of Object.entries(nodes)) {
      const p = providers.find((x) => x.id === id), a = accounts.find((x) => x.id === id);
      const col = (p && p.paused) || (a && a.needsLogin) ? 'var(--neg)' : id === 'router' || (p && p.via === 'router') || (a && a.kind === 'routed') ? 'var(--iris)' : (val[id] || 0) ? 'var(--ac)' : 'var(--m)';
      s += `<rect x="${n.x}" y="${n.y}" width="${nodeW}" height="${n.h}" rx="1.5" fill="${col}"/>`;
      const sub = p ? (p.paused ? 'paused' : p.tps ? fmt(p.tps) + ' tok/s' : 'idle') : a ? (a.needsLogin ? 'login expired' : fmt(a.tps) + ' tok/s') : id === 'router' ? ':8787' : '';
      s += `<text class="nl" x="${n.x + nodeW + 6}" y="${n.y + n.h / 2 + (sub ? -1 : 4)}">${esc(HM.label(id))}</text>${sub ? `<text class="ns" x="${n.x + nodeW + 6}" y="${n.y + n.h / 2 + 11}">${sub}</text>` : ''}`;
    }
    ['Accounts', 'Tools', 'Router', 'Providers'].forEach((t, i) => { s += `<text class="ch" x="${cols[i] * (W - 150)}" y="-6">${t}</text>`; });
    return `<svg viewBox="0 -18 ${W} ${Hh + 26}" height="${Hh + 26}" role="img" aria-label="Tokens per second from each account through its tool and the router to each provider">${s}</svg>`;
  }

  function small(vals, prev, color) {
    const n = 24, max = Math.max(...vals, ...prev) * 1.1, X = (i) => (i / (n - 1)) * 100, Y = (v) => 50 - (v / max) * 44;
    const line = (a) => a.map((v, i) => (i ? 'L' : 'M') + X(i).toFixed(2) + ' ' + Y(v).toFixed(2)).join(' ');
    return `<svg viewBox="0 0 100 52" preserveAspectRatio="none" aria-hidden="true">${[0, 6, 12, 18].map((hh) => `<line x1="${X(hh)}" x2="${X(hh)}" y1="0" y2="50" stroke="var(--line2)" vector-effect="non-scaling-stroke"/>`).join('')}
      <line x1="0" x2="100" y1="50" y2="50" stroke="var(--line)" vector-effect="non-scaling-stroke"/>
      <path d="${line(prev)}" fill="none" stroke="var(--m)" stroke-opacity=".45" stroke-width="1" vector-effect="non-scaling-stroke"/>
      <path d="${line(vals)} L${X(vals.length - 1)} 50 L0 50 Z" fill="${color}" fill-opacity=".12"/><path d="${line(vals)}" fill="none" stroke="${color}" stroke-width="1.6" vector-effect="non-scaling-stroke"/></svg>`;
  }

  function heat(W) {
    const off = (days[0].day.getDay() + 6) % 7, cols = Math.ceil((days.length + off) / 7), gap = 2, cell = Math.max(5, Math.min(12, Math.floor(W / cols) - gap));
    const nz = days.map((d) => d.tokens).filter(Boolean).sort((a, b) => a - b), q = (p) => nz[Math.floor(p * (nz.length - 1))], cuts = [q(0.25), q(0.5), q(0.75)], top = Math.max(...nz);
    const lv = (v) => (!v ? 0 : v >= top ? 4 : v <= cuts[0] ? 1 : v <= cuts[1] ? 2 : v <= cuts[2] ? 3 : 4), op = [1, 0.25, 0.45, 0.7, 1];
    let s = `<svg viewBox="0 0 ${cols * (cell + gap)} ${7 * (cell + gap)}" role="img" aria-label="Daily tokens, 53 weeks">`;
    days.forEach((d, i) => { const c = Math.floor((i + off) / 7), r = (i + off) % 7, l = lv(d.tokens); s += `<rect x="${c * (cell + gap)}" y="${r * (cell + gap)}" width="${cell}" height="${cell}" rx="2" fill="${l ? 'var(--ac)' : 'var(--line2)'}" fill-opacity="${op[l]}"><title>${d.day.toDateString()}: ${fmt(d.tokens)}</title></rect>`; });
    const best = days.reduce((a, d) => (d.tokens > a.tokens ? d : a));
    return s + `</svg><p>Busiest day ${best.day.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' })}, ${fmt(best.tokens)} tokens. A year of every computer added up.</p>`;
  }

  function render(app, o) {
    const ls = lanes(), cw = ls.find((L) => L.outs && L.outs.length && L.label === 'Weekly');
    const outT = cw.outs[0][0], resetT = NOW + cw.w.reset;
    const head = `<span class="w">${esc(cw.acct.label)}</span> will likely run out of its weekly limit ${when(outT).replace(/^(\w+) /, 'on $1 at about ')}, ${dur(resetT - outT)} before it resets.`;
    const tokRate = kpi.tokens.v / act(new Date(NOW).setHours(0, 0, 0, 0), NOW), costRate = kpi.cost.v / act(new Date(NOW).setHours(0, 0, 0, 0), NOW);
    const prevOf = (a, f) => Array.from({ length: 24 }, (_, i) => (a[Math.min(i, a.length - 1)] || a[a.length - 1]) * (0.8 + 0.12 * Math.sin(i * f)) * (i > 15 ? 0.7 : 1));
    const main = `
      <div class="lead"><div>
        <div class="crumb"><i class="dot"></i>Overview · live on 3 computers · ${running.length} sessions open</div>
        <h1 data-h>${head}</h1>
        <div class="basis" data-basis><span>Measured <b>${cw.w.used}% used</b>, resets <b>${when(resetT)}</b>, read ${Math.round((NOW - MEASURED) / 1e3)} s ago.</span><span>Estimated from today's pace, give or take ${Math.round(band(outT) * 100)}%.</span></div></div>
        <div class="ctl"><span class="seg"><button class="on">All tools</button><button>Claude Code</button><button>Codex</button><button>OpenCode</button></span><span class="seg"><button class="on">Today</button><button>7d</button><button>30d</button></span><button class="btn">${HM.icon('play', 12)} Start Claude Team</button></div></div>

      <section class="sec"><div class="sh"><h2>Runway</h2><span>every limit, a day back and six ahead</span><span class="r" data-at>Drag the line, or focus it and use the arrow keys, to look ahead</span></div>
        <div class="box"><div class="rw" tabindex="0" role="slider" aria-label="Look ahead in time" aria-valuemin="0" aria-valuemax="144" aria-valuenow="0"></div>
          <div class="rwleg"><span><i style="background:var(--ac);opacity:.5"></i>Used, measured</span><span><i style="background:var(--ac);opacity:.18"></i>Estimate at today's pace, widening with distance</span><span><i style="background:var(--negS);box-shadow:inset 0 0 0 1px var(--neg)"></i>Out until the reset</span><span><i style="background:var(--iris);opacity:.4"></i>Router daily cap</span><span><i style="background:var(--night);box-shadow:inset 0 0 0 1px var(--line)"></i>Night, little use</span></div></div></section>

      <section class="sec"><div class="sh"><h2>Today</h2><span>against yesterday by the hour</span><span class="r">${kpi.requests.v.toLocaleString()} requests</span></div>
        <div class="box sm">
          <div><label>Tokens<b>${pct(kpi.tokens.v, kpi.tokens.prev)}</b></label><strong>${fmt(kpi.tokens.v)}</strong>${small(series.tokens, prevOf(series.tokens, 0.6), 'var(--ac)')}<div class="proj" data-p="tok"></div></div>
          <div><label>Cost at list prices<b class="warn">+${money(kpi.cost.v - kpi.cost.prev)}</b></label><strong>${money(kpi.cost.v)}</strong>${small(series.cost, prevOf(series.cost, 0.4), 'var(--warn)')}<div class="proj" data-p="cost"></div></div>
          <div><label>Read from cache<b>+${Math.round((kpi.hit.v - kpi.hit.prev) * 100)} pt</b></label><strong>${Math.round(kpi.hit.v * 100)}<small>%</small></strong>${small(series.hit, series.hit.concat(series.hit.slice(0, 8)).map((x) => x - 0.04), 'var(--ac)')}<div class="proj" data-p="hit"></div></div>
          <div><label>Live now<b>last 5 min</b></label><strong>${accounts.reduce((a, x) => a + x.tps, 0).toLocaleString()}<small> tok/s</small></strong>${small(series.tps.slice(-24), series.tps.slice(0, 24), 'var(--iris)')}<div class="proj" data-p="tps"></div></div>
        </div></section>

      <section class="sec"><div class="sh"><h2>Where the tokens go</h2><span>widths are tokens per second over the last 5 minutes</span></div><div class="box flow"></div></section>

      <section class="sec two">
        <div><div class="sh"><h2>Running now</h2><span>${running.length} sessions on 3 computers</span></div><div class="box runs">${running.map((r) => `<div><span><b>${esc(r.project)}</b></span><span class="pc"><i style="background:${pcCol(r.device)}"></i>${pcName(r.device)}</span><span style="color:var(--t2)">${esc(r.account)}</span><time>${dur(r.since)}</time></div>`).join('')}</div></div>
        <div><div class="sh"><h2>Activity</h2><span>53 weeks</span></div><div class="box hmc"></div></div></section>

      <section class="sec"><div class="sh"><h2>Requests</h2><span>speed and first reply are approximate, from the times the tools wrote their logs</span></div>
        <div class="box"><div class="ltools"><span class="q">${HM.icon('search', 14)} Filter by model, account or project</span><button class="chipf on">All tools</button><button class="chipf">Slow first reply</button><button class="chipf">Through the router</button><button class="chipf">Lab</button></div>
        <table class="led"><thead><tr><th>Time</th><th>Account</th><th>Model</th><th>Route</th><th>Tokens · fresh, output, cache</th><th class="n">Cost</th><th class="n">≈ Speed</th><th>≈ First reply</th></tr></thead><tbody>
        ${replies.slice(0, 9).map((r) => { const tot = r.input + r.output + r.cacheRead + r.cacheWrite, routed = /routed|OpenRouter/.test(r.account);
          return `<tr><td class="tm">${clock(r.ts)}</td><td>${esc(r.account)}</td><td class="mono" style="font-size:var(--fs-xs)">${r.model}</td><td><span class="tag ${routed ? 'a' : ''}">${routed ? 'router' : 'direct'}</span></td>
          <td><span class="mix"><i style="width:${(r.input / tot) * 100}%;background:var(--ac)"></i><i style="width:${(r.output / tot) * 100}%;background:var(--iris)"></i><i style="width:${((r.cacheRead + r.cacheWrite) / tot) * 100}%;background:var(--acB);opacity:.6"></i></span>${fmt(tot)}</td>
          <td class="n">${money(r.cost, 3)}</td><td class="n">${r.tps} tok/s</td><td><span class="ttft ${r.first > 4 ? 'slow' : ''}" style="width:${r.first * 12}px"></span>${r.first} s</td></tr>`; }).join('')}</tbody></table></div></section>`;
    app.innerHTML = shell(main);

    const host = app.querySelector('.rw');
    const R = runway(host.clientWidth, ls);
    host.innerHTML = R.svg;
    app.querySelector('.flow').innerHTML = flow(app.querySelector('.flow').clientWidth - 32);
    app.querySelector('.hmc').innerHTML = heat(app.querySelector('.hmc').clientWidth - 32);
    const svg = host.querySelector('svg'), g = svg.querySelector('.scrub'), h1 = app.querySelector('[data-h]'), basis = app.querySelector('[data-basis]'), atEl = app.querySelector('[data-at]');
    const P = Object.fromEntries([...app.querySelectorAll('[data-p]')].map((el) => [el.dataset.p, el]));
    const head0 = h1.innerHTML, basis0 = basis.innerHTML;

    // look ahead: the line, every lane's level there, the headline and the key numbers, all at that time
    let T = NOW;
    const draw = () => {
      const x = R.X(T), ahead = T - NOW > 20 * 60e3;
      let s = `<line x1="${x}" x2="${x}" y1="18" y2="${R.Ht}"/><rect class="h" x="${x - 34}" y="2" width="68" height="16" rx="8"/><text x="${x}" y="14" text-anchor="middle">${ahead ? when(T) : 'now'}</text>`;
      for (const r of R.rows) {
        if (r.L.none) continue;
        const v = levelAt(r.L, T), out = outAt(r.L, T);
        const txt = out ? 'out' : (ahead ? '≈' : '') + Math.round(v) + '%';
        const tx = x > R.LW + (R.X(T1) - R.LW) * 0.85 ? x - 6 : x + 6;
        s += `<circle cx="${x}" cy="${r.Y(Math.min(100, v))}" r="3" fill="${out ? 'var(--neg)' : r.L.cap ? 'var(--iris)' : 'var(--ac)'}"/><text class="v" x="${tx}" y="${r.y0 + 13}" text-anchor="${tx < x ? 'end' : 'start'}" style="${out ? 'fill:var(--neg)' : ''}">${txt}</text>`;
      }
      g.innerHTML = s;
      host.setAttribute('aria-valuenow', Math.round((T - NOW) / H));
      host.setAttribute('aria-valuetext', ahead ? when(T) : 'now');
      if (!ahead) { h1.innerHTML = head0; basis.innerHTML = basis0; atEl.textContent = 'Drag the line, or focus it and use the arrow keys, to look ahead'; for (const k in P) P[k].textContent = ''; return; }
      const outs = R.rows.filter((r) => outAt(r.L, T)).map((r) => `${r.L.acct.label} ${r.L.label.toLowerCase()}`);
      const weekly = R.rows.filter((r) => r.L.label === 'Weekly' && !outAt(r.L, T)).map((r) => `${r.L.acct.label} ≈ ${Math.round(levelAt(r.L, T))}%`);
      h1.innerHTML = `By ${esc(when(T))}, ${outs.length ? `<span class="w">${esc(outs.join(' and '))}</span> ${outs.length > 1 ? 'are' : 'is'} out` : 'every limit still has room'}${weekly.length ? `; ${esc(weekly.join(', '))} of the week used` : ''}.`;
      basis.innerHTML = `<span>Estimated at today's pace, give or take ${Math.round(band(T) * 100)}%, ${dur(T - NOW)} from now.</span><span>Resets on the way are measured.</span>`;
      atEl.textContent = 'Looking ' + dur(T - NOW) + ' ahead. Press Home, or drag back, for now.';
      const a = act(NOW, T);
      P.tok.innerHTML = `By then <b>≈ ${fmt(kpi.tokens.v + tokRate * a)}</b> since today began`;
      P.cost.innerHTML = `By then <b>≈ ${money(kpi.cost.v + costRate * a, 0)}</b> at list prices`;
      P.hit.innerHTML = 'Stays near today’s rate';
      P.tps.innerHTML = `${outs.length} ${outs.length === 1 ? 'limit' : 'limits'} out by then`;
    };
    const setT = (t) => { T = Math.max(NOW, Math.min(T1 - 30 * 60e3, t)); draw(); };
    const hit = svg.querySelector('.hit');
    let dragging = false;
    const at2 = (e) => R.inv(local(host, e)[0]); // the svg fills its host, so host pixels are viewBox units
    hit.addEventListener('pointerdown', (e) => { dragging = true; hit.setPointerCapture(e.pointerId); setT(at2(e)); });
    hit.addEventListener('pointermove', (e) => { if (dragging) setT(at2(e)); });
    hit.addEventListener('pointerup', () => { dragging = false; });
    host.addEventListener('keydown', (e) => {
      const step = e.shiftKey ? DAY : H;
      if (e.key === 'ArrowRight') { setT(T + step); e.preventDefault(); } else if (e.key === 'ArrowLeft') { setT(T - step); e.preventDefault(); } else if (e.key === 'Home') { setT(NOW); e.preventDefault(); } else if (e.key === 'End') { setT(T1); e.preventDefault(); }
    });
    draw();

    // the only thing that moves on its own: the flow, as fast as its traffic
    let raf = 0, visible = true;
    const io = new IntersectionObserver((en) => { visible = en[0].isIntersecting; }, { threshold: 0.05 });
    io.observe(app.querySelector('.flow'));
    const fls = [...app.querySelectorAll('.flow .fl')].map((p) => ({ p, v: +p.dataset.v }));
    if (o.motion) {
      const t0 = performance.now();
      const loop = (n) => { if (visible) { const t = (n - t0) / 1000; fls.forEach((f) => { f.p.style.strokeDashoffset = (-t * 24 * f.v).toFixed(1); }); } raf = requestAnimationFrame(loop); };
      raf = requestAnimationFrame(loop);
      // one orchestrated moment: the line glides from now to the run-out and back, once, to show it can be moved
      setTimeout(() => { if (!app.isConnected) return; const target = outT + 2 * H, t0g = performance.now(); const glide = (n) => { const u = Math.min(1, (n - t0g) / 1600), e = u < 0.5 ? 2 * u * u : 1 - Math.pow(-2 * u + 2, 2) / 2; setT(NOW + (target - NOW) * e); if (u < 1) requestAnimationFrame(glide); else setTimeout(() => { const t1g = performance.now(); const back = (m) => { const v = Math.min(1, (m - t1g) / 900); setT(target - (target - NOW) * v * v); if (v < 1 && !dragging) requestAnimationFrame(back); }; if (!dragging) requestAnimationFrame(back); }, 1400); }; requestAnimationFrame(glide); }, 900);
    }
    return { destroy: () => { cancelAnimationFrame(raf); io.disconnect(); } };
  }

  HM.variations.C = { cls: 'vC', render };
})();
