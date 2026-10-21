<p align="center"><img src="app-icon.png" width="88" alt="Toolbay Notes"></p>

<h1 align="center">Toolbay Notes</h1>

<p align="center"><b>Free, local, open-source AI meeting notes. No bot joins your call.</b><br>
Jot a few words during the meeting. When it ends, the AI on your computer turns them and the transcript<br>
into clean notes with decisions and action items. No account. No limits. Nothing leaves your computer.</p>

<p align="center"><sub>Inspired by <img src="src/assets/inspired-by/granola.svg" height="12" alt="Granola"> Granola<br>
Toolbay Notes is an independent open-source project and is not affiliated with or endorsed by Granola.</sub></p>

---

Toolbay Notes is the second free tool in the [Toolbay](https://toolbay.ai) collection: open-source rebuilds of paid
software. It records **your microphone** ("You") and **your computer's sound** ("everyone else") as two separate
tracks, so it works with Zoom, Google Meet, Microsoft Teams, Slack huddles, Discord, Webex or a phone app, without
inviting anything into the call. Speech recognition ([whisper.cpp](https://github.com/ggml-org/whisper.cpp)),
speaker separation ([sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) with pyannote segmentation) and the notes
AI ([llama.cpp](https://github.com/ggml-org/llama.cpp) with an open Qwen model) all run on your own hardware.

<p align="center"><img src="docs/screenshots/meeting.png" width="760" alt="Toolbay Notes meeting page with AI notes and transcript"></p>

Written from scratch (Tauri 2 + Rust + React). No code, assets or services from any commercial product are used.

## Install

**Windows 10/11:** download `Toolbay.Notes_x.y.z_x64-setup.exe` from [Releases](https://github.com/sriptcollector/toolbay-notes/releases/latest)
and double-click it. No admin prompt and nothing else to install: the speech, speaker and notes engines and a starter
speech model are inside the installer. Notes picks the best AI models for your computer by itself and fetches them in
the background (a gaming GPU gets Whisper Large v3 Turbo and Qwen3 4B; an ordinary laptop gets compact models).

**macOS 13+ (Apple Silicon or Intel):** download the `.dmg` from Releases. Allow Microphone and "Screen & System
Audio Recording" when asked (Notes only uses the sound, never your screen). The Mac build is produced by CI and
has not been tested on a real Mac yet.

**Through the Toolbay app:** open Toolbay, find *Notes*, click Install. Toolbay reads [`toolbay.json`](toolbay.json),
installs Notes and its notes AI model in one step, and opens it.

## How it works

1. A call starts (any app that uses your microphone). A small popup asks **Take notes?** Nothing records until you click.
   Or click **New note** yourself.
2. Toolbay Notes reminds you to tell the others and gives you a message to paste in the meeting chat
   (in many places everyone must agree to be recorded).
3. Type rough notes as the call goes. The live transcript fills in on the right: **You**, and the other side.
4. When the call ends (Notes stops by itself 20 s after the call app releases the microphone), it re-transcribes the
   whole call at full quality, tells the other speakers apart (Speaker 1, 2, …; click to name them), removes the echo
   of the other side from your mic if you used speakers, and writes your notes with the template you picked.
   Your own points show in white, what the AI added in grey.
5. Ask questions about one meeting or all of them (`Ctrl+J`), search everything (`Ctrl+K`), copy as Markdown,
   save as a Markdown file or PDF.

Measured on a desktop with an RTX 4070 Ti (12 GB) and 32 threads, for a **44-minute, 4-person meeting**:
final transcript with speakers **45 s** after the call, notes **21 s** later: about **66 s** in total.

## Features

- **Two-track capture, no bot:** microphone + system audio (WASAPI loopback on Windows, ScreenCaptureKit on macOS),
  kept on one clock, follows device changes (plug in headphones mid-call), never mutes or ducks other audio.
- **Live transcript** while you talk (chunks cut at natural pauses), then a **full-quality pass** after the call.
- **Speaker separation:** "You" vs. the other side from the two tracks, plus local diarization of the other people;
  rename speakers once, everywhere.
- **Echo removal** for people without headphones.
- **AI notes from your notes:** keeps every point you typed, adds the rest from the transcript, never invents names or
  numbers. Long meetings are condensed part by part first, so a small local model handles 2-hour calls.
- **Templates:** Auto, 1:1, Sales call, Stand-up, Interview, Team meeting, User interview, Project kick-off,
  Investor pitch, Brainstorm, Lecture, plus your own. Switch and rewrite any time.
- **Chat with your notes** for one meeting or across all, with `/` recipes (TL;DR, action items by owner,
  follow-up email, decisions, "what did I miss?", my week, my open to-dos).
- **Search** across titles, notes and transcripts (SQLite full-text).
- **Export:** copy notes or transcript, Markdown file, PDF.
- **Meeting detection** for Zoom, Teams, Google Meet (and other browser calls), Slack huddles, Discord, Webex, Skype,
  WhatsApp and more; auto-stop when the call ends or after 15 silent minutes.
- **Consent:** reminder each time recording starts, editable "let others know" message.
- **Folders**, a tray icon, a small floating pill while recording, resume recording into the same note.
- **Automatic model choice** for your hardware; optional cloud engines with your own key (OpenAI-compatible speech,
  OpenAI-compatible or Anthropic notes).
- **Toolbay account sync** for settings and custom templates only. Recordings, transcripts and notes stay local.
- No telemetry, no analytics, no account required.

The full checklist against Granola is in [docs/FEATURES.md](docs/FEATURES.md).

## Privacy

Everything is stored in your app data folder (`%APPDATA%\ai.toolbay.notes` on Windows,
`~/Library/Application Support/ai.toolbay.notes` on Mac): audio, transcripts, notes and the search index. The only
network use is downloading AI models (Hugging Face, GitHub), Toolbay sync of settings/templates when you're signed in
to the Toolbay app, and a cloud engine only if you add your own API key.

## Build from source

```bash
npm install
npm run tauri dev
```

`npm run tauri build` makes the installer (it downloads the bundled engines and models first). Tests:
`cargo test --workspace`. See [AGENTS.md](AGENTS.md) for the code map and headless test modes.

## License

MIT. whisper.cpp, llama.cpp and sherpa-onnx are MIT/Apache-2.0; Whisper models (MIT), Qwen3 (Apache-2.0),
pyannote segmentation 3.0 (MIT), 3D-Speaker CAM++ (Apache-2.0).
