import { useTranslation } from "react-i18next";
import styles from "./log.module.css";
import { setLogging } from "./listener";
import { useLogStore } from "./store";
import { AudioLogPanel } from "./AudioLogPanel";

const ms = (value: number | null | undefined) =>
  value === null || value === undefined ? "—" : `${value.toFixed(2)} ms`;

export function StatusDetails() {
  const { t } = useTranslation();
  const info = useLogStore((state) => state.info);
  const logging = useLogStore((state) => state.logging);
  if (!info) {
    return (
      <section className={styles["status-panel"]}>
        <h2>{t("音声処理の状態")}</h2>
        <p>{t("状態を読み込んでいます…")}</p>
      </section>
    );
  }
  let outputDevice = "—";
  if (info.backend === "asio") {
    outputDevice = info.device ?? "—";
  } else if (info.backend === "cpal") {
    outputDevice = info.outputDevice || t("システム既定");
  }
  return (
    <section className={styles["status-panel"]}>
      <h2>{t("音声処理の状態")}</h2>
      {info.error && (
        <p className={styles.error} role="alert">
          {t(info.error)}
        </p>
      )}
      <p className={styles["device-name"]}>{info.device ?? t("デバイス未接続")}</p>
      <dl className={styles["status-grid"]}>
        <div>
          <dt>{t("バックエンド")}</dt>
          <dd>{info.backend?.toUpperCase() ?? "—"}</dd>
        </div>
        <div>
          <dt>{t("出力デバイス")}</dt>
          <dd>{outputDevice}</dd>
        </div>
        <div>
          <dt>{t("サンプルレート")}</dt>
          <dd>
            {info.sampleRate === null || info.sampleRate === undefined
              ? "—"
              : `${info.sampleRate.toLocaleString()} Hz`}
          </dd>
        </div>
        <div>
          <dt>{t("要求バッファ")}</dt>
          <dd>
            {info.frames === null || info.frames === undefined ? "—" : `${info.frames} frames`}
          </dd>
        </div>
        <div>
          <dt>{t("入力 / 出力")}</dt>
          <dd>
            {info.inputs === null || info.inputs === undefined
              ? "—"
              : `${info.inputs} / ${info.outputs} ch`}
          </dd>
        </div>
        <div>
          <dt>{t("要求処理周期")}</dt>
          <dd>{ms(info.periodMs)}</dd>
        </div>
        <div>
          <dt>{t("ドライバー入力遅延")}</dt>
          <dd>{ms(info.inputLatencyMs)}</dd>
        </div>
        <div>
          <dt>{t("ドライバー出力遅延")}</dt>
          <dd>{ms(info.outputLatencyMs)}</dd>
        </div>
        <div>
          <dt>{t("DSP遅延")}</dt>
          <dd>{ms(info.dspMs)}</dd>
        </div>
      </dl>
      <p className={styles.muted}>
        {info.backend === "cpal"
          ? t("CPALではドライバー遅延を取得できません。往復遅延は未測定です。")
          : t("ドライバー報告値です。実測の往復遅延とは異なります。")}
      </p>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={logging}
          onChange={(event) => void setLogging(event.target.checked)}
        />
        {t("処理ログを表示")}
      </label>
      {logging && <AudioLogPanel />}
    </section>
  );
}
