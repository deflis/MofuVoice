import styles from "./log.module.css";
import { setLogging } from "./listener";
import { useLogStore } from "./store";
import { AudioLogPanel } from "./AudioLogPanel";

const ms = (value: number | null | undefined) =>
  value === null || value === undefined ? "—" : `${value.toFixed(2)} ms`;

export function StatusDetails() {
  const info = useLogStore((state) => state.info);
  const logging = useLogStore((state) => state.logging);
  if (!info) {
    return (
      <section className={styles["status-panel"]}>
        <h2>音声処理の状態</h2>
        <p>状態を読み込んでいます…</p>
      </section>
    );
  }
  let outputDevice = "—";
  if (info.backend === "asio") {
    outputDevice = info.device ?? "—";
  } else if (info.backend === "cpal") {
    outputDevice = info.outputDevice || "システム既定";
  }
  return (
    <section className={styles["status-panel"]}>
      <h2>音声処理の状態</h2>
      {info.error && (
        <p className={styles.error} role="alert">
          {info.error}
        </p>
      )}
      <p className={styles["device-name"]}>{info.device ?? "デバイス未接続"}</p>
      <dl className={styles["status-grid"]}>
        <div>
          <dt>バックエンド</dt>
          <dd>{info.backend?.toUpperCase() ?? "—"}</dd>
        </div>
        <div>
          <dt>出力デバイス</dt>
          <dd>{outputDevice}</dd>
        </div>
        <div>
          <dt>サンプルレート</dt>
          <dd>
            {info.sampleRate === null || info.sampleRate === undefined
              ? "—"
              : `${info.sampleRate.toLocaleString()} Hz`}
          </dd>
        </div>
        <div>
          <dt>要求バッファ</dt>
          <dd>
            {info.frames === null || info.frames === undefined ? "—" : `${info.frames} frames`}
          </dd>
        </div>
        <div>
          <dt>入力 / 出力</dt>
          <dd>
            {info.inputs === null || info.inputs === undefined
              ? "—"
              : `${info.inputs} / ${info.outputs} ch`}
          </dd>
        </div>
        <div>
          <dt>要求処理周期</dt>
          <dd>{ms(info.periodMs)}</dd>
        </div>
        <div>
          <dt>ドライバー入力遅延</dt>
          <dd>{ms(info.inputLatencyMs)}</dd>
        </div>
        <div>
          <dt>ドライバー出力遅延</dt>
          <dd>{ms(info.outputLatencyMs)}</dd>
        </div>
        <div>
          <dt>DSP遅延</dt>
          <dd>{ms(info.dspMs)}</dd>
        </div>
      </dl>
      <p className={styles.muted}>
        {info.backend === "cpal"
          ? "CPALではドライバー遅延を取得できません。往復遅延は未測定です。"
          : "ドライバー報告値です。実測の往復遅延とは異なります。"}
      </p>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={logging}
          onChange={(event) => void setLogging(event.target.checked)}
        />
        処理ログを表示
      </label>
      {logging && <AudioLogPanel />}
    </section>
  );
}
