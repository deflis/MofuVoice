import { invoke } from "@tauri-apps/api/core";

export const hideLog = () => invoke<void>("close_log");
