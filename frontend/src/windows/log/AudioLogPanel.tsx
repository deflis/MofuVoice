import styles from "./AudioLogPanel.module.css";
import commonStyles from "./log.module.css";
import { useLogStore } from "./store";

export function AudioLogPanel() {
  const log = useLogStore((state) => state.log);
  const metrics = log?.telemetry;
  const elapsedSeconds = log?.elapsedSeconds;
  return (
    <div className={styles["log-panel"]}>
      <dl className={commonStyles["status-grid"]}>
        <div>
          <dt>経過時間</dt>
          <dd>
            {elapsedSeconds === null || elapsedSeconds === undefined
              ? "—"
              : `${elapsedSeconds.toFixed(1)} s`}
          </dd>
        </div>
        <div>
          <dt>処理ブロック</dt>
          <dd>{metrics?.blocks ?? "—"}</dd>
        </div>
        <div>
          <dt>非ゼロ入力サンプル</dt>
          <dd>{metrics?.nonzeroSamples ?? "—"}</dd>
        </div>
        <div>
          <dt>最大コールバック</dt>
          <dd>
            {metrics === null || metrics === undefined
              ? "—"
              : `${metrics.maxCallbackUs.toFixed(2)} μs`}
          </dd>
        </div>
        <div>
          <dt>期限超過</dt>
          <dd>{metrics?.deadlineMisses ?? "—"}</dd>
        </div>
        <div>
          <dt>タイムスタンプブロック</dt>
          <dd>{metrics?.timestampBlocks ?? "—"}</dd>
        </div>
        <div>
          <dt>サンプル位置の欠落</dt>
          <dd>{metrics?.samplePositionGaps ?? "—"}</dd>
        </div>
        <div>
          <dt>エラーフラグ</dt>
          <dd>
            {metrics === null || metrics === undefined
              ? "—"
              : `0x${metrics.faultFlags.toString(16)}`}
          </dd>
        </div>
        {metrics?.outputBlocks !== null && metrics?.outputBlocks !== undefined && (
          <>
            <div>
              <dt>出力コールバック</dt>
              <dd>{metrics.outputBlocks}</dd>
            </div>
            <div>
              <dt>アンダーラン</dt>
              <dd>{metrics.underruns}</dd>
            </div>
            <div>
              <dt>オーバーラン</dt>
              <dd>{metrics.overruns}</dd>
            </div>
            <div>
              <dt>現在のキュー</dt>
              <dd>{metrics.queueFrames} frames</dd>
            </div>
            <div>
              <dt>観測した最大入力バッファ</dt>
              <dd>{metrics.observedInputFrames ?? "—"} frames</dd>
            </div>
            <div>
              <dt>観測した最大出力バッファ</dt>
              <dd>{metrics.observedOutputFrames ?? "—"} frames</dd>
            </div>
            <div>
              <dt>最大入力コールバック</dt>
              <dd>{metrics.maxInputCallbackUs?.toFixed(2) ?? "—"} μs</dd>
            </div>
            <div>
              <dt>最大出力コールバック</dt>
              <dd>{metrics.maxOutputCallbackUs?.toFixed(2) ?? "—"} μs</dd>
            </div>
            <div>
              <dt>キューの最大量</dt>
              <dd>{metrics.queuePeakFrames ?? "—"} frames</dd>
            </div>
            <div>
              <dt>開始時のキュー目標</dt>
              <dd>{metrics.startupQueueFrames ?? "—"} frames</dd>
            </div>
            <div>
              <dt>キュー容量</dt>
              <dd>{metrics.queueCapacityFrames ?? "—"} frames</dd>
            </div>
          </>
        )}
      </dl>
      <p className={commonStyles.muted}>
        非ゼロ入力は信号の到着を示します。タイムスタンプが0または未取得の場合、欠落検出は利用できません。
      </p>
    </div>
  );
}
