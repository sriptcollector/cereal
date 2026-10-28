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
  meeting_start: "record" | "ask" | "nothing";
  app_rules: Record<string, string>;
  auto_consent_shown: boolean;
  auto_stop: boolean;
  consent_reminder: boolean;
  consent_message: string;
  default_template: string;
  keep_audio: "keep" | "30d" | "delete";
  sync_enabled: boolean;
  sync_asked: boolean;
  job_title: string;
  about: string;
  role: string;
  vocabulary: string[];
  summary_language: string;
  calendars: CalendarSrc[];
  calendar_popups: boolean;
  reposition: boolean;
  sounds: boolean;
  show_indicator: boolean;
  quick_chat: boolean;
  chat_enter: string;
  show_solo_events: boolean;
  company_name: string;
  company_about: string;
  notion_token: string;
  notion_page: string;
  hubspot_token: string;
  screen_snapshots: boolean;
  speaker_suggestions: boolean;
  hot_corner: boolean;
  transcript_retention: string;
  theme: "system" | "light" | "dark";
  beta: boolean;
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
  versions: string;
  folder: string;
  attendees: string;
  speakerNames: string;
  error: string;
  hasAudio: boolean;
  updatedAt: string;
  deletedAt: string | null;
  eventId: string;
}

export interface CalendarSrc {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
}

export interface CalEvent {
  id: string;
  uid: string;
  title: string;
  start: string;
  end: string;
  allDay: boolean;
  attendees: string[];
  location: string;
  link: string;
  calendar: string;
  description: string;
  declined: boolean;
}

export interface Folder {
  id: string;
  name: string;
  icon: string;
  description: string;
  sort: number;
  autoMatch: string;
  webhook: string;
  createdAt: string;
  count: number;
  parentId: string;
  favorite: boolean;
}

export interface FileItem {
  id: string;
  folderId: string;
  name: string;
  chars: number;
  addedAt: string;
}

export interface Thread {
  id: string;
  scope: string;
  scopeId: string;
  title: string;
  createdAt: string;
  updatedAt: string;
}

export interface Recipe {
  id: string;
  cmd: string;
  name: string;
  prompt: string;
  kind: "single" | "multi";
  builtin: boolean;
}

export interface Person {
  key: string;
  name: string;
  email: string;
  company: string;
  meetings: number;
  last: string;
}

export interface Company {
  key: string;
  name: string;
  domain: string;
  people: string[];
  meetings: number;
  last: string;
}

export interface Source {
  n: number;
  meetingId: string;
  title: string;
  date: string;
}

export interface Answer {
  threadId: string;
  answer: string;
  sources: Source[];
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
  trash: (id: string) => invoke<void>("meeting_trash", { id }),
  restore: (id: string) => invoke<void>("meeting_restore", { id }),
  purge: (id: string) => invoke<void>("meeting_purge", { id }),
  trashList: () => invoke<Meeting[]>("trash_list"),
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
  threads: (scope?: string | null, scopeId?: string | null) => invoke<Thread[]>("threads_list", { scope: scope ?? null, scopeId: scopeId ?? null }),
  thread: (id: string) => invoke<{ thread: Thread | null; msgs: ChatMsg[]; sources: Source[] }>("thread_get", { id }),
  deleteThread: (id: string) => invoke<void>("thread_delete", { id }),
  chatSend: (a: { threadId: string | null; scope: string; scopeId: string; question: string; attachment?: string | null; requestId: string }) =>
    invoke<Answer>("chat_send", { ...a, attachment: a.attachment ?? null }),
  folders: () => invoke<Folder[]>("folders_list"),
  saveFolder: (folder: Partial<Folder>) => invoke<Folder>("folder_save", { folder: { id: "", name: "", icon: "", description: "", sort: 0, autoMatch: "", webhook: "", createdAt: "", count: 0, ...folder } }),
  deleteFolder: (id: string) => invoke<void>("folder_delete", { id }),
  files: (folder: string) => invoke<FileItem[]>("files_list", { folder }),
  addFile: (folder: string, path: string) => invoke<void>("file_add", { folder, path }),
  deleteFile: (id: string) => invoke<void>("file_delete", { id }),
  people: () => invoke<Person[]>("people_list"),
  companies: () => invoke<Company[]>("companies_list"),
  meetingsWith: (person?: string | null, domain?: string | null) => invoke<Meeting[]>("meetings_with", { person: person ?? null, domain: domain ?? null }),
  recipes: () => invoke<Recipe[]>("recipes_list"),
  saveRecipe: (recipe: Recipe) => invoke<Recipe>("recipe_save", { recipe }),
  deleteRecipe: (id: string) => invoke<void>("recipe_delete", { id }),
  events: () => invoke<CalEvent[]>("calendar_events"),
  refreshCalendars: () => invoke<number>("calendar_refresh"),
  noteForEvent: (eventId: string) => invoke<Meeting>("note_for_event", { eventId }),
  briefForEvent: (eventId: string) => invoke<string>("brief_for_event", { eventId }),
  joinEvent: (eventId: string, record: boolean) => invoke<Meeting>("join_event", { eventId, record }),
  dock: () => invoke<void>("dock_window"),
  dictateStart: () => invoke<void>("dictate_start"),
  dictateStop: () => invoke<string>("dictate_stop"),
  importAudio: (path: string) => invoke<Meeting>("import_audio", { path }),
  exportDocx: (name: string, markdown: string) => invoke<string>("export_docx", { name, markdown }),
  exportAll: (kind: "csv" | "json") => invoke<string>("export_all", { kind }),
  setAppRule: (app: string, rule: string) => invoke<void>("set_app_rule", { app, rule }),
  feedbackInfo: () => invoke<boolean>("feedback_info"),
  feedbackSend: (kind: string, text: string, screenshot: string | null, includeDebug: boolean) => invoke<string>("feedback_send", { kind, text, screenshot, includeDebug }),
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
  syncDeleteRemote: () => invoke<void>("sync_delete_remote"),
  pillClick: () => invoke<void>("pill_click"),
  stillHere: () => invoke<void>("still_here"),
  screens: (id: string) => invoke<{ name: string; thumb: string }[]>("screens_list", { id }),
  screenToNote: (id: string, name: string) => invoke<void>("screen_to_note", { id, name }),
  screenRemove: (id: string, name: string) => invoke<void>("screen_remove", { id, name }),
  sendToNotion: (id: string) => invoke<string>("send_to_notion", { id }),
  sendToHubspot: (id: string) => invoke<number>("send_to_hubspot", { id }),
  postText: (folder: string, text: string) => invoke<void>("post_text", { folder, text }),
  calendarDraft: (summary: string, details: string) => invoke<void>("calendar_draft", { summary, details }),
  miniWindow: () => invoke<boolean>("mini_window"),
  exePath: () => invoke<string>("exe_path"),
  useVersion: (id: string, index: number) => invoke<void>("use_version", { id, index }),
  quickHide: () => invoke<void>("quick_hide"),
  pillWide: (wide: boolean) => invoke<void>("pill_wide", { wide }),
  screenshot: () => invoke<void>("screenshot"),
  currentEvent: () => invoke<CalEvent | null>("current_event"),
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
