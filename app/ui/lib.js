// Shared helpers: escaping, formatting, icons, charts and a small Markdown renderer.

export const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

export const fmt = (n) => {
  n = Number(n || 0);
  if (n >= 1e9) return (n / 1e9).toFixed(2) + 'B';
  if (n >= 1e6) return (n / 1e6).toFixed(2) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'k';
  return String(n);
};

export const hm = (ms) => new Date(ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
export const dayShort = (ms) => new Date(ms).toLocaleDateString([], { weekday: 'short' });
export const when = (ms) => {
  if (!ms) return '-';
  const d = new Date(ms), now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  if (sameDay) return 'Today ' + hm(ms);
  const y = new Date(now); y.setDate(now.getDate() - 1);
  if (d.toDateString() === y.toDateString()) return 'Yesterday ' + hm(ms);
  return d.toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' }) + ', ' + hm(ms);
};
export const dur = (ms) => {
  const m = Math.max(0, Math.round(ms / 60000));
  if (m >= 1440) return `${Math.floor(m / 1440)} d ${Math.floor((m % 1440) / 60)} h`;
  if (m >= 60) return `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, '0')} m`;
  return `${m} m`;
};
export const resetText = (ms, now) => {
  if (!ms) return 'no window open';
  if (ms <= now) return 'reset';
  const sameWeek = ms - now < 6 * 86400000;
  const d = new Date(ms);
  return (ms - now < 86400000 && d.getDate() === new Date(now).getDate()) ? `until ${hm(ms)}` : sameWeek ? `${dayShort(ms)} ${hm(ms)}` : d.toLocaleDateString();
};
export const lvl = (p) => (p >= 90 ? 'neg' : p >= 75 ? 'warn' : '');

export const initials = (name) => name.split(/[-_ ]/).filter(Boolean).slice(0, 2).map((s) => s[0].toUpperCase()).join('');
export const hue = (name) => [...name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7);
export const avatar = (name, sm = false) => `<span class="av${sm ? ' sm' : ''}">${esc(initials(name))}</span>`;

/** The tool's own icon (LobeHub Icons, see icons/README.md). */
/** Pages rows in place: those off the page get `.paged`. Returns the pager's HTML, or nothing when one page holds all;
 * the page and size buttons carry `<act>-page` and `<act>-per` actions. */
export function pager(rows, st, act) {
  const pages = Math.max(1, Math.ceil(rows.length / st.per));
  if (st.page >= pages) st.page = pages - 1;
  rows.forEach((tr, i) => tr.classList.toggle('paged', i < st.page * st.per || i >= (st.page + 1) * st.per));
  if (rows.length <= 25) return '';
  return `<button class="btn sm" data-act="${act}-page" data-v="${Math.max(0, st.page - 1)}" ${st.page === 0 ? 'disabled' : ''} aria-label="Previous page">‹</button>${Array.from({ length: pages }, (_, i) => `<button class="btn sm ${i === st.page ? 'on' : ''}" data-act="${act}-page" data-v="${i}" data-testid="${act}-page-${i}">${i + 1}</button>`).join('')}<button class="btn sm" data-act="${act}-page" data-v="${Math.min(pages - 1, st.page + 1)}" ${st.page === pages - 1 ? 'disabled' : ''} aria-label="Next page">›</button>
    <span class="help">${st.page * st.per + 1}–${Math.min(rows.length, (st.page + 1) * st.per)} of ${rows.length}</span><span class="seg3 mono">${[25, 50, 100].map((n) => `<a class="${st.per === n ? 'on' : ''}" data-act="${act}-per" data-v="${n}" data-testid="${act}-per-${n}">${n}</a>`).join('')}</span><span class="help">a page</span>`;
}
export const toolIcon = (h, s = 16) => `<img class="ticon" src="icons/${esc(h)}.svg" width="${s}" height="${s}" alt="">`;
export const toolPill = (h, label) => `<span class="tool ${esc(h)}">${toolIcon(h, 13)}${esc(label || h)}</span>`;

export const meter = (p, txt) => `<div class="meter"><div class="bar"><i class="${lvl(p)}" style="width:${Math.min(100, p)}%"></i></div><span><b>${Math.round(p)}%</b> · ${esc(txt)}</span></div>`;

const I = (d, s = 16) => `<svg width="${s}" height="${s}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;
export const ic = {
  home: I('<path d="M3 11 12 4l9 7M5 10v10h5v-6h4v6h5V10"/>'),
  dash: I('<path d="M4 13h6V4H4zM14 20h6v-9h-6zM14 4h6v4h-6zM4 20h6v-4H4z"/>'),
  users: I('<circle cx="9" cy="8" r="3.5"/><path d="M2.5 20c.8-3.5 3.3-5.5 6.5-5.5s5.7 2 6.5 5.5"/><path d="M16 4.8a3.5 3.5 0 0 1 0 6.4M18.5 14.8c1.7.8 2.7 2.6 3 5.2"/>'),
  chart: I('<path d="M4 20V10M10 20V4M16 20v-7M22 20H2"/>'),
  key: I('<circle cx="8" cy="15" r="4"/><path d="m11 12 9-9M17 6l3 3M14 9l2 2"/>'),
  term: I('<rect x="2.5" y="4" width="19" height="16" rx="2"/><path d="m7 10 3 2.5L7 15M12.5 15H17"/>'),
  plug: I('<path d="M9 2v5M15 2v5M6 7h12v4a6 6 0 0 1-12 0zM12 17v5"/>'),
  spark: I('<path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z"/>'),
  doc: I('<path d="M6 2h9l5 5v15H6z"/><path d="M14 2v6h6M9 13h8M9 17h6"/>'),
  route: I('<circle cx="6" cy="18" r="2.5"/><circle cx="18" cy="6" r="2.5"/><path d="M8.5 18H15a3 3 0 0 0 0-6H9a3 3 0 0 1 0-6h6.5"/>'),
  gear: I('<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>'),
  book: I('<path d="M4 4.5A2.5 2.5 0 0 1 6.5 2H20v17H6.5A2.5 2.5 0 0 0 4 21.5zM4 21.5A2.5 2.5 0 0 1 6.5 19H20"/>'),
  search: I('<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>', 15),
  play: I('<path d="m7 4 13 8-13 8z"/>', 13),
  plus: I('<path d="M12 5v14M5 12h14"/>', 14),
  folder: I('<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>', 14),
  refresh: I('<path d="M20 11a8 8 0 0 0-14.6-4.5L3 9M3 4v5h5M4 13a8 8 0 0 0 14.6 4.5L21 15M21 20v-5h-5"/>', 14),
  dots: I('<circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/>', 16),
  x: I('<path d="M6 6l12 12M18 6 6 18"/>', 14),
  back: I('<path d="M19 12H5M11 6l-6 6 6 6"/>', 18),
  prev: I('<path d="m15 6-6 6 6 6"/>', 16),
  next: I('<path d="m9 6 6 6-6 6"/>', 16),
  caret: I('<path d="m6 9 6 6 6-6"/>', 14),
  edit: I('<path d="M4 20h4L19 9l-4-4L4 16zM14 6l4 4"/>', 15),
  pin: I('<path d="M12 17v5M9 3h6l-1 7 3 3H7l3-3z"/>', 15),
  tag: I('<path d="M20.6 13.4 13.4 20.6a2 2 0 0 1-2.8 0L3 13V3h10l7.6 7.6a2 2 0 0 1 0 2.8z"/><circle cx="7.5" cy="7.5" r="1.5"/>', 15),
  eye: I('<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>', 15),
  download: I('<path d="M12 4v11M7 10l5 5 5-5M5 20h14"/>', 15),
  grid: I('<rect x="4" y="4" width="6.5" height="6.5" rx="1.2"/><rect x="13.5" y="4" width="6.5" height="6.5" rx="1.2"/><rect x="4" y="13.5" width="6.5" height="6.5" rx="1.2"/><rect x="13.5" y="13.5" width="6.5" height="6.5" rx="1.2"/>'),
  info: I('<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/>', 16),
  toEnd: I('<path d="M12 4v14M6 12l6 6 6-6M5 21h14"/>', 15),
  caretR: I('<path d="m9 6 6 6-6 6"/>', 14),
  panel: I('<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/>', 17),
  save: I('<path d="M12 4v11M7 10l5 5 5-5M5 20h14"/>', 17),
  find: I('<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>', 17),
};
/** The Harness Manager symbol: the Interlocking H (brand Concept B), drawn from the brand kit's master paths. */
export const brandMark = (s = 28) => `<svg width="${s}" height="${s}" viewBox="60 10 410 500" role="img" aria-label="Harness Manager"><defs>
<linearGradient id="hm-lu" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#006D79"/><stop offset="1" stop-color="#009C99"/></linearGradient>
<linearGradient id="hm-ll" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#393F91"/><stop offset="1" stop-color="#7F82E8"/></linearGradient>
<linearGradient id="hm-bc" x1="0" y1=".1" x2="1" y2=".8"><stop stop-color="#00616C"/><stop offset=".58" stop-color="#097D8C"/><stop offset="1" stop-color="#57CAC9"/></linearGradient>
<linearGradient id="hm-ru" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#6588DD"/><stop offset=".6" stop-color="#4455AF"/><stop offset="1" stop-color="#283875"/></linearGradient>
<linearGradient id="hm-rl" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#00A3A8"/><stop offset="1" stop-color="#00676D"/></linearGradient>
<linearGradient id="hm-fc" x1="0" y1="1" x2="1" y2="0"><stop stop-color="#65DBD8"/><stop offset=".4" stop-color="#36C8C5"/><stop offset=".76" stop-color="#078B98"/><stop offset="1" stop-color="#087680"/></linearGradient>
<linearGradient id="hm-rm" x1="0" y1="0" x2="1" y2="0"><stop stop-color="#B0E8E4"/><stop offset="1" stop-color="#5BCAD1"/></linearGradient></defs>
<path d="M84 259 L84 115 C84 90 96 77 121 63 L179 31 C194 23 207 25 207 44 L207 211 C173 229 122 255 84 280 Z" fill="url(#hm-lu)"/>
<path d="M188 292 C231 302 284 329 333 356 L333 425 C286 402 236 376 184 347 L153 330 Z" fill="url(#hm-bc)"/>
<path d="M84 342 L84 381 C84 402 96 417 113 428 L176 473 C196 487 207 482 207 465 L207 305 C170 320 119 340 84 342 Z" fill="url(#hm-ll)"/>
<path d="M333 41 C333 24 342 20 358 28 L418 62 C439 74 447 90 447 110 L447 192 C408 218 372 236 333 251 Z" fill="url(#hm-ru)"/>
<path d="M333 244 L447 190 L447 287 L333 344 Z" fill="url(#hm-rm)"/>
<path d="M333 340 L447 283 L447 413 C447 435 437 450 416 464 L365 494 C347 506 333 496 333 479 Z" fill="url(#hm-rl)"/>
<path d="M84 270 C84 242 106 229 149 208 L333 121 L333 244 L151 332 C111 353 91 368 84 393 Z" fill="url(#hm-fc)"/></svg>`;


/** Bars of values over day labels; the last bar (today) in the accent. */
export function bars(vals, labels, { w = 640, h = 170 } = {}) {
  const padL = 44, padB = 22, padT = 8, iw = w - padL - 4, ih = h - padB - padT;
  const max = Math.max(1, ...vals);
  const steps = [1, 2, 5];
  let step = 1;
  for (let e = 1; step * 4 < max; e *= 10) for (const s of steps) if (step * 4 < max) step = s * e;
  const top = Math.ceil(max / step) * step, bw = iw / vals.length;
  let s = `<svg width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" role="img" aria-label="Tokens per day">`;
  for (let v = 0; v <= top; v += step) {
    const y = padT + ih - (v / top) * ih;
    s += `<line x1="${padL}" x2="${w - 4}" y1="${y}" y2="${y}" style="stroke:var(--ru)"/><text x="${padL - 8}" y="${y + 4}" text-anchor="end" font-size="11" style="fill:var(--m)">${fmt(v)}</text>`;
  }
  vals.forEach((v, i) => {
    const bh = (v / top) * ih, x = padL + i * bw + bw * 0.22, today = i === vals.length - 1;
    s += `<rect x="${x}" y="${padT + ih - bh}" width="${bw * 0.56}" height="${Math.max(bh, v ? 1 : 0)}" rx="2" style="fill:${today ? 'var(--ac)' : 'var(--bar)'}"><title>${esc(labels[i])}: ${fmt(v)}</title></rect>`;
    s += `<text x="${x + bw * 0.28}" y="${h - 6}" text-anchor="middle" font-size="11" style="fill:${today ? 'var(--ac)' : 'var(--m)'}">${esc(labels[i])}</text>`;
  });
  return s + '</svg>';
}

/** "When limits free up": the next seven days, one row per account with limits. */
export function timeline(rows, now, w = 950) {
  // the design's lanes: a name column, then seven days from now with a shaded lane per account
  w = Math.max(560, w);
  const padL = 150, rowH = 28, top = 22, h = top + rows.length * rowH + 4, span = 7 * 86400000;
  const x = (t) => padL + Math.max(0, Math.min(1, (t - now) / span)) * (w - padL - 8);
  const day = (t) => new Date(t).toLocaleDateString([], { weekday: 'short' }) + ' ' + hm(t);
  let s = `<svg width="${w}" height="${h}" style="display:block;max-width:100%;height:auto;font-family:var(--sans)" viewBox="0 0 ${w} ${h}" role="img" aria-label="When each limit resets over the next seven days">`;
  const mid = new Date(now); mid.setHours(24, 0, 0, 0);
  s += `<text x="${padL + 6}" y="13" font-size="11" style="fill:var(--m);font-family:var(--mono)">${dayShort(now)} now</text>`;
  for (let i = 0; i < 7; i++) {
    const t = mid.getTime() + i * 86400000, xx = x(t);
    if (xx > w - 40) break;
    s += `<line x1="${xx}" x2="${xx}" y1="2" y2="${h}" style="stroke:var(--ru)"/>${xx - padL > 60 ? `<text x="${xx + 6}" y="13" font-size="11" style="fill:var(--m);font-family:var(--mono)">${dayShort(t)}</text>` : ''}`;
  }
  rows.forEach((r, i) => {
    const y = top + i * rowH, cy = y + 11;
    s += `<text x="0" y="${cy + 4}" font-size="13" style="fill:var(--t)">${esc(r.name)}</text>`;
    s += `<rect x="${padL}" y="${y}" width="${w - padL}" height="22" rx="4" style="fill:var(--sub)"/>`;
    const txt = (tx, str) => `<text x="${tx}" y="${cy + 4}" font-size="11" style="fill:var(--t2);font-family:var(--mono)">${esc(str)}</text>`;
    if (r.note) { s += txt(padL + 8, r.note); return; }
    const sess = r.session && r.session.resets_at > now ? r.session : null, week = r.week && r.week.resets_at > now ? r.week : null;
    let tx = padL + 8;
    if (sess) {
      const bw = Math.max(8, x(sess.resets_at) - x(now));
      s += `<rect x="${padL + 4}" y="${cy - 4}" width="${bw}" height="8" rx="4" style="fill:${sess.used >= 75 ? 'var(--warn)' : 'var(--t)'}"/>`;
      tx = padL + 4 + bw + 6;
    }
    const label = [sess ? '5 h → ' + hm(sess.resets_at) : '', week ? 'week resets ' + day(week.resets_at) : ''].filter(Boolean).join(' · ') || 'no window open';
    s += txt(tx, label);
    if (week) s += `<rect x="${x(week.resets_at) - 1}" y="${y + 1}" width="2" height="20" style="fill:var(--ac)"/>`;
    // where a limit runs out at the pace so far, when that is before its reset
    if (r.out && r.out > now) s += `<g data-testid="runs-out"><title>Runs out ${esc(day(r.out))} at this pace</title><rect x="${x(r.out) - 1}" y="${y + 1}" width="2" height="20" style="fill:var(--warn)"/></g>`;
  });
  return s + '</svg>';
}

/** Markdown subset for assistant answers: fences, headings, lists, tables, inline code, bold, italics, links. */
export function md(src) {
  const lines = String(src || '').replace(/\r/g, '').split('\n');
  let out = '', i = 0;
  // code spans are set aside first, so ** and * inside them stay literal
  const inline = (t) => {
    const codes = [];
    return esc(t)
      .replace(/`([^`]+)`/g, (_, c) => `\u0000${codes.push(c) - 1}\u0000`)
      .replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>')
      .replace(/(^|[\s(])\*([^*\n]+)\*/g, '$1<i>$2</i>')
      .replace(/\[([^\]]+)\]\((https?:[^)\s]+)\)/g, '<span title="$2">$1</span>')
      .replace(/\u0000(\d+)\u0000/g, (_, n) => `<code>${codes[+n]}</code>`);
  };
  while (i < lines.length) {
    const l = lines[i];
    if (/^```/.test(l)) {
      const buf = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i])) buf.push(lines[i++]);
      i++;
      out += `<pre><code>${esc(buf.join('\n'))}</code></pre>`;
      continue;
    }
    const hd = /^(#{1,6})\s+(.*)/.exec(l);
    if (hd) { out += `<h${Math.min(6, hd[1].length + 2)}>${inline(hd[2])}</h${Math.min(6, hd[1].length + 2)}>`; i++; continue; }
    if (/^\s*\|.*\|\s*$/.test(l) && i + 1 < lines.length && /^\s*\|[\s:|-]+\|\s*$/.test(lines[i + 1])) {
      const cells = (r) => r.trim().replace(/^\||\|$/g, '').split('|').map((c) => c.trim());
      out += '<table><tr>' + cells(l).map((c) => `<th>${inline(c)}</th>`).join('') + '</tr>';
      i += 2;
      while (i < lines.length && /^\s*\|.*\|\s*$/.test(lines[i])) out += '<tr>' + cells(lines[i++]).map((c) => `<td>${inline(c)}</td>`).join('') + '</tr>';
      out += '</table>';
      continue;
    }
    if (/^\s*([-*]|\d+\.)\s+/.test(l)) {
      const ordered = /^\s*\d+\./.test(l);
      out += ordered ? '<ol>' : '<ul>';
      while (i < lines.length && /^\s*([-*]|\d+\.)\s+/.test(lines[i])) out += `<li>${inline(lines[i++].replace(/^\s*([-*]|\d+\.)\s+/, ''))}</li>`;
      out += ordered ? '</ol>' : '</ul>';
      continue;
    }
    if (!l.trim()) { i++; continue; }
    const para = [];
    while (i < lines.length && lines[i].trim() && !/^(```|#{1,6}\s|\s*([-*]|\d+\.)\s+|\s*\|)/.test(lines[i])) para.push(lines[i++]);
    if (!para.length) para.push(lines[i++]);
    out += `<p>${inline(para.join('\n')).replace(/\n/g, '<br>')}</p>`;
  }
  return out;
}

export function diffHtml(diff) {
  return '<div class="diff">' + String(diff || '').split('\n').filter((l, i, a) => l || i < a.length - 1).map((l) =>
    `<div class="${l.startsWith('+') ? 'add' : l.startsWith('-') ? 'del' : ''}">${esc(l) || ' '}</div>`).join('') + '</div>';
}
