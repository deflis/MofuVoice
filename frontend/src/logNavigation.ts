import { invoke } from "@tauri-apps/api/core";

export async function openLog() {
  await invoke("open_log");
}
