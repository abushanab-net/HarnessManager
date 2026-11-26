# Calibre widget studies — Designer B

Open [the gallery](widgets.html) and select a family. Enamel and Night show the same reading side by side on desktop and stack on mobile. The source switch compares a fresh snapshot with a later offline reading. Android also has size and colour controls.

## Choosing a face

| Family | B1 | B2 |
| --- | --- | --- |
| iOS small | A power-reserve hand shows the recommended account’s weekly room. The account and its Saturday loss sit below it. | A harvest receipt shows +120 XP earned across three of four eligible resets. |
| iOS medium | A pinned account shows its five-hour and weekly room on the same scale. | Two accounts are ordered by their next five-hour reset. |
| iOS large | The next account leads. Short bars compare weekly reserves. An unknown reading gets a dash. | Three dated harvests explain where the 120 XP came from. |
| Lock Screen circular | A 270-degree gauge fills to 93%. The open sector separates the ends even near full capacity. | Three of four marked segments count completed harvests. |
| Lock Screen rectangular | The pinned account’s local reset time leads, with room and reading age below. | The waiting count leads. After contact is lost, the face reports that the phone window ended. |
| Lock Screen inline | The pinned account’s 5-hour room and age fit beside the date. | Earned XP and its reading age use the same space. |
| Lock Screen Live Activity | The time left leads, with a deadline and the line “If not answered, it waits in the terminal.” | A short timeline shows the transition from Working to Waiting for you, or Lost contact. |
| Dynamic Island compact | A bell pairs with the remaining answer time. | A Calibre watch hand pairs with the waiting count. On loss of contact the hand pauses and the count becomes unknown. |
| Dynamic Island minimal | The bell’s outer arc tracks the response window. | A watch hand identifies the waiting session and changes to a paused hand on loss of contact. |
| Dynamic Island expanded | The request, remaining time and local deadline explain when to open the app. | The session timeline, reading age and waiting count explain why to open the app. |
| Android resizable | 2×2 shows the account and its weekly loss. 4×2 makes room for the five-hour reading. 4×3 adds its reset and a second account’s weekly room. | 2×2 shows the harvest total. 4×2 adds the XP per harvest. 4×3 adds the dates and eligible-window count. |
| Android ongoing notification | A private permission request leads with the remaining answer time. | The last session transition or loss of contact leads. Both expand to Open and Stop following. |

## Reading the numbers

At the 09:41 snapshot, **claude-Resal-Dev01** has 64% of its week left before Saturday’s reset, the soonest large loss identified by the app. Every Use next face gives the same reason, **64% erased Sat**. Its five-hour room is 52%. Separately, the pinned **sabushanab-resal** has 93% five-hour room and 29% weekly room. Its five-hour reset is 11:29. In the large register, 75% and 11% have proportional bars. The dash for opencode-main means its weekly reading is unavailable.

The 25 November 2026 snapshot supplies the account readings. Session times and harvest receipts are review fixtures. Each +40 XP receipt represents planned work that used room before a reset. The +120 XP belongs to the week ending 22 November. A receipt’s reading age describes when the record was refreshed, independently of the dates the work occurred.

Reading ages are relative throughout. At 09:41, capacity is **8m ago** and the session is **just now**. At 10:46 offline, the same observations are **Stale · 73m ago** and **Stale · 65m ago**. Stale starts after 60 minutes. Colour-capable faces add amber, and monochrome accessories keep the word *Stale* within the system tint. Compact and minimal Island specimens put the age in the review caption and accessible description to preserve the camera region. Clock times elsewhere denote a reset or answer deadline.

## Platform handoff

- **WidgetKit.** Use systemSmall, systemMedium and systemLarge timeline entries. The review frames are 158×158, 338×158 and 338×354 points. Schedule entries at stale and reset boundaries and reload on new measurements. At reset, expire the old recommendation until the owner computer supplies a new reading. [WidgetKit timelines](https://developer.apple.com/documentation/widgetkit/keeping-a-widget-up-to-date)
- **Lock Screen.** Use an accessoryCircular gauge for the reserve arc. Allow the host to supply vibrant rendering and tint. Protect private session details until unlock. The gallery photo is *Fronalpstock big* by Hannes Röst, cropped for the accessory trays and overlaid with day and night tints. The photograph and its displayed treatments are licensed under CC BY-SA 3.0. [Original photograph](https://commons.wikimedia.org/wiki/File:Fronalpstock_big.jpg) · [Licence](https://creativecommons.org/licenses/by-sa/3.0/)
- **ActivityKit.** The permission fixture has 2:46 remaining in its original five-minute phone response window at 09:41. Drive a native countdown from absolute expiry. After expiry, the terminal can still accept the answer. A disconnected phone cannot infer an answer or the current waiting count. Update on authoritative revisions and end when the desktop confirms completion. [Live Activity guidance](https://developer.apple.com/documentation/activitykit/displaying-live-data-with-live-activities)
- **Android App Widgets.** Select the layout from host dp bounds with Glance SizeMode.Responsive. Cell spans are illustrative. Resolve the background radius from the system widget dimension on supported launchers. The preview uses a 24dp radius fallback and 16dp internal padding. The Material You switch previews a blue dynamic palette in both appearances. Resolve actual device colours from the host. [Android widget guidance](https://developer.android.com/develop/ui/views/appwidgets/enhance) · [Glance layouts](https://developer.android.com/develop/ui/compose/glance/build-ui)
- **Android notifications.** Use system chrome for the app icon, app name, relative age and expansion chevron. The expanded Open action routes through unlock. Stop following ends phone updates for this session and lets the desktop session continue. The gallery simulates these actions locally. [Notification guidance](https://developer.android.com/develop/ui/views/notifications)

## Review evidence

Run `node docs/mobile-2026-11/design-b/widgets/review.mjs` from the repository root. It renders the 56 theme and size combinations in both source states, exercises the Android controls and writes the screenshot plates. The screenshot review caught a wrapped XP total in the 2×2 Android layout and a crowded stale label on the rectangular accessory. Both were corrected before the final capture. Physical-device rendering, system palette selection and OS refresh delivery still need native verification.

[Collection](shots/collection.png) · [Home Screen](shots/home.png) · [Lock Screen accessories](shots/accessories.png) · [Live Activities](shots/activities.png) · [Android sizing](shots/android.png) · [Android Calibre palette](shots/android-calibre.png) · [Expanded notifications](shots/notification-expanded.png) · [Offline](shots/offline.png)
