import { useTranslation } from "react-i18next";
import styles from "./overview.module.css";
import type { ComponentType } from "react";
import { useSettingsConnection } from "../listener";
import type { CardProps } from "./Card";
import { EqCard } from "./EqCard";
import { NoiseGateCard, DeesserCard, VoiceCard, LevelCard } from "./ExistingCards";
import { LimiterCard, DereverbCard, ReverbCard, CompressorCard } from "./NewCards";
import { useSettingsLocked } from "../hooks";

const cards: Record<string, ComponentType<CardProps>> = {
  inputEq: (props) => (
    <EqCard {...props} slot="inputEq" id="inputEq" title="入力EQ・低域ノイズ対策" />
  ),
  limiter: LimiterCard,
  noiseGate: NoiseGateCard,
  dereverb: DereverbCard,
  deesser: DeesserCard,
  compressor: CompressorCard,
  preConversionEq: (props) => (
    <EqCard {...props} slot="preConversionEq" id="preConversionEq" title="変換前EQ" />
  ),
  voice: VoiceCard,
  postConversionEq: (props) => (
    <EqCard {...props} slot="eq" id="postConversionEq" title="変換後EQ" />
  ),
  level: LevelCard,
  reverb: ReverbCard,
};

export function FilterOverview({ onSelect }: Pick<CardProps, "onSelect">) {
  const { t } = useTranslation();
  const stages = useSettingsConnection((state) => state.chainStages);
  const locked = useSettingsLocked();
  const dirty = useSettingsConnection((state) => state.dirty);
  const busy = useSettingsConnection((state) => state.audioBusy);
  const status = useSettingsConnection((state) => state.status);
  let lifecycle = t("保存済み · 次回開始時に適用");
  if (status === "loading") {
    lifecycle = t("設定を読み込み中");
  } else if (status === "loadFailed") {
    lifecycle = t("設定の読み込みに失敗");
  } else if (dirty) {
    lifecycle = t("未保存 · 保存後、次回開始時に適用");
  } else if (busy) {
    lifecycle = t("音声処理中 · 設定表示（実測ではありません）");
  }
  return (
    <div className={styles["filter-overview"]}>
      <h2>{t("フィルタ構成")}</h2>
      <output className={styles["filter-lifecycle"]}>{lifecycle}</output>
      <p>
        {t(
          "上から下が音声の処理順です。各チェックボックスで個別に切り替えられ、無効中は詳細を表示しません。図と値は個別設定と共通で、実測メーターではありません。",
        )}
      </p>
      {!stages.length ? (
        <p>{t("処理順の情報を取得できていません。")}</p>
      ) : (
        <>
          <p className={styles["filter-endpoint"]}>{t("音声入力")}</p>
          <ol className={styles["filter-chain"]} aria-label={t("固定の音声処理順")}>
            {stages.map((id) => {
              const StageCard = cards[id];
              return (
                <li key={id}>
                  <span className={styles["filter-arrow"]} aria-hidden="true">
                    ↓
                  </span>
                  {id === "inputEq" && (
                    <p className={styles["filter-divider"]}>{t("変換前の処理")}</p>
                  )}
                  {id === "voice" && <p className={styles["filter-divider"]}>{t("声変換")}</p>}
                  {id === "postConversionEq" && (
                    <p className={styles["filter-divider"]}>{t("変換後の処理")}</p>
                  )}
                  {StageCard ? (
                    <StageCard onSelect={onSelect} locked={locked} />
                  ) : (
                    <p>
                      {t("表示に対応していない処理:")}
                      {id}
                    </p>
                  )}
                </li>
              );
            })}
          </ol>
          <span className={styles["filter-arrow"]} aria-hidden="true">
            ↓
          </span>
          <p className={styles["filter-endpoint"]}>{t("音声出力")}</p>
        </>
      )}
    </div>
  );
}
