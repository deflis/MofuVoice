# VB-Audio Matrix 設定ガイド

[READMEに戻る](../README.md) / [はじめてのセットアップ](quickstart.md)

VB-Audio Matrix（以下 **Matrix**）の **Virtual ASIO（VASIO）** スロットを使い、マイクなどの音源を本アプリ（もふボイス）に通してから、ヘッドホン・仮想出力・別 ASIO へ送ります。

Voicemeeter の **Insert** に相当する「1本の処理ループ」は、Matrix では **ルーティンググリッド上の VASIO 入出力** で作ります。

## 信号の流れ（基本構成）

```text
音源（物理 ASIO / Windows 入力 / VAIO など）
        ↓  ルーティンググリッド
   VASIO 入力 チャンネル 1
        ↓  （ASIO で接続）
   本アプリ（入力1 → 出力1）
        ↓
   VASIO 出力 チャンネル 1
        ↓  ルーティンググリッド
   ヘッドホン / VAIO / 別アプリへ
```

本アプリは **ASIO 入力チャンネル1** と **出力チャンネル1** だけを使います。Matrix 側でも **VASIO の ch1 → ch1** で送受信を揃えてください。

## 事前準備

1. [VB-Audio Matrix](https://vb-audio.com/Matrix/) をインストールし、指示に従って **再起動** します。
2. 使用するオーディオインターフェース（または Windows マイク）を、Matrix の **ASIO** または **WINDOWS** スロットに割り当てられる状態にします。
3. 本アプリの `MofuVoice.exe` を用意します。

## 1. Matrix のクロックとバッファ

1. Matrix を起動し、**Menu → System settings**（または `Ctrl + ,`）を開きます。
2. **Preferred main sample rate を 48000 Hz** にします。
3. **ASIO バッファ** はまず **512 samples** 程度に揃えます（本アプリの **バッファサイズ（サンプル）= 512** と合わせやすいです）。
4. **マスタークロック** は、遅延を抑えたい場合は **使用する物理 ASIO デバイス** にするとよいです（[マニュアル: Master Clock](https://vb-audio.com/Matrix/VBMatrix_UserManual.pdf)）。Virtual ASIO はマスターに **厳密同期（Strict）** されるため、レートとバッファの不一致はクリップや無音の原因になります。

## 2. Virtual ASIO スロットを有効化

Matrix には複数の仮想 ASIO があります（[VASIO スロットの説明](https://vb-audio.com/Matrix/VBMatrix_UserManual.pdf)）。

| スロット例 | Windows に見える名前（例） | クライアント数 | 本構成での目安 |
| --- | --- | --- | --- |
| **VASIO128** | `VB-Matrix VASIO-128` | **1** | **推奨**（本アプリだけが独占しやすい） |
| **VASIO8** | `VB-Matrix VASIO-8` | 最大 4 | 他 ASIO アプリと VASIO-8 を共有する場合 |
| **VASIO64A / 64B** | `VB-Matrix VASIO-64A` など | 2 | 複数 DAW 併用時 |

手順:

1. Matrix 画面上部の **VASIO** スロットで、使うスロット（初回は **VASIO128** 推奨）の **Online** を **Ctrl + クリック** して有効化します。
2. 有効化したスロットが **Routing Grid** のソース／ターゲットに現れることを確認します。

## 3. ルーティンググリッド（マイク → 処理 → 出力）

以下は **物理 ASIO の入力1（マイク）** を処理して **同じ ASIO のヘッドホン出力** に戻す例です。マイクが Windows デバイスの場合は、ソースを **WIN.IN**、出力を **WIN.OUT** や **VAIO** に読み替えます。

1. **Menu → Routing Grid**（または Matrix ロゴ）を開きます。
2. **ソース** にマイク側（例: `ASIOxxx` の **Input 1**）、**ターゲット** に `VASIO128` の **Input 1** を選び、交差セルを **Ctrl + クリック** で接続します。
3. **ソース** を `VASIO128` の **Output 1**、**ターゲット** をヘッドホン側（例: `ASIOxxx` の **Output 1**）にし、同様に接続します。
4. **VASIO の Output 1 を VASIO の Input 1 に戻すルート** を作らないでください（ハウリングや無音の原因になります）。

モノラル1本のときは、グリッドでも **チャンネル1だけ** をつなぎ、不要な L/R ペアはつながない方が分かりやすいです。

### 配信・通話（VAIO）への例

- 処理後の声を Discord / OBS に送る: `VASIO128 Output 1` → **VAIO** スロットの録音側（Line in 系ピン）→ 各アプリで **VB-Audio Matrix VAIO** をマイクに指定します。
- VAIO 内部バッファが大きいと遅延が増えます。途切れがある場合は [VAIO Latency メニュー](https://vb-audio.com/Matrix/VBMatrix_UserManual.pdf) を調整し、安定と低遅延のバランスを取ります。

## 4. 本アプリの設定

1. Matrix で上記ルーティングを済ませた状態で GUI を起動します。
2. **設定 → 音声デバイス** の **入出力デバイス** で、有効にした VASIO 名（例: **`VB-Matrix VASIO-128`**）を選びます。
3. **詳細設定** の **バッファサイズ（サンプル）** を **512** にして、**設定を保存** → **音声を開始** を押します。
4. ピッチ0で原音が出力されることを確認してから、**音声を停止** → パラメータ変更 → **設定を保存** → **音声を開始** で聞き比べます。

他のプログラムが同じ VASIO ドライバを既に開いていると、入出力デバイスに出ない・開けないことがあります。VASIO128 は **1クライアント** のため、本アプリ実行中は同じ `VB-Matrix VASIO-128` を別アプリで開かないでください。

## 5. Voicemeeter と Matrix を併用する場合（上級）

配信ミキサーとして Voicemeeter を使いつつ、Matrix で ASIO ルートを拡張する構成があります。VB-Audio では、**Voicemeeter の A1 を Matrix の Virtual ASIO に接続し、Matrix の ASIO スロットを Voicemeeter の Insert に渡す** ような組み合わせが紹介されています（詳細は [The Voicemeeter And Matrix Wedding](https://www.youtube.com/results?search_query=voicemeeter+matrix+wedding) など）。

本アプリをその中に置く場合の要点:

1. **どの段でボイスチェンジするか** を1か所に決める（Voicemeeter Insert **か** Matrix VASIO ループのどちらか。両方に同じマイクを並列接続しない）。
2. 本アプリが接続する ASIO 名は、**Voicemeeter Insert** か **VB-Matrix VASIO-*** のどちらか一方に固定する。
3. 全体の **48 kHz** と **バッファサイズ** を一本化する。

初めて Matrix を使う場合は、**Voicemeeter なしで VASIO128 ループだけ** を通してから、併用構成に広げることを推奨します。

## 6. よくある問題

| 症状 | 確認すること |
| --- | --- |
| VASIO が 入出力デバイス一覧にない | Matrix で VASIO スロットが **Online** か、Matrix が起動しているか |
| 開始時にレートエラー | System settings の **48 kHz**、マスターデバイス側のレート |
| 無音 | グリッドで **マイク → VASIO In1** と **VASIO Out1 → 出力** の両方があるか、ch1 だけつながっているか |
| ハウリング・二重音 | VASIO 出力が同じ VASIO 入力やマイク入力に戻っていないか、Windows マイクと処理済み VAIO をアプリが同時に使っていないか |
| 音切れ | 本アプリ / ASIO のバッファサイズを大きくする、グリッド上の不要な高ゲインルートを減らす |
| Coconut 版 | VASIO-256 などチャンネル数の多いドライバでも、本アプリは **入出力1本** のみ使用 |

## 参考リンク

- [VB-Audio Matrix 製品ページ](https://vb-audio.com/Matrix/)
- [VBMatrix ユーザーマニュアル（PDF）](https://vb-audio.com/Matrix/VBMatrix_UserManual.pdf)
- [本リポジトリ README（アプリ共通の操作・トラブルシュート）](../README.md)
- Voicemeeter Insert を使う場合: [Voicemeeter 設定ガイド](./voicemeeter.md)
