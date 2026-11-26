# Split Time · Designer B

## Three candidates considered before drawing

| Direction | How the owner reads it | Trade-off |
| --- | --- | --- |
| Split Time | A two-lane timing instrument puts remaining room beside the reset that closes the window. | Gauges take space, so reserve the largest one for the recommended account. |
| Switchboard | A routing diagram links accounts, computers and waiting sessions. | Machine relationships are clear, but tracing a route slows a quick capacity check. |
| Field Log | A chronological notebook follows the owner from reading to request to receipt. | Strong audit trail, but the account with room can slip below older entries. |

I chose **Split Time**. Two accounts can have the same room left and very different reset times. A filled lane always means room remaining, and the adjacent clock gives the owner a reason to choose. Coral calls out usable capacity. Blue identifies a waiting decision. The same readings sit on graphite at night and on paper-grey by day.

## Scope and data

The approved November 25 wireframes supply the structure. The companion data snapshot supplies the account, window, session, project, host and usage figures. The recommendation uses the earliest reported five-hour reset among accounts with room. The fixture clock advances from the snapshot when the review opens. The command, request deadlines, attribution to a computer and game history are demonstration fixtures. Approval confirmation simulates the native authentication handoff.

The account forecast describes the result if usage stays flat. A single limits reading cannot establish a burn rate. The streak counts recorded work and treats missing sync days as unknown. XP comes from using expiring room for planned work. Spending and answering requests earn no points.

## Implementation and review questions

Map surfaces, text and accent roles to Flutter ColorScheme and a small ThemeExtension for capacity and requests. Draw the segmented gauge with CustomPainter and expose one Semantics value. Use tabular figures for time, a scrollable body under fixed navigation, and MediaQuery.disableAnimations for the static state. Drive deadlines from the request's absolute expiry and recheck on resume.

Test whether someone can name the recommended account and explain the reset in two seconds, distinguish remaining from consumed room, find the command's host before answering, and explain what earned their most recent XP.

## Checkpoint 1 · Revision 1

Each screen has two independent, interactive 390 × 844 frames. Compare dark and light directly, or swipe the pair horizontally on a narrow display. The **Full-length static artboards** disclosure under each pair shows the complete initial screen at natural height, including the recap and bottom actions. The static copies keep the opening readings while the interactive clocks advance.

| Review point | Design decision |
| --- | --- |
| Theme comparison | Each pair starts from the same data. Actions change only the chosen theme so the other side can remain a before-state. |
| Design system | The Instrument kit specifies type faces, sizes, weights and line heights, spacing, radii, elevation, colours and five motion tokens. Its live specimens cover decision buttons, chips, a session row, request card, sheet, tabs, period selector and reminder switch. |
| Badges | The coral timing chip records the first harvest. The numbered race bib marks seven recorded days. A dashed lap marker shows a locked four-harvest streak. Select any badge to see its rule and progress. |
| Harvest & level | Home carries a split-flap level counter beside the streak. Use **Preview harvest +40 XP** on Account detail to simulate completed planned work using expiring room. Split 03 flips to 04, XP counts from 1,240 to 1,280, and the consecutive-harvest badge advances from 2/4 to 3/4. Each theme awards once. The motion lab replays without affecting either account. |
| Account chart | All seven dates label their bars. Sun 22 is the peak with 3.14 M tokens. Wed 25 is today with 164,606. The forecast reads “≈ 93% will be unused at the reset if nothing else runs.” |
| Permission | The last assistant message explains which change the tests check. A remaining-time bar is calibrated from 0:00 to 5:00. Expiry also closes an open confirmation. |
| Coral contrast | Browser-computed small text was #27221E on #F9A17E for both the light Home recommendation and Progress recap. Relative luminance gave 7.82:1. WCAG AA for normal-size text requires 4.5:1. |
| Cover | A two-register reset clock reads 01 hours and 48 minutes. A finish-line check and the capacity caption connect the timing equipment to the account. |
| Full-scroll review | The static pairs and PNGs include the footer after all content. The interactive phones retain their own scroll and pinned actions. |

### Reproduce the review

Open `index.html` directly from disk. From the repository root, run `node docs/mobile-2026-11/design-b/review.mjs` to check the prototype with the installed Playwright and Edge and regenerate the screenshots. `shots/*-full.png` contains the four paired artboards. `shots/dark.png` and `shots/light.png` show the fixed-height frames. The cover, kit and completed harvest also have individual captures.

During screenshot review I tightened the permission layout so Claude’s last line fits above the decision buttons, widened the level register to keep its digits together, and increased the capture viewport to include the entire Progress tab bar. Checks cover both themes, static artboard height, permission confirmation and expiry, single-award harvest and XP propagation, badge progress, widget reordering, recap export, reminders, reduced motion and layout at 100%, 150% and 200% text. Biometric confirmation and harvest history are prototype fixtures. A device implementation is needed to validate haptics, native Face ID and background reminders.
