import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type Settings, type Snapshot } from "./api";

const levelFields: {
  key: Exclude<keyof Settings["level"], "enabled">;
  label: string;
  min: number;
  max: number;
}[] = [
  { key: "targetDb", label: "Target RMS (dBFS)", min: -36, max: -6 },
  { key: "maxGainDb", label: "Max gain (dB)", min: 0, max: 18 },
  { key: "floorDb", label: "Activity floor (dBFS)", min: -90, max: -30 },
  { key: "responseMs", label: "Level response (ms)", min: 100, max: 10000 },
  { key: "thresholdDb", label: "Comp. threshold (dBFS)", min: -36, max: 0 },
  { key: "ratio", label: "Ratio", min: 1, max: 20 },
  { key: "attackMs", label: "Attack (ms)", min: 0.1, max: 100 },
  { key: "releaseMs", label: "Release (ms)", min: 10, max: 2000 },
];

function Semitones({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const number = Number(value);
  return (
    <label className="semitones">
      <span>
        {label}
        <small>半音</small>
      </span>
      <div className="slider-row">
        <input
          type="range"
          min="-12"
          max="12"
          step="0.01"
          value={Number.isFinite(number) ? number : 0}
          onChange={(event) => onChange(event.target.value)}
        />
        <input
          aria-label={`${label}の値`}
          type="number"
          min="-12"
          max="12"
          step="0.01"
          required
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
      </div>
    </label>
  );
}

export function App() {
  const [settings, setSettings] = useState<Settings>();
  const [levelDefaults, setLevelDefaults] = useState<Settings["level"]>();
  const [devices, setDevices] = useState<string[]>([]);
  const [snapshot, setSnapshot] = useState<Snapshot>({
    busy: false,
    status: "準備中…",
  });
  const [pending, setPending] = useState(false);
  const [closing, setClosing] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const settingsRef = useRef(settings);
  const operationRef = useRef(false);
  useEffect(() => {
    settingsRef.current = settings;
  }, [settings]);

  useEffect(() => {
    let disposed = false;
    const close = async () => {
      if (operationRef.current || !settingsRef.current) return;
      operationRef.current = true;
      setClosing(true);
      setError("");
      try {
        await api.close(settingsRef.current);
      } catch (failure) {
        setError(String(failure));
        setClosing(false);
        operationRef.current = false;
      }
    };

    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const current = await api.status();
        if (!disposed) setSnapshot(current);
      } catch (failure) {
        if (!disposed) setError(String(failure));
      }
      if (!disposed) timer = setTimeout(poll, 250);
    };
    api
      .bootstrap()
      .then((result) => {
        if (!disposed) {
          setSettings(result.settings);
          setLevelDefaults(result.levelDefaults);
          setDevices(result.devices);
        }
      })
      .catch((failure) => {
        if (!disposed) setError(String(failure));
      });
    void poll();
    const unlisten = listen("request-close", () => {
      void close();
    });
    unlisten.catch((failure) => {
      if (!disposed) setError(String(failure));
    });
    return () => {
      disposed = true;
      clearTimeout(timer);
      void unlisten.then((remove) => remove()).catch(() => {});
    };
  }, []);

  const update = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setSettings((current) => (current ? { ...current, [key]: value } : current));
    setNotice("");
  };
  const run = async (action: () => Promise<void>) => {
    if (operationRef.current) return;
    operationRef.current = true;
    setPending(true);
    setError("");
    setNotice("");
    try {
      await action();
      setSnapshot(await api.status());
    } catch (failure) {
      setError(String(failure));
    } finally {
      operationRef.current = false;
      setPending(false);
    }
  };
  const locked = pending || closing || snapshot.busy;
  const firstLine = snapshot.status.split("\n")[0];

  return (
    <main>
      <header>
        <h1>Voice Changer</h1>
        <span>{closing ? "終了中" : snapshot.busy ? "処理中" : "停止中"}</span>
      </header>
      {error && (
        <div className="error" role="alert">
          {error}
        </div>
      )}
      {notice && (
        <div className="notice" role="status">
          {notice}
        </div>
      )}
      {!settings ? (
        <p>設定を読み込んでいます…</p>
      ) : (
        <>
          <section>
            <div className="actions">
              <button
                disabled={locked || !settings.device}
                onClick={() => void run(() => api.start(settings, false))}
              >
                Start
              </button>
              <button
                disabled={!snapshot.busy || pending || closing}
                onClick={() => void run(api.stop)}
              >
                Stop
              </button>
              <button
                disabled={locked || !settings.device}
                onClick={() => void run(() => api.start(settings, true))}
              >
                Inspect
              </button>
              <button disabled={pending || closing} onClick={() => void run(api.spectrum)}>
                スペアナ
              </button>
            </div>
            <p role="status">{firstLine}</p>
          </section>
          <fieldset className="settings" disabled={locked}>
            <div className="grid">
              <section>
                <h2>音声デバイス</h2>
                <label>
                  ASIO device
                  <select
                    value={settings.device}
                    onChange={(event) => update("device", event.target.value)}
                  >
                    <option value="">デバイスを選択</option>
                    {settings.device && !devices.includes(settings.device) && (
                      <option value={settings.device}>{settings.device}（未検出）</option>
                    )}
                    {devices.map((device) => (
                      <option key={device}>{device}</option>
                    ))}
                  </select>
                </label>
                <label>
                  Frames
                  <input
                    type="number"
                    min="1"
                    max="8192"
                    step="1"
                    required
                    value={settings.frames}
                    onChange={(event) => update("frames", event.target.value)}
                  />
                </label>
              </section>
              <section>
                <h2>声の調整</h2>
                <Semitones
                  label="Pitch"
                  value={settings.pitch}
                  onChange={(value) => update("pitch", value)}
                />
                <Semitones
                  label="Formant shift"
                  value={settings.formantShift}
                  onChange={(value) => update("formantShift", value)}
                />
                <label className="check">
                  <input
                    type="checkbox"
                    checked={settings.preserveFormants}
                    onChange={(event) => update("preserveFormants", event.target.checked)}
                  />
                  Preserve formants
                </label>
              </section>
            </div>
            <section>
              <div className="section-heading">
                <h2>ノイズゲート</h2>
                <label className="check">
                  <input
                    type="checkbox"
                    checked={settings.gate}
                    onChange={(event) => update("gate", event.target.checked)}
                  />
                  有効
                </label>
              </div>
              <div className="gate-fields">
                <label>
                  Threshold · dB / auto
                  <input
                    value={settings.gateThreshold}
                    onChange={(event) => update("gateThreshold", event.target.value)}
                  />
                </label>
                <label>
                  Attack · ms
                  <input
                    type="number"
                    min="0"
                    step="any"
                    required
                    value={settings.gateAttackMs}
                    onChange={(event) => update("gateAttackMs", event.target.value)}
                  />
                </label>
                <label>
                  Release · ms
                  <input
                    type="number"
                    min="0"
                    step="any"
                    required
                    value={settings.gateReleaseMs}
                    onChange={(event) => update("gateReleaseMs", event.target.value)}
                  />
                </label>
              </div>
            </section>
            <section>
              <h2>Auto level</h2>
              <label className="check">
                <input
                  type="checkbox"
                  checked={settings.level.enabled}
                  onChange={(event) =>
                    update("level", { ...settings.level, enabled: event.target.checked })
                  }
                />
                有効
              </label>
              <details>
                <summary>詳細設定</summary>
                <div className="level-fields">
                  {levelFields.map((field) => (
                    <label key={field.key}>
                      {field.label}
                      <input
                        type="number"
                        step="any"
                        required
                        min={field.min}
                        max={field.max}
                        value={settings.level[field.key]}
                        onChange={(event) =>
                          update("level", { ...settings.level, [field.key]: event.target.value })
                        }
                      />
                    </label>
                  ))}
                </div>
                <button
                  disabled={!levelDefaults}
                  onClick={() => {
                    if (levelDefaults)
                      update("level", { ...levelDefaults, enabled: settings.level.enabled });
                  }}
                >
                  既定値に戻す
                </button>
              </details>
            </section>
          </fieldset>
          <footer>
            <button
              disabled={pending || closing}
              onClick={() =>
                void run(async () => {
                  await api.save(settings);
                  setNotice("設定を保存しました。");
                })
              }
            >
              設定を保存
            </button>
          </footer>
          <details>
            <summary>処理状況・レイテンシの詳細</summary>
            <pre>{snapshot.status}</pre>
          </details>
        </>
      )}
    </main>
  );
}
