from pathlib import Path
from PIL import Image, ImageDraw
import json, html
p=Path(__file__).resolve().parent
shots=p/'screenshots';out=p/'review';out.mkdir(exist_ok=True)
scenarios=['overview','overview-lower','accounts','account-detail','account-sessions','account-projects','account-settings','add-account','first-run','first-run-accounts','empty','12-accounts','tray','tray-12']
for name in scenarios:
 sheet=Image.new('RGB',(1536,1374),'#0b151a');draw=ImageDraw.Draw(sheet)
 for row,width in enumerate([1280,1536,1920]):
  for col,theme in enumerate(['dark','light']):
   file=shots/f'{name}-{width}-{theme}.png'
   with Image.open(file) as im:
    assert im.size==(width,{1280:720,1536:864,1920:1080}[width]),file
    thumb=im.convert('RGB').resize((768,432),Image.Resampling.LANCZOS)
   x,y=col*768,row*458
   draw.text((x+10,y+6),f'{name} | {width} x {int(width*9/16)} | {theme}',fill='#eaf2f5')
   sheet.paste(thumb,(x,y+26))
 sheet.save(out/f'{name}.jpg',quality=92)
assert len(scenarios)*6==84
sections=''
for name in scenarios:
 links=' · '.join(f'<a href="screenshots/{name}-{width}-{theme}.png">{width} {theme}</a>' for width in [1280,1536,1920] for theme in ['dark','light'])
 sections+=f'<section><h2>{html.escape(name.replace("-"," ").title())}</h2><p>{links}</p><a href="review/{name}.jpg"><img loading="lazy" width="1536" height="1374" src="review/{name}.jpg" alt="{name}: three widths in dark and light" /></a></section>'
report='''<!doctype html><html lang="en" data-theme="dark"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Chrono-Runway · Round 2 phase 1 review</title><link rel="stylesheet" href="styles.css"><style>body{padding:32px;max-width:1600px;margin:auto}h1{font-size:28px;margin-bottom:12px}h2{font-size:20px;margin:24px 0 12px}p,li{font-size:14px;line-height:1.6;max-width:100ch}a{color:var(--teal)}section{margin-top:32px;border-top:1px solid var(--line-default)}img{display:block;width:100%;height:auto;border:1px solid var(--line-default);margin-top:12px}table{border-collapse:collapse;width:100%;font-size:13px;margin:16px 0}td,th{text-align:left;vertical-align:top;padding:10px;border-bottom:1px solid var(--line-default)}th{color:var(--ink-muted)}code{font-family:inherit;color:var(--teal)}</style><h1>Chrono-Runway · Phase 1 repaired</h1><p>The dispatch deck, Reset Horizon, separate capacity column, rail, Inter, official icons and Harness palette remain the selected design. <a href="index.html">Open the prototype</a> · <a href="design-language.md">Design language</a> · <a href="verification.json">Verification manifest</a>.</p><p>84 inspected Edge captures: 1280×720, 1536×864 and 1920×1080, dark and light. The studio sits outside the exported frame. 34 behavior assertions passed; zero console errors. Source SHA-256 values pin the captures. <a href="phase1-report.md">Defect-by-defect handoff and limitations</a>.</p><h2>What changed</h2><table><thead><tr><th>Open defect</th><th>Repair</th></tr></thead><tbody>
<tr><td>Wrong capability matrix and fixtures</td><td>API key: Claude/Codex/Pi. Routed: Claude/Pi. Unsupported OpenCode/Oh My Pi fixtures now use subscription; the routed cap sample uses Pi.</td></tr>
<tr><td>Ruler and gate label overlap; distorted intervals</td><td>Spaced day ticks, separate captions, exact interval fractions, and a labelled 0–5h short track. The legend names time until reset.</td></tr>
<tr><td>Clipped tray columns</td><td>Two bounded columns, wrapping account names, all 8/12 accounts, no horizontal overflow; clicks inspect the account.</td></tr>
<tr><td>Small text and crowded findings</td><td>11px minimum, shorter lead finding, named forecast windows, neutral secondary Starts, no Add keyboard legend, consistent rail SVGs.</td></tr>
<tr><td>Contradictory first-run data and counts</td><td>Fresh profiles clear usage, sessions, models and forecasts; no unread capacity recommendation; Setup counts and updates zero.</td></tr>
<tr><td>Hidden running sessions and disagreeing count</td><td>All 18 rows remain in the scroller. Initial rows include each computer; SAMYMSI shows four of four. Tool/window and computer names remain readable.</td></tr>
<tr><td>Unmodelled fold and whole-app replacement</td><td>Exact width/height pairs, container breakpoints, independent main scroll, keyed region patches, input/caret/scroll/dialog/text-selection/animation retention.</td></tr>
<tr><td>Missing extension contract/account page</td><td>Registered account page with Overview/Sessions/Projects/Settings, editable settings and safe previews. A/B script slots and id-based rail navigation are ready.</td></tr>
</tbody></table><h2>Tradeoffs and remaining work</h2><p>The 5-hour row has its own explicitly labelled scale, giving short resets a readable drawing. At 720px the deck and first lanes fit; the rest of the lanes and telemetry use main scroll. The tray preview uses 540px for readable names rather than asserting an unchanged OS popup size. These are documented design choices.</p><p>Phase 2 pages still use placeholders until Designers A and B register them. This remains an offline mock: no terminal launch, vendor sign-in/read, credential write or file deletion is claimed. Production Rust/Tauri build/test runners and macOS/WebKit acceptance were excluded because they execute the real app or write outside the delegated directory.</p>'''+sections+'</html>'
(p/'review.html').write_text(report,encoding='utf-8')
print('Built 14 contact sheets and a report linking all 84 original screenshots.')

