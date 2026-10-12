import { useAudioControls } from "../settingsHooks";

export function AudioControls() {
  const { startDisabled, stopDisabled, spectrumDisabled, firstLine, start, stop, spectrum } =
    useAudioControls();
  return (
    <section>
      <div className="actions">
        <button type="button" disabled={startDisabled} onClick={() => void start(false)}>
          Start
        </button>
        <button type="button" disabled={stopDisabled} onClick={() => void stop()}>
          Stop
        </button>
        <button type="button" disabled={startDisabled} onClick={() => void start(true)}>
          Inspect
        </button>
        <button type="button" disabled={spectrumDisabled} onClick={() => void spectrum()}>
          スペアナ
        </button>
      </div>
      <p>
        <output>{firstLine}</output>
      </p>
    </section>
  );
}
