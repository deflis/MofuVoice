import type { StateCreator } from "zustand";
import type { SettingsStore } from "../store";
import type { DevicesValues } from "./schema";

type DevicesState = DevicesValues & {
  setBackend: (value: DevicesValues["backend"]) => void;
  setDevice: (value: DevicesValues["device"]) => void;
  setOutputDevice: (value: DevicesValues["outputDevice"]) => void;
  setInputChannel: (value: DevicesValues["inputChannel"]) => void;
  setOutputChannels: (value: DevicesValues["outputChannels"]) => void;
  setFrames: (value: DevicesValues["frames"]) => void;
  setSampleRate: (value: DevicesValues["sampleRate"]) => void;
  setRouting: (inputChannel: number, outputChannels: number[]) => void;
};
export type DevicesSlice = { devices: DevicesState };
export const createDevicesSlice: StateCreator<SettingsStore, [], [], DevicesSlice> = (set) => ({
  devices: {
    backend: "auto",
    device: "",
    outputDevice: "",
    inputChannel: 1,
    outputChannels: [1],
    frames: 512,
    sampleRate: 48000,
    setBackend: (value) => set((state) => ({ devices: { ...state.devices, backend: value } })),
    setDevice: (value) => set((state) => ({ devices: { ...state.devices, device: value } })),
    setOutputDevice: (value) =>
      set((state) => ({ devices: { ...state.devices, outputDevice: value } })),
    setInputChannel: (value) =>
      set((state) => ({ devices: { ...state.devices, inputChannel: value } })),
    setOutputChannels: (value) =>
      set((state) => ({ devices: { ...state.devices, outputChannels: [...value] } })),
    setFrames: (value) => set((state) => ({ devices: { ...state.devices, frames: value } })),
    setSampleRate: (value) =>
      set((state) => ({ devices: { ...state.devices, sampleRate: value } })),
    setRouting: (inputChannel, outputChannels) =>
      set((state) => ({
        devices: { ...state.devices, inputChannel, outputChannels: [...outputChannels] },
      })),
  },
});
