import { invoke } from "@tauri-apps/api/core";

/** Reads a small text file the user picked (for chat attachments). */
export function readTextFile(path: string) {
  return invoke<string>("read_text_file", { path });
}
