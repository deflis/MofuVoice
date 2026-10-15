import { useShallow } from "zustand/react/shallow";
import { useSettingsStore } from "../store";
import { useSettingsConnection, changeBackend } from "../listener";
export function useDeviceSelection() {
  const values = useSettingsStore((state) => state.devices);
  const metadata = useSettingsConnection(
    useShallow((state) => ({
      resolvedBackend: state.resolvedBackend,
      devices: state.devices,
      outputDevices: state.outputDevices,
    })),
  );
  return { ...values, ...metadata };
}
export function useSampleRateSettings() {
  const values = useSettingsStore((state) => state.devices);
  const resolvedBackend = useSettingsConnection((state) => state.resolvedBackend);
  return { ...values, resolvedBackend };
}
export const useChannelRouting = () => useSettingsStore((state) => state.devices);
export const useBufferSettings = () => useSettingsStore((state) => state.devices);
export function useBackendSettings() {
  const backend = useSettingsStore((state) => state.devices.backend);
  const backends = useSettingsConnection((state) => state.backends);
  return { backend, backends, changeBackend };
}
