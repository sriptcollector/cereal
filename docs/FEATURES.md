# Parity checklist: Toolbay Notes vs. Granola

Source: `toolbay-research/granola-report.pdf` (public information only; Granola was not installed).
✅ done · 🟡 partly · ⬜ not yet · ➖ out of scope for v1. "Better" marks where Notes goes beyond Granola.

## Capture

| # | Feature | Granola | Toolbay Notes |
|---|---|---|---|
| 1 | Bot-free capture, mic + system audio | Mac + Windows | ✅ WASAPI loopback (Windows, tested), ScreenCaptureKit (macOS, CI-built, untested) |
| 2 | "Me" / "Them" labels from the two tracks | Yes | ✅ (uses your name if set) |
| 3 | Voice diarization of the other side | No on desktop | ✅ Better: sherpa-onnx (pyannote 3.0 + CAM++), "Speaker 1, 2…" |
| 4 | Speaker names from Zoom/Teams/Meet UI | Yes | 🟡 Rename a speaker once (applies everywhere); auto names from the call app ⬜ |
| 5 | Microphone picker | Yes | ✅ System default or a named mic; follows default-device changes mid-call |
| 6 | Echo handling on speakers | Mac only | ✅ Both: transcript-level echo removal |
| 7 | Capture only the meeting app's audio | No | ⬜ (per-process loopback) |
| 8 | Live transcript panel: bubbles, copy, search | Yes | ✅ |
| 9 | Delete transcript lines | Yes | ✅ |
| 10 | Edit transcript text | No | ✅ Better |
| 11 | Pause / resume / resume later in the same note | Yes | ✅ |
| 12 | Auto-stop (call ended, 15 min silence) | Yes | ✅ Call app releases mic (+20 s) or 15 silent minutes; sleep ⬜ |
| 13 | Import an audio file | No | 🟡 Headless only (`--process`); UI ⬜ |
| 14 | Keep audio for playback | Never stored | ✅ Keep / 30 days / delete after notes; in-app player ⬜ |
| 15 | Languages | 31+ | ✅ 99 (Whisper multilingual) with a language setting; summary language follows the meeting |
| 16 | Custom vocabulary | 30 / 50 terms | ⬜ |
| 17 | Shared-screen snapshots | Mac only | ⬜ |
| 18 | Works offline | No | ✅ Better: fully local |

## Detection and calendar

| # | Feature | Granola | Toolbay Notes |
|---|---|---|---|
| 19–20 | Calendar (Google, Microsoft, ICS) | Google + Microsoft | ⬜ (later, optional) |
| 21 | Popup 1 minute before calendar meetings | Yes | ⬜ needs calendar |
| 22 | Detect calls by microphone use | Yes | ✅ Windows: Zoom, Teams, Meet/Teams/Zoom/Whereby/Jitsi/Webex in the browser, Slack huddles, Discord, Webex, Skype, WhatsApp, Signal, Telegram, GoTo, RingCentral, Dialpad, Aircall. macOS: Zoom only ⬜ others |
| 23 | Link ad-hoc call to calendar event | Yes | ⬜ needs calendar |
| 24 | Per-app notification toggles | Yes | 🟡 Global toggle + "Not now" snoozes that app 10 min |
| 25 | Move the window beside the call | Yes | ⬜ |
| 26 | Floating live indicator | Yes | ✅ Pill on the screen edge, click to return |
| 27 | Tray / menu bar | Yes | ✅ Open, Start/Stop taking notes, Settings, Quit |
| 28 | Auto-start for all meetings | No | ⬜ (deliberately off: consent) |

## Notes and AI

| # | Feature | Granola | Toolbay Notes |
|---|---|---|---|
| 29 | Notepad during the meeting | Markdown, images, @mentions | 🟡 Plain-text/markdown notepad; images and @mentions ⬜ |
| 30 | Enhance after the call | Cloud LLM | ✅ Local LLM (Qwen3 4B/1.7B) or your own key; map-reduce for long meetings |
| 31 | Your text vs. AI text | Black / grey | ✅ White / grey (dark theme) + legend |
| 32 | Source view per point | Magnifying glass | ⬜ |
| 33 | Templates | ~29 + custom | ✅ 11 built-in (incl. 1:1, Sales call, Stand-up, Interview) + custom |
| 34 | Custom template as default | No | ✅ Better |
| 35 | Switch template / regenerate | Yes | ✅ |
| 36 | Edit notes | Yes | ✅ Edit as Markdown; tick action items |
| 37 | Chat: one meeting / all meetings | + folder, person, company | ✅ Meeting + all; folder/person scopes ⬜ |
| 38 | Chat citations | Yes | 🟡 Names the meeting and date; inline citations ⬜ |
| 39 | Voice question in chat | Yes | ⬜ (Toolbay Flow dictation works in the box) |
| 40 | Recipes with `/` | Yes | ✅ 9 built-in; custom recipes ⬜ |
| 41 | Model choice | Paid | ✅ Free: auto, manual, or own key |
| 42 | Pre-meeting briefs | Paid | ⬜ |
| 43 | Follow-up email | Paid, Gmail | ✅ `/email` recipe, copy |
| 44–45 | Chat actions, yearly recap | Yes | ⬜ |

## Organise and share

| # | Feature | Granola | Toolbay Notes |
|---|---|---|---|
| 46 | Home with notes by day | + Coming up | ✅ (Coming up needs calendar ⬜) |
| 47 | Folders | Spaces + folders | 🟡 Folders (one level) |
| 48 | Auto-file recurring meetings | Yes | ⬜ |
| 49 | People / Companies | Yes | ⬜ |
| 50 | Search | Yes | ✅ SQLite full-text over titles, notes and transcripts |
| 51 | Trash with restore | 30 days | ⬜ (delete asks to confirm) |
| 52 | Share links | Hosted | ➖ (no server by design) |
| 53 | Copy as Markdown | Yes | ✅ Notes and transcript |
| 54–55 | Slack / Notion / CRM / Zapier | Partly paid | ⬜ |
| 56 | Export | CSV by email | ✅ Better: instant Markdown file and PDF |
| 57–58 | API / MCP | Paid / limited | ⬜ |

## Privacy and consent

| # | Feature | Granola | Toolbay Notes |
|---|---|---|---|
| 59 | Where audio goes | Cloud, then deleted | ✅ Never leaves the computer |
| 60 | Auto-delete | Transcripts | 🟡 Audio retention options; transcript retention ⬜ |
| 61 | Training on your data | On by default | ✅ Never; no telemetry at all |
| 62 | Consent message | Auto chat message | ✅ Reminder + one-click copy of an editable message |
| 63–64 | Camera watermark, consent page | Yes / Enterprise | ⬜ |

## Platform and account

| # | Feature | Granola | Toolbay Notes |
|---|---|---|---|
| 65 | Windows + Mac | Yes | ✅ Windows tested; Mac configured and built by CI, not tested on a Mac |
| 66 | Mobile, web | Yes | ➖ |
| 67 | Account required | Yes | ✅ Better: none; optional Toolbay sync of settings/templates |
| 68 | Dark mode | Yes | ✅ Toolbay dark theme |
| 69 | Updates | Auto | ✅ Signed silent updates (standalone); the hub updates hub installs |
| 70 | Self-test | Network check | ✅ Audio test (mic + computer sound) in onboarding and Settings |
