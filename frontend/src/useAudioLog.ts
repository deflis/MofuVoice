import { useEffect } from "react";
import { useShallow } from "zustand/react/shallow";
import { useAudioLogStore } from "./audioLogStore";

// Only the mounted log panel owns a connection to the log stream.
export function useAudioLog() {
  useEffect(() => useAudioLogStore.getState().connect(), []);
  return useAudioLogStore(useShallow((state) => ({ log: state.log, error: state.error })));
}
