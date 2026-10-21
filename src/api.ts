import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Provider = "local" | "open_ai" | "anthropic";
export interface CloudCfg {
  base_url: string;
  api_key: string;
  model: string;
}

export interface Settings {
  onboarded: boolean;
  name: string;
  language: string;
  microphone: string | null;
  capture_system: boolean;
  detect_meetings: boolean;
  auto_stop: boolean;
  consent_reminder: boolean;
  consent_message: string;
  default_template: string;
  keep_audio: "keep" | "30d" | "delete";
  launch_at_login: boolean;
  sync_enabled: boolean;
  model_mode: "auto" | "manual";
  speech_model: string;
  llm_model: string;
  use_gpu: boolean;
  hw_fingerprint: string;
  auto_reason: string;
  final_pass: boolean;
  speech_cloud_on: boolean;
  speech_cloud: CloudCfg;
  llm_provider: Provider;
  llm_cloud: CloudCfg;
}

export type Status = "draft" | "recording" | "paused" | "processing" | "ready" | "failed";

export interface Meeting {
  id: string;
  title: string;
  createdAt: string;
  startedAt: string | null;
  endedAt: string | null;
  durationS: number;
  app: string;
  status: Status;
  step: string;
  templateId: string;
  myNotes: string;
  enhanced: string;
  folder: string;
  attendees: string;
  speakerNames: string;
  error: string;
  hasAudio: boolean;
  updatedAt: string;
}

export interface Segment {
  id: number;
  source: "mic" | "system";
  speaker: string;
  start: number;
  end: number;
  text: string;
  isFinal: boolean;
}

export interface Template {
  id: string;
  name: string;
  description: string;
  sections: string;
  instructions: string;
  builtin: boolean;
  updatedAt: string;
}

export interface ChatMsg {
  id: number;
  meetingId: string | null;
  role: "user" | "assistant";
  content: string;
  createdAt: string;
}

export interface Hit {
  meetingId: string;
  title: string;
  createdAt: string;
  snippet: string;
}

export interface Levels {
  mic: number;
  system: number;
  secs: number;
  paused: boolean;
  mic_device: string;
  system_device: string;
  mic_error: string | null;
  system_error: string | null;
}

export interface RecState {
  meetingId: string | null;
  app: string;
  levels: Levels | null;
}

export interface Hardware {
  gpu_vendor: string;
  gpu_name: string;
  vram_mb: number;
  ram_mb: number;
  threads: number;
}

export interface ModelsInfo {
  hardware: Hardware;
  choice: { tier: string; speech_model: string; llm_model: string; final_pass: boolean; why: string };
  models: { id: string; label: string; kind: "speech" | "llm"; sizeMb: number; note: string; installed: boolean }[];
  notesReady: boolean;
  speechInUse: string;
}

export interface Call {
  key: string;
  app: string;
}

export interface Account {
  name: string;
  handle: string;
  avatar_url: string | null;
}

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  appInfo: () => invoke<{ version: string; platform: string; fromHub: boolean; dataDir: string }>("app_info"),
  uiReady: () => invoke<void>("ui_ready"),
  meetings: (folder?: string | null) => invoke<Meeting[]>("meetings_list", { folder: folder ?? null }),
  meeting: (id: string) => invoke<{ meeting: Meeting; segments: Segment[] }>("meeting_get", { id }),
  createMeeting: (title = "") => invoke<Meeting>("meeting_create", { title }),
  update: (id: string, field: keyof Meeting | string, value: string) => invoke<void>("meeting_update", { id, field: toSnake(field), value }),
  deleteMeeting: (id: string) => invoke<void>("meeting_delete", { id }),
  folders: () => invoke<string[]>("folders"),
  renameSpeaker: (id: string, speaker: string, name: string) => invoke<void>("rename_speaker", { id, speaker, name }),
  segmentUpdate: (meetingId: string, segmentId: number, text: string) => invoke<void>("segment_update", { meetingId, segmentId, text }),
  segmentDelete: (meetingId: string, segmentId: number) => invoke<void>("segment_delete", { meetingId, segmentId }),
  recordStart: (meetingId?: string | null, app?: string) => invoke<Meeting>("record_start", { meetingId: meetingId ?? null, app: app ?? null }),
  recordStop: () => invoke<string | null>("record_stop"),
  recordPause: (paused: boolean) => invoke<void>("record_pause", { paused }),
  recState: () => invoke<RecState>("rec_state"),
  regenerate: (id: string, templateId?: string | null) => invoke<void>("regenerate", { id, templateId: templateId ?? null }),
  reprocess: (id: string) => invoke<void>("reprocess", { id }),
  templates: () => invoke<Template[]>("templates_list"),
  saveTemplate: (template: Template) => invoke<Template>("template_save", { template }),
  deleteTemplate: (id: string) => invoke<void>("template_delete", { id }),
  search: (q: string) => invoke<Hit[]>("search", { q }),
  chats: (meetingId: string | null) => invoke<ChatMsg[]>("chat_list", { meetingId }),
  ask: (meetingId: string | null, question: string, requestId: string) => invoke<string>("chat_ask", { meetingId, question, requestId }),
  clearChat: (meetingId: string | null) => invoke<void>("chat_clear", { meetingId }),
  mics: () => invoke<string[]>("list_mics"),
  audioCheck: (secs: number) => invoke<[number, number]>("audio_check", { secs }),
  models: () => invoke<ModelsInfo>("models_info"),
  rerunAuto: () => invoke<void>("rerun_auto"),
  downloadModel: (id: string) => invoke<void>("download_model", { id }),
  detected: () => invoke<Call | null>("detected"),
  popupAction: (take: boolean) => invoke<void>("popup_action", { take }),
  openHub: (page?: string) => invoke<void>("open_hub", { page: page ?? null }),
  exportMarkdown: (name: string, content: string) => invoke<string>("export_markdown", { name, content }),
  openDataDir: () => invoke<void>("open_data_dir"),
  revealAudio: (id: string) => invoke<void>("reveal_audio", { id }),
  quit: () => invoke<void>("quit"),
  account: () => invoke<Account | null>("toolbay_account"),
  syncStatus: () => invoke<[boolean, boolean]>("sync_status"),
  pillClick: () => invoke<void>("pill_click"),
};

function toSnake(s: string) {
  return s.replace(/[A-Z]/g, (c) => "_" + c.toLowerCase());
}

/** Subscribe to a backend event; returns an unsubscribe function. */
export function on<T = unknown>(event: string, cb: (payload: T) => void): () => void {
  let un: (() => void) | null = null;
  let dead = false;
  listen<T>(event, (e) => cb(e.payload)).then((u) => {
    if (dead) u();
    else un = u;
  });
  return () => {
    dead = true;
    un?.();
  };
}
