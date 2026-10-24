# Voicemeeter シリーズ 設定ガイド

[READMEに戻る](../README.md) / [はじめてのセットアップ](quickstart.md)

Voicemeeter **Banana** または **Potato** の **Insert Virtual ASIO** を使い、マイク入力に本アプリ（もふボイス）を差し込みます。

> **VoiceMeeter（無料版）** には Insert ドライバがありません。Banana または Potato が必要です。

## 信号の流れ（概要）

```text
マイク → Voicemeeter ハードウェア入力ストリップ
              ↓（Insert 送信）
         本アプリ（ピッチ / フォルマント処理）
              ↓（Insert 返送）
         同じストリップの信号が「処理後」に置き換わる
              ↓
         A1（ヘッドホン）や B1/B2/B3（アプリ用仮想出力）へルーティング
```

Insert が有効なとき、そのストリップでは **生のマイクと処理後の音はミックスされません**。返送された ASIO 出力がストリップの音になります（[公式ガイド](https://voicemeeter.com/user-guide-connect-audio-apps-to-the-voicemeeter-insert-driver/)）。

本アプリは **ASIO の入力チャンネル1** を読み、**出力チャンネル1** に書き込みます。Voicemeeter の Insert ドライバでは、ハードウェア入力 **1** の **左** に相当する送受信（表示名は **In#1 Left** など）を使うのが基本です。

## 事前準備

1. [VB-Audio VoiceMeeter Banana または Potato](https://vb-audio.com/Voicemeeter/) をインストールし、再起動後に起動できることを確認します。
2. マイクを Voicemeeter に入れる方法を決めます。
   - **USB マイクのみ**: Windows の録音デバイスとしてストリップに割り当てる、など。
   - **ASIO インターフェース**: 出力 **A1** にその ASIO を選び、**System settings / Options** の **PATCH ASIO Inputs to strips** でマイクの物理入力番号を **ストリップ1** に割り当てます（[マイクと ASIO の公式手順](https://voicemeeter.com/setup-a-mic-with-your-asio-audio-interface/)）。
3. 本アプリの実行ファイル（`MofuVoice.exe`）を用意します。

## 1. Voicemeeter の基本設定

### サンプルレートとバッファ

1. **Menu → System settings / Options…** を開きます。
2. **サンプルレートを 48000 Hz** に揃えます（A1 に ASIO を使っている場合は、多くの場合インターフェース側のレートに追従します）。
3. バッファサイズは、まず **512 samples** 前後を目安にします（本アプリの **バッファサイズ（サンプル）= 512** と合わせやすい値です）。

### マイクをストリップ1へ

1. **Hardware Input 1** にマイクが入るようにします。
2. モノラルマイクならストリップの **MONO** をオンにします。
3. 配信・通話アプリへ送る場合は **B1**（Potato では **B2** / **B3** も可）をオンにします。ヘッドホンで自分の声を聞く場合は **A1** もオンにします。

### Insert の送信位置（Pre / Post）

**System settings** の Insert 関連で、送信・返送位置を選べます。

| 設定 | 意味 | 本アプリとの相性 |
| --- | --- | --- |
| **Pre-FX / Pre-Fader**（既定に近い） | Voicemeeter 内の EQ・コンプなど **前** の音を外部へ送る | ボイスチェンジだけを本アプリに任せたいときに向く |
| **Post-FX** | Voicemeeter のエフェクト **後** を送る | VM 側でコンプ等をかけたあとピッチ変換する |

まずは **Pre-Fader** で送受信し、問題なければ Post-FX を試してください。

## 2. Insert ドライバと PATCH INSERT

### エディションごとの ASIO 名

| 製品 | Insert ASIO ドライバ名（一覧の例） | Insert チャンネル数 |
| --- | --- | --- |
| **Banana** | `Voicemeeter Insert Virtual ASIO` | 最大 22 |
| **Potato** | `Voicemeeter Potato Insert Virtual ASIO` | 最大 34 |

Potato では Banana 用の Insert 名ではなく、**Potato 用**を選びます。PATCH INSERT も Potato 側で有効にします。

### PATCH INSERT を有効化

1. **System settings / Options** の **PATCH INSERT** で、使うストリップの **左チャンネル**（ストリップ1なら **In#1 Left** に対応する返送）をオンにします。
2. メイン画面のストリップ左上に **「i」** が出たら、そのストリップで Insert 返送が有効です。**「i」** はストリップ単位の Insert 切り替えにも使えます。

返送を有効にしないと、本アプリは Voicemeeter から音を受け取れても、処理結果がストリップに戻らず無音や生音のままになります。

## 3. 本アプリの設定

1. **Voicemeeter を起動した状態で** 本アプリ GUI を起動します（Insert ドライバは Voicemeeter が提供します）。
2. **設定 → 音声デバイス** の **入出力デバイス** で、上表の **Insert Virtual ASIO** を選びます。
3. **詳細設定** の **バッファサイズ（サンプル）** を **512** にします。初回はピッチとフォルマントを0にします。
4. **設定を保存** を押します。
5. **音声を開始** を押してマイクに向かって話し、A1 または B1 経由で声が聞こえることを確認します。
6. **音声を停止** を押してからピッチなどを変更し、**設定を保存** → **音声を開始** で聞き比べます。

基本的な操作の流れは [はじめてのセットアップ](quickstart.md) を参照してください。

## 4. 配信・通話への接続例

| 目的 | Voicemeeter 側 | Windows / アプリ側 |
| --- | --- | --- |
| ヘッドホン監聴 | ストリップ1 → **A1** | A1 = ヘッドホン用デバイス |
| OBS・Discord 等 | ストリップ1 → **B1** | 録音デバイス = **VoiceMeeter Output (VB-Audio Voicemeeter VAIO)** など B1 に対応するデバイス |

Discord などで **マイクを Windows デバイス直指定** したままにすると、処理前の声が混ざります。仮想出力（B1 等）だけをマイクに指定してください。

## 5. よくある問題

| 症状 | 確認すること |
| --- | --- |
| 入出力デバイス一覧に Insert がない | Banana/Potato か、Voicemeeter が起動しているか。Banana 名と Potato 名を取り違えていないか |
| 開始時にレートエラー | System settings と A1 デバイスを **48 kHz** に |
| 開始しても無音 | PATCH INSERT / ストリップの **「i」**、マイクがストリップ1に入っているか、本アプリが **入力1・出力1** であること |
| 変換前と変換後が二重に聞こえる | アプリが **生マイク** を同時に使っていないか、A1 のハードウェアモニターで生音が出ていないか |
| 音切れ・停止 | バッファサイズを大きくする、Voicemeeter のバッファを増やす、CPU 負荷を下げる |
| 別の DAW と同時に使えない | Insert は **単一クライアント** 想定。本アプリと DAW で同じ Insert を同時開かない |

## 参考リンク

- [Connect audio apps to the VoiceMeeter Insert Driver](https://voicemeeter.com/user-guide-connect-audio-apps-to-the-voicemeeter-insert-driver/)
- [Setup a Mic with your ASIO audio interface](https://voicemeeter.com/setup-a-mic-with-your-asio-audio-interface/)
- [VoiceMeeter Banana ユーザーマニュアル（PDF）](https://vb-audio.com/Voicemeeter/VoicemeeterBanana_UserManual.pdf)
- [VoiceMeeter Potato ユーザーマニュアル（PDF）](https://vb-audio.com/Voicemeeter/VoicemeeterPotato_UserManual.pdf)
