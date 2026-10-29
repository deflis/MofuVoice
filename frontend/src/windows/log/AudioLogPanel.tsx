import { useTranslation } from "react-i18next";
import styles from "./AudioLogPanel.module.css";
import commonStyles from "./log.module.css";
import { useLogStore } from "./store";

export function AudioLogPanel() {
  const { t } = useTranslation();
  const log = useLogStore((state) => state.log);
  const metrics = log?.telemetry;
  const elapsedSeconds = log?.elapsedSeconds;
  return (
    <div className={styles["log-panel"]}>
      <dl className={commonStyles["status-grid"]}>
        <div>
          <dt>{t("経過時間")}</dt>
          <dd>
            {elapsedSeconds === null || elapsedSeconds === undefined
              ? "—"
              : `${elapsedSeconds.toFixed(1)} s`}
          </dd>
        </div>
        <div>
          <dt>{t("処理ブロック")}</dt>
          <dd>{metrics?.blocks ?? "—"}</dd>
        </div>
        <div>
          <dt>{t("非ゼロ入力サンプル")}</dt>
          <dd>{metrics?.nonzeroSamples ?? "—"}</dd>
        </div>
        <div>
          <dt>{t("最大コールバック")}</dt>
          <dd>
            {metrics === null || metrics === undefined
              ? "—"
              : `${metrics.maxCallbackUs.toFixed(2)} μs`}
          </dd>
        </div>
        <div>
          <dt>{t("期限超過")}</dt>
          <dd>{metrics?.deadlineMisses ?? "—"}</dd>
        </div>
        <div>
          <dt>{t("タイムスタンプブロック")}</dt>
          <dd>{metrics?.timestampBlocks ?? "—"}</dd>
        </div>
        <div>
          <dt>{t("サンプル位置の欠落")}</dt>
          <dd>{metrics?.samplePositionGaps ?? "—"}</dd>
        </div>
        <div>
          <dt>{t("エラーフラグ")}</dt>
          <dd>
            {metrics === null || metrics === undefined
              ? "—"
              : `0x${metrics.faultFlags.toString(16)}`}
          </dd>
        </div>
        {metrics?.outputBlocks !== null && metrics?.outputBlocks !== undefined && (
          <>
            <div>
              <dt>{t("出力コールバック")}</dt>
              <dd>{metrics.outputBlocks}</dd>
            </div>
            <div>
              <dt>{t("アンダーラン")}</dt>
              <dd>{metrics.underruns}</dd>
            </div>
            <div>
              <dt>{t("オーバーラン")}</dt>
              <dd>{metrics.overruns}</dd>
            </div>
            <div>
              <dt>{t("現在のキュー")}</dt>
              <dd>{metrics.queueFrames} frames</dd>
            </div>
            <div>
              <dt>{t("観測した最大入力バッファ")}</dt>
              <dd>{metrics.observedInputFrames ?? "—"} frames</dd>
            </div>
            <div>
              <dt>{t("観測した最大出力バッファ")}</dt>
              <dd>{metrics.observedOutputFrames ?? "—"} frames</dd>
            </div>
            <div>
              <dt>{t("最大入力コールバック")}</dt>
              <dd>{metrics.maxInputCallbackUs?.toFixed(2) ?? "—"} μs</dd>
            </div>
            <div>
              <dt>{t("最大出力コールバック")}</dt>
              <dd>{metrics.maxOutputCallbackUs?.toFixed(2) ?? "—"} μs</dd>
            </div>
            <div>
              <dt>{t("キューの最大量")}</dt>
              <dd>{metrics.queuePeakFrames ?? "—"} frames</dd>
            </div>
            <div>
              <dt>{t("開始時のキュー目標")}</dt>
              <dd>{metrics.startupQueueFrames ?? "—"} frames</dd>
            </div>
            <div>
              <dt>{t("キュー容量")}</dt>
              <dd>{metrics.queueCapacityFrames ?? "—"} frames</dd>
            </div>
          </>
        )}
      </dl>
      <p className={commonStyles.muted}>
        {t(
          "非ゼロ入力は信号の到着を示します。タイムスタンプが0または未取得の場合、欠落検出は利用できません。",
        )}
      </p>
    </div>
  );
}
