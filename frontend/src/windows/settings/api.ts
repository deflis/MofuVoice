import * as v from "valibot";
import { settingsViewSchema, deviceChoicesSchema } from "./schema";
import { invoke } from "@tauri-apps/api/core";
import type { DeviceSampleRates, Settings } from "../../types";

export const settingsApi = {
  getView: async () => v.parse(settingsViewSchema, await invoke<unknown>("settings_view")),
  save: (settings: Settings) => invoke<void>("save_settings", { settings }),
  deviceChoices: async (backend: string) =>
    v.parse(deviceChoicesSchema, await invoke<unknown>("device_choices", { backend })),
  deviceSampleRates: (
    query: Pick<Settings["devices"], "device" | "outputDevice"> & { backend: string },
  ) => invoke<DeviceSampleRates>("device_sample_rates", query),
  hide: () => invoke<void>("close_settings"),
  closeApplication: () => invoke<void>("close_application"),
  cancelApplicationClose: () => invoke<void>("cancel_application_close"),
};

export type SettingsApi = typeof settingsApi;
