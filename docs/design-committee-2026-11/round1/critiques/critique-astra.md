# Astra (Codex): critique, round 1

## Design A (Claude) — score 6/10

**What works**

- The computer board gives “running” and “where” a concrete shared structure: computer → project → account → session. It is the strongest information architecture here for locating work.
- The answer strip asks useful questions in ordinary language: “Running now,” “Runs out first,” and “Use next.” These are substantially better first-screen priorities than the baseline’s competing charts and readouts.
- Individual session cards open a Follow view. A user can choose the actual project of interest instead of navigating through a computer summary.
- Unknown limits have a distinct dashed treatment, and a computer last heard from 26 minutes ago carries that uncertainty into the board. Both support an honest operational view.
- The four-second update path patches existing board and ladder elements. That is a useful starting point for a vanilla-JS application receiving frequent updates.

**What fails**

- **Code-inferred, all widths/themes:** the account window renderer substitutes the estimated run-out time for the reset time when a forecast exists. Accounts and the Follow drawer therefore conceal the reset precisely when comparing exhaustion against replenishment matters most.
- **Code-inferred, all widths/themes:** the answer strip is built separately from the periodic updates. Its “resets in” countdown stays fixed while the ladder changes. The most prominent answer can become less current than its supporting detail.
- **Code-inferred, all widths/themes:** “Runs out first” searches forecast values without an explicit exhausted-now branch. The fixtures do not demonstrate a zero-remaining account. The design establishes future pressure better than the owner’s immediate question, “which account is exhausted?”
- **Code-inferred, all widths/themes:** completing Add account closes the flow and produces a toast without adding the entered account. From first run, it reveals the canned populated dataset. The transition cannot demonstrate that the user’s new account exists or what remains unknown about it.
- **Code-inferred, all widths/themes:** the Overview’s reply table includes a Speed column and `t/s`. This directly violates the brief’s Overview restriction and spends attention on a measurement the owner explicitly excluded.
- **Code-inferred, 1280, both themes:** the shell hides its freshness label. Meanwhile, the narrow answer strip repeats the long recommended account name in its heading and Start button. The code removes evidence of freshness while retaining repetition; rendered clipping is unverified.
- **Code-inferred, all widths/themes:** the custom modal lacks complete focus containment and restoration. Keyboard users can leave the dialog for the underlying interface, weakening an otherwise straightforward account flow.

**Must change before it could win**

- Keep the computer board, but derive the top answers and supporting values from the same current snapshot. Explicitly distinguish exhausted now, forecast exhaustion, reset, and unknown.
- Show both reset and forecast where relevant; remove Overview throughput; retain freshness at 1280 without duplicating account names.
- Complete Add account and keyboard behavior, then demonstrate all required widths and themes with exhausted, unknown, and twelve-account cases.

## Design B (Codex) — score 7/10

**What works**

- In the supplied 1280, 1536, and 1920 screenshots, light and dark, the capacity ledger is the most composed surface of the three. Account identity, window availability, and state occupy consistent positions. Comparison is materially easier than in the baseline.
- The supplied eight-account Overview screenshots keep all eight accounts in the initial view, including at 1280. Exhausted capacity and unread limits look different; a missing measurement is not drawn as zero.
- “Use now” and “Resets first” separate two legitimate decisions. The reset recommendation shows the particular window’s remaining room alongside weekly capacity, avoiding the suggestion that a five-hour reset replenishes everything.
- The Add account screenshots have a clear tool → account kind → fields progression in both themes. The source supplies validation, native-dialog behavior, and focus restoration. Newly added accounts begin with unknown limits rather than invented availability.
- Folding the daily chart and model breakdown is the right structural move for Accounts. The first-run screenshots also give a clear next action without filling an empty installation with fake activity.

**What fails**

- **Observed, especially 1280 dark and light:** the ledger’s small reset and state labels require close reading. The source sets some labels at eight pixels. The bars are quickly comparable, but the consequential answer—when a constrained account becomes usable—is not equally legible. At 1920 there is room to improve this without losing density.
- **Observed, all supplied Overview widths/themes:** “Room to keep going.” receives more typographic emphasis than the actual exception. With an exhausted account present, that headline is reassuring but weakly informative. The owner wants “I got my answer”; the headline should deliver one.
- **Observed in the ledger, confirmed by code, all widths/themes:** the exhausted Codex account also shows a future run-out estimate without naming that estimate’s window in the column. Zero five-hour capacity and a weekly forecast can both be correct, but the display makes the user reconcile the apparent contradiction.
- **Code-inferred, all widths/themes:** a Running now computer card lists several projects, but its Follow action targets only the first session. Seeing the desired project does not mean the visible Follow action follows that project. Another navigation step is needed to choose precisely.
- **Observed, Accounts at 1280 in both themes:** despite the collapsed chart, the large heading, supporting statement, toolbar, and chart header delay the first account row until well down the screen. Only two complete account rows fit before the next is cut by the viewport. The Overview’s discipline has not carried through to account administration.
- **Code-inferred, all widths/themes:** some tiny trend graphics are generated from seeds or account identity rather than the corresponding usage series. They look like instruments without faithfully encoding those instruments’ data. Decorative “live” polish damages trust here.
- **Code-inferred, all widths/themes:** Data tick changes freshness rather than exercising a real data redraw. Full rendering replaces the main content and restores scroll, but not focused controls. The prototype therefore does not substantiate stable interaction under production polling.

**Must change before it could win**

- Promote an explicit current exception into the headline; enlarge reset/state information and name the window behind every forecast.
- Make each running project directly followable, and compress the Accounts introduction so administration starts near the top.
- Bind every chart to its actual series and demonstrate repeated data updates while preserving selection, keyboard focus, open dialogs, and scroll.

## Design C (OpenCode) — score 4/10

**What works**

- The reset horizon attempts the right causal comparison: capacity available now, expected depletion, and the next replenishment. That is more useful than quota percentages alone.
- Separating “most room” from capacity worth using before a reset acknowledges that the account with the largest balance is not always the best immediate choice.
- Computer filters, per-account inspection, and a tray-limit preview recognize the application’s operational breadth. These are worthwhile pieces to retain.
- The source supplies theme tokens and a motion-off path. The concept does not require a framework or a separate rendering engine.

**What fails**

- **Code-inferred, 1280/1536/1920, both themes:** the horizon is not a trustworthy time instrument. Five-hour and weekly gates are remapped and clamped differently, while ruler labels use separately fixed positions. A five-hour reset can land beyond the ruler’s “+5h” tick; weekly gates follow another scale. Forecast gaps are artificially expanded. Users cannot safely compare positions, although the graphic invites them to.
- **Code-inferred, all widths/themes:** capacity and time share the same bar geometry: remaining percentage determines fill length relative to a reset position. The result requires interpretation before answering either “how much?” or “when?” Headings, chips, rings, gates, and inspector explanations then repeat the story. This recreates “crowded and disorganised” with a more elaborate vocabulary.
- **Code-inferred, all widths/themes:** prominent recommendations and summaries are hardcoded rather than derived consistently. “Expiring soonest” points to a later reset than another account’s five-hour reset. In the twelve-account fixture, an account with 1% left is described as exhausted. These are different operational states.
- **Code-inferred, all widths/themes:** the default running list takes the first three sessions, which are local. Remote projects disappear from the initial answer, while computer filters rely on colored dots and counts. The user gets a total for “elsewhere” without immediately learning what is running there.
- **Code-inferred, all widths/themes:** every Add-field keystroke calls `updateField()`, which rebuilds `root.innerHTML`. The focused input is removed without focus restoration. Simulated redraw follows the same full replacement and claims scroll preservation without implementing it. Retaining filter values does not preserve interaction or animation continuity.
- **Code-inferred, all widths/themes:** Add account immediately labels a new subscription as measured with 100% availability, fresh reset times, and forecast values. From an empty installation it also switches to the populated base preset. This manufactures answers before any account reading has occurred.
- **Code-inferred, all widths/themes:** several required actions stop at a success message: Follow does not open a continuing session view, removal does not remove the account, and CSV export produces no file. Clickable account rows also lack complete keyboard activation. The breadth of the presentation exceeds the completeness of its behavior.

**Must change before it could win**

- Replace the horizon with one mathematically consistent, explicitly labeled scale—or separate capacity and time columns. Remove positional adjustments that falsify comparisons.
- Reduce the first screen to derived operational answers and directly selectable local and remote sessions. Preserve unknown states after account creation.
- Replace full-root interaction rendering with stable updates, complete the core actions, and verify keyboard, form, scroll, and motion continuity.

## Across the three

- **The one thing a winning design here must have:** a readable first-glance answer linking constrained accounts and running projects to their computers, current windows, resets, and next useful actions.
- **The one thing it must not have:** decorative or stale precision that makes a forecast, quota, freshness claim, or successful action look more certain than it is.
- **Parts worth combining:** A’s computer-and-project session board.
- **Parts worth combining:** B’s aligned capacity ledger and explicit unknown states.
- **Parts worth combining:** B’s tool-first Add account flow.
- **Parts worth combining:** C’s distinction between choosing maximum room and using capacity before a reset, recalculated from actual data.
- **Ranking: B > A > C.** B is the strongest base for another round; A contributes the clearest running/where structure; C needs its instruments and interaction model rebuilt.
  A and C were judged from code only; B from code and supplied screenshots. No browser, live-motion, or WebView2 checks were performed; rendered behavior and update stability remain unverified.