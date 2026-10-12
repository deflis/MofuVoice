# 設定ガイド

[Rust Low-Latency Voice Changer](../README.md) を Voicemeeter または VB-Audio Matrix と組み合わせる手順です。いずれも **ASIO** で接続し、アプリ側は **入力1 → 出力1**（モノラル1本）だけを処理します。

| ガイド | 向いている構成 |
| --- | --- |
| [Voicemeeter シリーズ](./voicemeeter.md) | Banana / Potato の **Insert Virtual ASIO** でマイクストリップにボイスチェンジャーを差し込む |
| [VB-Audio Matrix](./vbmatrix.md) | Matrix の **Virtual ASIO（VASIO）** で送受信し、同じ要領で処理する |

共通の前提:

- サンプルレート **48 kHz**（Voicemeeter / Matrix / 本アプリで一致させる）
- GUI の **Frames** はまず **512**（Inspect で対応を確認）
- Insert / VASIO ドライバは **同時に1つの ASIO アプリだけ** が開けることが多い（本アプリ実行中は他の DAW などを同じドライバで開かない）

公式の Insert 解説: [Connect audio apps to the VoiceMeeter Insert Driver](https://voicemeeter.com/user-guide-connect-audio-apps-to-the-voicemeeter-insert-driver/)
