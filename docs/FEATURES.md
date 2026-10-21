# Parity checklist: Cereal vs. Granola

Source: `toolbay-research/granola-report.pdf`, Granola's public press-kit screenshots, and a static REA study of
Granola 7.668.1 for Windows (installed with Orion's OK, never launched; ledger kept outside this repo).
✅ done · 🟡 partly · ⬜ not yet · ➖ out of scope (needs a hosted server, or a phone).

## Capture

| # | Feature | Granola | Cereal |
|---|---|---|---|
| 1 | Bot-free capture, mic + system audio | Mac + Windows | ✅ WASAPI loopback (Windows, tested); ScreenCaptureKit (Mac, CI-built, untested) |
| 2 | Me / Them labels | Yes | ✅ |
| 3 | Diarization of the other side | No on desktop | ✅ Better: Speaker 1, 2… (sherpa-onnx), rename once |
| 4 | Speaker names from the call app | Zoom/Teams/Meet | 🟡 Suggested from attendees, 1:1s named in one click; reading names off the call window needs a real Zoom/Teams call to build against |
| 5 | Mic picker (Automatic) | Yes | ✅ |
| 6 | Echo handling on speakers | Mac only | ✅ Both |
| 7 | Follow the call's output device | Teams | ✅ Better: records whichever speakers/headset any call app plays to |
| 8 | Live transcript: bubbles, copy, search | Yes | ✅ |
| 9 | Delete transcript chunks | Yes | ✅ |
| 10 | Edit transcript | No | ✅ Better |
| 11 | Pause / resume / resume later | Yes | ✅ |
| 12 | Auto-stop (call ended, 15 min silence) | Yes | ✅ Same rules: 3 s after the app lets go, at the event's end, "Still there?" before the silence stop (+ false-alarm cleanup) |
| 13 | Import audio | No | ✅ Better |
| 14 | Keep audio | Never | ✅ Keep / 30 days / delete after notes; in-app player ⬜ |
| 15 | Languages, notes language | 31+ | ✅ 99 + notes language setting |
| 16 | Custom vocabulary | 30 / 50 terms | ✅ Unlimited (speech prompt + notes) |
| 17 | Shared-screen snapshots | Mac | ✅ Kept when the screen changes; add to notes or remove (off by default, like Granola) |
| 18 | Works offline | No | ✅ |

## Detection and calendar

| # | Feature | Granola | Cereal |
|---|---|---|---|
| 19 | Google + Microsoft calendar | OAuth | ✅ Private iCal link (no sign-in, local) |
| 20 | Apple / ICS / CalDAV | No | ✅ Any iCal feed |
| 21 | Popup 1 min before | Yes | ✅ 58 s before, hides 4 min in, skips declined/all-day; Join opens Zoom links in the Zoom app |
| 22 | Detect calls by mic use | Yes | ✅ Windows (incl. browser calls by tab title); Mac 14.2+ per-app |
| 23 | Link ad-hoc call to calendar event | 15 min | ✅ |
| 24 | Per-app notification toggles | Yes | ✅ Record / Ask / Ignore per app |
| 25 | Move window beside the call | Yes | ✅ |
| 26 | Floating indicator | Yes | ✅ Pause, Stop, screenshot into the note, live captions; on/off setting |
| 27 | Tray / menu bar with next meeting | Mac opt-in | ✅ Today's meetings, next one in the tooltip, Restart |
| 28 | Auto-record all meetings | No | ✅ Default (Orion), never silent |

## Notes and AI

| # | Feature | Granola | Cereal |
|---|---|---|---|
| 29 | Notepad: markdown, images, @mentions | Yes | ✅ TipTap editor, paste/drop images, @people |
| 30 | Enhance after the call | Cloud | ✅ Local (or your key); long meetings chunked |
| 31 | Your text dark, AI text grey | Yes | ✅ |
| 32 | Source view per point | Magnifier | ✅ Magnifier per line → transcript lines |
| 33 | Templates | ~29 | ✅ 26 + custom |
| 34 | Custom template as default | No | ✅ |
| 35 | Switch template / regenerate, `/` in notes | Yes | ✅ Grouped picker with Recent and search; each template's notes kept as a tab; rewrite asks first |
| 36 | Rewrite / edit notes | Yes | ✅ Edit inline; rewrite via chat |
| 37 | Chat scopes | meeting, live, folder, selected, person, company, all | ✅ All of these |
| 38 | Chat citations | Yes | ✅ Numbered, clickable |
| 39 | Voice question | Yes | ✅ |
| 40 | Recipes `/`, custom | Yes (+share/remix) | ✅ 12 built-in + custom; sharing ➖ |
| 41 | Model choice | Paid | ✅ Free |
| 42 | Pre-meeting briefs | Paid | ✅ Free, from past notes with the same people |
| 43 | Follow-up email | Paid | ✅ Recipe + "Email the notes" |
| 44 | Chat actions (send email/Slack/calendar) | Yes | ✅ Email draft, calendar event draft, post to a folder's Slack (asks first); suggested follow-up questions; live prompts |
| 45 | Yearly recap | Yes | ✅ `/year` recipe |
| — | File attachments in chat | Yes | ✅ |

## Organise and share

| # | Feature | Granola | Cereal |
|---|---|---|---|
| 46 | Home: Coming up + notes by day | Yes | ✅ |
| 47 | Spaces / folders | Spaces + 1 level | ✅ My notes + folders with one level of subfolders, stars, icons, description |
| 48 | Auto-file recurring meetings | Yes | ✅ Title rule per folder |
| 49 | People / Companies | Enriched online | ✅ From attendees (no outside data) |
| 50 | Search | Yes | ✅ Ctrl+K full-text, Ctrl+F in a note |
| 51 | Trash, 30-day restore | Yes | ✅ |
| 52 | Share links | Hosted | ➖ |
| 53 | Copy as Markdown / email | Yes | ✅ |
| 54–55 | Slack / Notion / CRM / Zapier, folder auto-post | Partly paid | 🟡 Slack + any webhook (Zapier/Make/n8n) per folder, Send to Notion, Log in HubSpot (your own tokens); Attio, Affinity, Salesforce not yet |
| 56 | Export | CSV by email, daily | ✅ Instant: MD, Word, PDF, all-notes CSV/JSON |
| 57–58 | API / MCP | Paid | ✅ Local MCP server (`cereal --mcp`) and CLI (`cereal notes …`), free and on this computer |
| — | Folder files as context | Yes | ✅ txt/md/csv/json/html/docx |
| — | Multi-select notes → ask | Yes | ✅ |

## Privacy and consent

| # | Feature | Granola | Cereal |
|---|---|---|---|
| 59 | Where audio goes | Cloud | ✅ Never leaves the computer |
| 60 | Transcript auto-delete periods | Yes | ✅ Same periods |
| 61 | Training on your data | Opt-out | ✅ Never; no telemetry |
| 62 | Consent message | Auto chat message | ✅ Reminder + copy message; first auto-record shows it |
| 63–64 | Camera watermark, consent page | Yes / Enterprise | ⬜ needs a signed virtual-camera component / ➖ |

## Platform and account

| # | Feature | Granola | Cereal |
|---|---|---|---|
| 65 | Windows + Mac | Yes | ✅ Windows tested; Mac built + signed by CI, untested |
| 66 | Mobile, web | Yes | ➖ |
| 67 | Account required | Yes | ✅ None; Toolbay sync optional |
| 68 | Dark mode | Yes | ✅ Light / dark / system |
| 69 | Updates | Auto | ✅ Signed silent updates; hub updates hub installs |
| 70 | Self-test | Network check | ✅ Audio test |
| — | Onboarding: role, calendar, permissions | Yes | ✅ Name, role (picks default template), calendar, audio test, auto-record choice + consent |
| — | Profile context for notes | Yes | ✅ Name, job title, about you |
| — | Feedback | In-app | ✅ To toolbay.ai |
| — | Sounds on start / pause / resume | Yes | ✅ Own chimes, can be turned off |
| — | Quick chat from anywhere | Alt+Space | ✅ Alt+Space |
| — | Deep links, jump list | granola://, Windows tasks | ✅ cereal://, Windows jump list |
| — | Keyboard shortcuts | Ctrl+K, J, N, F, [, comma, Shift+G | ✅ Same set |
| — | Company in profile | Yes | ✅ Name and description, used by the notes AI |
| — | Chat Enter key, events with no participants | Settings | ✅ |
| — | Mini window, hot corner | Yes | ✅ Mini window (on top), bottom-left hot corner (off by default) |

## Not matched yet, and why

| Feature | Why | What Cereal does instead |
|---|---|---|
| Speaker names read from the Zoom/Teams/Meet window | Granola reads the active speaker through Windows UI Automation. That has to be built and tested against the real Zoom and Teams apps during a call. | Names suggested from the attendees; the other person in a 1:1 is named in one click. |
| Recognising your own voice when everyone shares one mic | Needs voice enrolment (a speaker embedding of you) on top of the speaker models. Planned. | In calls, your mic is always you. In a room, everyone on the mic is labelled as you. |
| Pasting the consent message into the call chat | Granola uses UI Automation and keystrokes to type into Zoom or Teams for you. It can't be tested without those apps, and it posts on your behalf. | The message is one click to copy, shown when recording starts. |
| Camera watermark | Needs a signed virtual-camera driver (Windows) or camera extension (macOS, Apple Developer ID). | Consent reminder and message. |
| Google / Microsoft sign-in for calendars | Needs OAuth apps registered and verified by Google and Microsoft under Orion's accounts. | Private iCal link, which works with Google, Outlook, iCloud and any other calendar, with no sign-in. |
| Images sent to the notes AI | The local notes model reads text only. | Screens and images stay in the note next to the AI notes. |
| Browser extension for Meet | Has to be published on the Chrome Web Store. | Meet, Teams and Zoom in the browser are detected by tab title. |
| Attio, Affinity, Salesforce | Not built yet. | Webhooks (Zapier, Make, n8n) reach these. |
