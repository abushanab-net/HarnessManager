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


/** The tool's own icon (LobeHub Icons, see icons/README.md). */
export const toolIcon = (h, s = 16) => `<img class="ticon" src="icons/${esc(h)}.svg" width="${s}" height="${s}" alt="">`;


const I = (d, s = 16) => `<svg width="${s}" height="${s}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;
export const ic = {
  play: I('<path d="m7 4 13 8-13 8z"/>', 13),
  dots: I('<circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/>', 16),
  edit: I('<path d="M4 20h4L19 9l-4-4L4 16zM14 6l4 4"/>', 15),
  pin: I('<path d="M12 17v5M9 3h6l-1 7 3 3H7l3-3z"/>', 15),
  tag: I('<path d="M20.6 13.4 13.4 20.6a2 2 0 0 1-2.8 0L3 13V3h10l7.6 7.6a2 2 0 0 1 0 2.8z"/><circle cx="7.5" cy="7.5" r="1.5"/>', 15),
  info: I('<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/>', 16),
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
