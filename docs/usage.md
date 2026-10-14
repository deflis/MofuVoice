# 詳細な使い方

[README](../README.md) / [利用ガイド一覧](README.md)

デバイス設定、音声の調整、設定ファイル、トラブル対処、CLIの詳しい説明です。初めて使う場合は、[READMEの「GUIでの使い方」](../README.md#guiでの使い方)から始めてください。

## 音声バックエンド

音声入出力にはCPALを使用し、既定は自動選択で、Windowsに登録ASIOドライバがある場合は直接ASIOを優先します。それ以外はCPAL経由でWindowsのWASAPI、macOSのCoreAudio、LinuxのALSAを使います。スペアナのウィンドウはWindows専用です。LinuxのビルドにはALSA開発パッケージ（Debian/Ubuntuでは`libasound2-dev`）が必要です。

CPALでは48 kHzの共通f32/i32/i16形式を検査し、入力と出力の間に事前確保キューを使います。キュー遅延はDSP遅延とは別に表示します。別デバイスのクロック差は補正せず、キュー不足・満杯やデバイス異常を検出して停止します。実機の往復遅延と聴取品質は個別に確認してください。

```sh
cargo run --bin rust-low-latency-voice-changer-cli -- --list-devices
cargo run --bin rust-low-latency-voice-changer-cli -- --backend cpal --device "INPUT" --output-device "OUTPUT" --frames 512
```

CPALではデバイスを省略するとOSの既定入力・出力を使います。ASIOではドライバ名を指定してください。`--backend cpal` / `--backend asio`で明示選択できます。実行ファイルのビルド構成については[ビルド・開発](../DEVELOPMENT.md)を参照してください。

## はじめに

48 kHzの音声入出力に対応したデバイスが必要です。実行ファイルを用意してから、次の手順で音声処理を開始します。WindowsのGUIにはWebView2 Runtimeが必要です。ソースコードから実行ファイルを作成する場合は、[ビルド手順](../DEVELOPMENT.md)を参照してください。

### 1. 起動する

`rust-low-latency-voice-changer-gui.exe` をダブルクリックします。コンソールウィンドウは開きません。メイン画面の **設定** を押すと、デバイスや声の設定を変更できます。InspectやStartを操作するときは、設定画面の **閉じる** でメイン画面へ戻ります。

### 2. デバイスと音声の接続を準備する

**音声API** は自動（ASIO優先）が既定です。ASIOが選ばれた場合は **ASIO device** を指定します。CPALでは **入力デバイス** と **出力デバイス** を選び、省略するとOSの既定デバイスを使います。次回からは前回保存した設定を復元します。

デバイス側の設定でサンプルレートを **48 kHz** にし、マイクの信号が届く **入力チャンネル** を選び、**出力チャンネル** を1つ以上チェックしてください。選択したすべての出力へ同じ処理済み音声を送り、その他の出力は無音にします。既定は入力1 → 出力1です。チャンネルの変更は次のStart時に反映されます。CLIでは `--input-channel 2 --output-channels 1,2` のように指定できます。デバイスに存在しないチャンネルは開始時にエラーになります。

出力1をヘッドホンなどのモニター先、または録音・通話・配信に使う音声入力へ接続します。接続方法はデバイスや音声ルーティング機能によって異なります。アプリは外部の接続設定を自動変更しません。出力を入力へ戻す接続は避けてください。

Voicemeeter（Banana / Potato）や VB-Audio Matrix でルーティングする場合は、[接続ガイド](README.md) を参照してください（[Voicemeeter](voicemeeter.md) / [VB-Matrix](vbmatrix.md)）。

### 3. 設定を確認する

最初は **Frames = 512**、**Pitch = 0**、**Preserve formants = オン**、**Formant shift = 0** のままで **Inspect** を押します。

下部の表示が `Inspection OK` になれば、デバイスの形式・レート・バッファサイズが対応しています。Inspectでは音声処理は始まりません。エラーの場合は表示内容を確認し、設定を直してから再度Inspectを押してください。

### 4. 元の声で接続を確かめる

**Start** を押し、マイクに向かって話します。下部に `Running` と表示され、出力先から元の声が聞こえることを確認してください。PitchとFormant shiftが両方0で、低域ノイズ除去・ディエッサー・Noise gate・EQ・Auto levelがオフのときは、入力をそのまま返します。

### 5. 声を変える

**Stop** を押し、停止が完了して設定欄が操作できるようになったら、**Pitch** を変更して **Start** を押します。

- `3`：声を3半音高くする
- `-3`：声を3半音低くする
- `0`：元の声の高さに戻す

小数も指定できます。まずは小さな変更で試し、聞きながら調整してください。設定を変えるたびにStop → 設定変更 → Startの順で操作します。

声の高さを保って響きを変えるには、Preserve formantsをオンにし、**Formant shift** を変更します。`3` は共鳴を高く、`-3` は低くし、`0` は元の共鳴を保持します。Pitchが0でも変更でき、Pitchと組み合わせても使えます。

## 設定項目

| 項目 | 使い方 |
| --- | --- |
| **音声API / Device** | 自動（ASIO優先）、CPAL、ASIOと、使用する入力・出力デバイスを選択します |
| **Frames** | 音声バッファのサイズ。既定512。デバイスが対応する値を指定します |
| **Pitch** | 声の高さ。−12〜+12半音、小数可。既定0 |
| **Preserve formants** | オンにすると、ピッチ変更時に元の声の響きを保つ補正を行います。既定オン |
| **Formant shift** | 声の共鳴の移動量。−12〜+12半音、小数可。既定0。非ゼロではPreserve formantsをオンにします |

Preserve formantsをオフにすると、声の高さとともに響きも変わります。同じPitchでオン・オフを切り替えて聞き比べると違いを確認できます。オフのときにFormant shiftを非ゼロにするとエラーになります。PitchとFormant shiftが両方0で、低域ノイズ除去・ディエッサー・Noise gate・EQ・Auto levelがオフのときは、この設定にかかわらず元の音声を返します。

大きなフォルマント移動では響きの幅や音量も変わり、歪みが生じる場合があります。まず±2〜3半音程度で聞きながら調整してください。

Framesを小さくすると音声処理の周期が短くなりますが、処理が間に合わず音が途切れる場合があります。まず対応するサイズで安定して使えることを確認し、必要に応じて小さくしてください。

## 低域ノイズ除去

**低域ノイズ除去（80 Hz）** をオンにすると、机やマイクスタンドの振動、空調の低い唸りを抑えます。80 Hz・12 dB/octのハイパスフィルターをゲートと声変換の前に適用します。既定はオフです。低い声の厚みも減るため、聞き比べながら使ってください。破裂音の低域成分は軽減できますが、マイク自体のクリップは修復できません。

CLIでは `--rumble-filter on` / `--rumble-filter off` で指定します。GUIではStop → 設定変更 → Startの順で切り替えます。設定はINIの `rumble_filter=true` / `false` に保存され、キーのない既存INIはオフとして読み込みます。先読みや音声バッファによる追加サンプル遅延はありませんが、周波数に応じた位相変化はあります。

## 声の音色（EQ）

声変換後、Auto levelの前に3バンドのピーキングEQを適用します。既定はオフです。**簡単モード**では「低域の厚み」「明瞭さ」「明るさ」を調整し、**プロ向けモード**では各バンドの周波数（40〜16000 Hz）、ゲイン（−12〜+12 dB）、Q（0.2〜8）と出力の追加減衰（−18〜0 dB）を指定できます。

まず0から1〜3 dBずつ調整してください。こもる声は低域の厚みを下げ、言葉がぼやける場合は明瞭さを上げます。明るさを上げてサ行が刺さる場合は戻してください。簡単モードの中央はそのバンドの補正なしです。モードを切り替えても全設定を保持し、簡単モードではプロ向けモードの周波数・Qを使ってゲインだけを変更します。

**EQだけリセット**は周波数・Qを既定値（250 Hz / Q 0.7、3000 Hz / Q 0.8、8000 Hz / Q 0.7）に戻し、ゲインと追加減衰を0にします。EQのオン・オフと選択モードは保持します。設定変更はStop → 変更 → Startの順で行います。オン・オフで比較するときは、ブースト時の減衰による音量差にも注意してください。

正のゲインの合計分をフィルター前に減衰し、音量の余裕を確保します。これはピーク値を保証するリミッターではありません。出力が小さく感じる場合は後段のAuto levelで調整できます。先読みや音声バッファによる追加サンプル遅延は0ですが、周波数に応じた位相変化はあります。EQオフ、または全ゲイン・追加減衰が0のとき、この段はサンプルのビットを保ってバイパスします。

CLIでは例えば `--eq on --eq-band1 250,-3,0.7 --eq-band2 3000,2,0.8 --eq-band3 8000,1,0.7` を指定します。バンドの値は `周波数,ゲイン,Q`、出力の追加減衰は `--eq-output -2`、GUIモードは `--eq-mode simple` / `pro` で指定します。モードは操作画面だけを変更し、DSPの処理は共通です。

GUI設定INIには以下の6キーが必要です。既存INIにキーがない場合は、追加するかINIを削除して既定値で起動してください。

```ini
eq=off
eq_mode=simple
eq_band1=250,0,0.7
eq_band2=3000,0,0.8
eq_band3=8000,0,0.7
eq_output=0
```

## サ行の刺さり（ディエッサー）

「さ・し・す」が耳に刺さるときは、**サ行をやわらかくする** をオンにしてください。まずは **簡単モード** の標準の位置で「さしすせそ」と話して聞き比べます。まだ刺さるときは「抑える強さ」を右へ、声がこもるときは左へ動かします。0では補正しません。設定はStop → 変更 → Startの順で適用します。

**プロ向けモード** では、分割周波数・しきい値・最大減衰量・アタック・リリースを調整できます。モードを切り替えるだけでは音も数値も変わりません。簡単モードでもプロ向けで設定した詳細値を使い、抑える強さ（最大減衰量）だけを変更します。**ディエッサーだけリセット** は詳細値と強さを標準に戻し、オン・オフと操作モードは維持します。

| プロ向けの設定 | 標準 | 範囲 | CLI |
| --- | --- | --- | --- |
| 分割周波数 | 5000 Hz | 2000〜12000 Hz | `--deesser-frequency` |
| しきい値 | −24 dBFS | −60〜0 dBFS | `--deesser-threshold` |
| 最大減衰量 | 6 dB | 0〜12 dB | `--deesser-range` |
| アタック | 0.5 ms | 0.1〜20 ms | `--deesser-attack` |
| リリース | 60 ms | 10〜500 ms | `--deesser-release` |

声変換と自動音量調整の後で、一次相補フィルターで分離した高域のRMS音量と全体に対する高域の比率を検出し、高域だけを動的に抑えます。最大減衰量は上限なので、常にその強さで減衰するわけではありません。先読み・追加バッファはなく、検出とゲインの応答時間および減衰中の位相変化があります。声やマイクによって効き方が変わり、息や明るい母音にも反応する場合があります。

CLIでは `--deesser on` / `--deesser off`、表示モードは `--deesser-mode simple` / `pro` を指定します。INIには `deesser`、`deesser_mode`、`deesser_frequency`、`deesser_threshold`、`deesser_range`、`deesser_attack`、`deesser_release` を保存します。新しいキーのない既存INIも、簡単モードと標準の詳細値で読み込めます。オン・オフの既定はオフです。

## 自動音量調整

**Auto level** をオンにすると、声の変換後に緩やかなRMS音量補正とソフトニーコンプレッサーを適用します。既定ではオフです。まず既定値で使い、必要なときだけ **詳細設定** を開いて調整してください。**既定値に戻す** は詳細設定を既定値に戻します。オフにすると詳細設定を保持したまま、この処理をバイパスします。

| 詳細設定 | 既定値 | 範囲・効果 | CLI |
| --- | --- | --- | --- |
| Target RMS dB | −20 dBFS | −36〜−6。会話の目標RMS音量 | `--level-target` |
| Max gain dB | +9 dB | 0〜18。小声の最大増幅量 | `--level-max-gain` |
| Activity floor dB | −50 dBFS | −90〜−30。これ以下では音量補正を更新しない。目標音量より低くする | `--level-floor` |
| Level response ms | 1500 ms | 100〜10000。音量補正の応答。長いほどゆっくり | `--level-response` |
| Comp. threshold dB | −12 dBFS | −36〜0。圧縮が始まるピーク音量。ニー幅は6 dB | `--compress-threshold` |
| Ratio | 3:1 | 1〜20。1ではコンプレッサーの圧縮なし | `--compress-ratio` |
| Attack ms | 5 ms | 0.1〜100。大声の圧縮速度 | `--compress-attack` |
| Release ms | 150 ms | 10〜2000。圧縮とピーク保護の回復速度 | `--compress-release` |

GUIの詳細値は保存・復元され、CLI指定は各項目だけを上書きします。設定変更はStop → 変更 → Startの順です。GUI設定INIには `auto_level` と、表の順に `level_target_db`、`level_max_gain_db`、`level_floor_db`、`level_response_ms`、`compress_threshold_db`、`compress_ratio`、`compress_attack_ms`、`compress_release_ms` を保存します。既存のINIにはこれらのキーが必要です。起動エラーが出た場合はキーを追加するか、INIを削除して既定値で起動してください。

```powershell
.\target\release\rust-low-latency-voice-changer-cli.exe --backend asio --device "ASIOデバイス名" --auto-level on
.\target\release\rust-low-latency-voice-changer-cli.exe --backend asio --device "ASIOデバイス名" --auto-level on --level-target -18 --level-max-gain 6 --compress-ratio 4
```

無音では補正量を保持し、下限を超えるノイズも声として扱うため、発話判定の下限と最大増幅量を環境に合わせて調整してください。前段のゲートとも併用できます。音量補正の減衰は最大18 dB、後段のピーク保護は−1 dBFSで固定です。突発ピークは即時にゲインを下げるため、強い過渡音では波形に影響があります。入力やピッチ変換ですでにクリップした波形は復元しません。

この処理による追加のサンプル遅延は0です。応答時間はゲイン変化の速度であり、音声を遅らせる時間ではありません。ピッチ変換・ドライバ・ハードウェアの遅延は別に残ります。実機での聞き比べ・会話のしやすさ・総遅延は確認が必要です。

## 停止と終了

**Stop** で音声処理を停止します。停止後にデバイスや声の設定を変更し、Startで再開できます。処理中は設定欄が無効になります。

ウィンドウを閉じると音声処理を停止して終了します。ドライバの解放が完了するまで、閉じる処理に時間がかかる場合があります。

ウィンドウを閉じるときに、Device、Frames、Pitch、Preserve formants、Formant shiftをGUIの実行ファイルと同じフォルダーの `settings.ini` に自動保存します。StartやInspectを押していない変更も保存します。次回起動時に設定を復元しますが、音声処理はStartを押すまで始まりません。

INIファイルはUTF-8で、次の形式です。`device` は未選択の場合に空欄になります。`preserve_formants` は `true` または `false` で指定します。

```ini
[gui]
backend=asio
output_device=
device=Example ASIO Device
frames=512
pitch=0
formant_shift=0
preserve_formants=true
rumble_filter=false
deesser=false
deesser_mode=simple
deesser_frequency=5000
deesser_threshold=-24
deesser_range=6
deesser_attack=0.5
deesser_release=60
eq=off
eq_mode=simple
eq_band1=250,0,0.7
eq_band2=3000,0,0.8
eq_band3=8000,0,0.7
eq_output=0
gate=false
gate_threshold=auto
gate_attack_ms=5
gate_release_ms=80
auto_level=false
level_target_db=-20
level_max_gain_db=9
level_floor_db=-50
level_response_ms=1500
compress_threshold_db=-12
compress_ratio=3
compress_attack_ms=5
compress_release_ms=150
```

入力値が不正な場合や保存に失敗した場合はエラーを表示し、ウィンドウを開いたままにします。問題を直してから閉じ直してください。保存ファイルが壊れている場合は起動時にエラーを表示します。ファイルを修正するか削除すると、初期設定で起動できます。

## 困ったとき

| 状況 | 確認すること |
| --- | --- |
| Deviceの一覧にデバイスがない | 音声APIの選択とOSのデバイス接続を確認し、ASIOの場合は対応ドライバをインストールします |
| Inspectでレートのエラーが出る | デバイス側を48 kHzに設定し直します。アプリはレートを変更しません |
| Framesが非対応と表示される | 下部に表示されるバッファ範囲・推奨値を確認し、対応するサイズを指定します |
| 形式が非対応と表示される | 全入出力チャンネルが同じlittle-endian f32、i32、またはi16形式に対応するデバイスが必要です |
| Startしても声が聞こえない | 選択した入力チャンネルにマイク音声が届いているか、選択した出力チャンネルが聞きたい出力先へ接続されているか確認します |
| 元の声と変えた声が重なる | デバイスの直接モニターや、未処理マイクを出力する別経路を確認します |
| 音が途切れる・異常で停止する | 下部のエラーを確認し、対応範囲内でFramesを大きくして再開します |
| 声が歪む・揺れる | マイクの入力レベルを下げ、Pitchの変更幅を小さくして試します |

下部には入力信号の累計、処理回数、ドライバのレイテンシ、異常などが表示されます。`nonzero input samples` が増えていれば入力信号が届いていますが、出力先で聞こえるかは実際に確認してください。

PitchとFormant shiftが両方0で、低域ノイズ除去・ディエッサー・Noise gate・EQ・Auto levelがオフならDSPは遅延なしのバイパスです。ゲートと自動音量調整も追加サンプル遅延は0ですが、オンの場合は音量を変更します。ピッチまたはフォルマント変換では約5.33 msのDSP遅延に、デバイスや音声の接続経路の遅延が加わります。Framesを小さくしてもDSPの合成遅延（256サンプル）は減りません。表示されるドライバのレイテンシは、マイクから出力先までの総遅延とは異なります。

## CLIでの操作

設定確認や時間を指定した実行には、補助ツールの `rust-low-latency-voice-changer-cli.exe` を使用できます。

```powershell
.\target\release\rust-low-latency-voice-changer-cli.exe --list-devices
.\target\release\rust-low-latency-voice-changer-cli.exe --backend asio --device "ASIOデバイス名" --inspect
.\target\release\rust-low-latency-voice-changer-cli.exe --backend asio --device "ASIOデバイス名" --pitch 3 --seconds 60
.\target\release\rust-low-latency-voice-changer-cli.exe --backend asio --device "ASIOデバイス名" --pitch 0 --formant-shift -3
```

`--seconds` を省略するとEnterで停止します。`--frames`、`--pitch`、`--formant preserve` または `--formant off`、`--formant-shift` で設定を指定できます。フォルマントシフトは−12〜+12半音、小数可、既定0です。詳細は `--help` を参照してください。

GUIにも `--backend`、`--output-device`、`--device`、`--frames`、`--pitch`、`--formant`、`--formant-shift` で起動時の値を渡せます。指定した項目は保存済みの値より優先し、指定しない項目は復元します。通常はウィンドウ内で設定できます。CLIはGUIの保存設定を使用・変更しません。
