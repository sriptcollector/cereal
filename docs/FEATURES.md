# Parity checklist: Cereal vs. Granola

Source: `toolbay-research/granola-report.pdf` (public information; Granola was not installed), and Granola's public
press-kit screenshots (compared side by side outside this repo).
✅ done · 🟡 partly · ⬜ not yet · ➖ out of scope (needs a hosted server, or a phone).

## Capture

| # | Feature | Granola | Cereal |
|---|---|---|---|
| 1 | Bot-free capture, mic + system audio | Mac + Windows | ✅ WASAPI loopback (Windows, tested); ScreenCaptureKit (Mac, CI-built, untested) |
| 2 | Me / Them labels | Yes | ✅ |
| 3 | Diarization of the other side | No on desktop | ✅ Better: Speaker 1, 2… (sherpa-onnx), rename once |
| 4 | Speaker names from the call app | Zoom/Teams/Meet | 🟡 From calendar attendees + rename; reading names off the call window ⬜ |
| 5 | Mic picker (Automatic) | Yes | ✅ |
| 6 | Echo handling on speakers | Mac only | ✅ Both |
| 7 | Capture only the meeting app | No | ⬜ |
| 8 | Live transcript: bubbles, copy, search | Yes | ✅ |
| 9 | Delete transcript chunks | Yes | ✅ |
| 10 | Edit transcript | No | ✅ Better |
| 11 | Pause / resume / resume later | Yes | ✅ |
| 12 | Auto-stop (call ended, 15 min silence) | Yes | ✅ (+ false-alarm cleanup) |
| 13 | Import audio | No | ✅ Better |
| 14 | Keep audio | Never | ✅ Keep / 30 days / delete after notes; in-app player ⬜ |
| 15 | Languages, notes language | 31+ | ✅ 99 + notes language setting |
| 16 | Custom vocabulary | 30 / 50 terms | ✅ Unlimited (speech prompt + notes) |
| 17 | Shared-screen snapshots | Mac | ⬜ |
| 18 | Works offline | No | ✅ |

## Detection and calendar

| # | Feature | Granola | Cereal |
|---|---|---|---|
| 19 | Google + Microsoft calendar | OAuth | ✅ Private iCal link (no sign-in, local) |
| 20 | Apple / ICS / CalDAV | No | ✅ Any iCal feed |
| 21 | Popup 1 min before (2+ people) | Yes | ✅ Join & take notes |
| 22 | Detect calls by mic use | Yes | ✅ Windows (incl. browser calls by tab title); Mac 14.2+ per-app |
| 23 | Link ad-hoc call to calendar event | 15 min | ✅ |
| 24 | Per-app notification toggles | Yes | ✅ Record / Ask / Ignore per app |
| 25 | Move window beside the call | Yes | ✅ |
| 26 | Floating indicator | Yes | ✅ With Pause and Stop |
| 27 | Tray / menu bar with next meeting | Mac opt-in | ✅ Tray shows next meeting |
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
| 35 | Switch template / regenerate, `/` in notes | Yes | ✅ |
| 36 | Rewrite / edit notes | Yes | ✅ Edit inline; rewrite via chat |
| 37 | Chat scopes | meeting, live, folder, selected, person, company, all | ✅ All of these |
| 38 | Chat citations | Yes | ✅ Numbered, clickable |
| 39 | Voice question | Yes | ✅ |
| 40 | Recipes `/`, custom | Yes (+share/remix) | ✅ 12 built-in + custom; sharing ➖ |
| 41 | Model choice | Paid | ✅ Free |
| 42 | Pre-meeting briefs | Paid | ✅ Free, from past notes with the same people |
| 43 | Follow-up email | Paid | ✅ Recipe + "Email the notes" |
| 44 | Chat actions (send email/Slack/calendar) | Yes | 🟡 Email draft (mailto), Slack via folder webhook |
| 45 | Yearly recap | Yes | ✅ `/year` recipe |
| — | File attachments in chat | Yes | ✅ |

## Organise and share

| # | Feature | Granola | Cereal |
|---|---|---|---|
| 46 | Home: Coming up + notes by day | Yes | ✅ |
| 47 | Spaces / folders | Spaces + 1 level | 🟡 My notes + folders (icons, description); nested folders ⬜ |
| 48 | Auto-file recurring meetings | Yes | ✅ Title rule per folder |
| 49 | People / Companies | Enriched online | ✅ From attendees (no outside data) |
| 50 | Search | Yes | ✅ Ctrl+K full-text |
| 51 | Trash, 30-day restore | Yes | ✅ |
| 52 | Share links | Hosted | ➖ |
| 53 | Copy as Markdown / email | Yes | ✅ |
| 54–55 | Slack / Notion / CRM / Zapier, folder auto-post | Partly paid | 🟡 Slack + any webhook (Zapier/Make/n8n) per folder; Notion/HubSpot direct ⬜ |
| 56 | Export | CSV by email, daily | ✅ Instant: MD, Word, PDF, all-notes CSV/JSON |
| 57–58 | API / MCP | Paid | ⬜ |
| — | Folder files as context | Yes | ✅ txt/md/csv/json/html/docx |
| — | Multi-select notes → ask | Yes | ✅ |

## Privacy and consent

| # | Feature | Granola | Cereal |
|---|---|---|---|
| 59 | Where audio goes | Cloud | ✅ Never leaves the computer |
| 60 | Transcript auto-delete periods | Yes | ✅ Same periods |
| 61 | Training on your data | Opt-out | ✅ Never; no telemetry |
| 62 | Consent message | Auto chat message | ✅ Reminder + copy message; first auto-record shows it |
| 63–64 | Camera watermark, consent page | Yes / Enterprise | ⬜ / ➖ |

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
