# Rust Low-Latency Voice Changer

Windows向けのリアルタイムボイスチェンジャーです。GUIで音声デバイスを選び、声の高さとフォルマントの移動量を個別に設定できます。通常の操作はGUIで行い、CLIは設定確認や自動実行に利用できます。

音声変換には、自前のFFTと位相同期を使うスペクトル方式を使用します。低い声を上げたときの不要な低域成分を抑えることを検証した実験的な方式です。変換時のDSP遅延は512サンプル（48 kHzで約10.67 ms）です。解析窓は2048サンプルのまま、右寄せの短い合成窓で遅延を抑えます。マイクからヘッドホンまでの総遅延10 ms以下を今後の目標とし、現在は品質検証を優先しています。実際の会話音声の自然さは聴取で確認する必要があります。

## はじめに

Windows x64と、48 kHzのフルデュプレックス入出力に対応したASIOドライバが必要です。実行ファイルを用意してから、次の手順で音声処理を開始します。操作画面はReact + TypeScript / Tauri 2、スペアナはWin32の別ウィンドウです。WindowsではWebView2 Runtimeが必要です。ソースコードから実行ファイルを作成する場合は、末尾の「ビルド」を参照してください。

### 1. 起動する

`rust-low-latency-voice-changer-gui.exe` をダブルクリックします。コンソールウィンドウは開きません。

### 2. デバイスと音声の接続を準備する

**ASIO device** の一覧から使用するASIOデバイスを選びます。初回起動時は未選択です。次回からは前回保存した設定を復元します。

デバイス側の設定でサンプルレートを **48 kHz** にし、マイクの信号を **入力1** に接続してください。アプリは入力1を処理して **出力1** へ送り、その他の出力は無音にします。

出力1をヘッドホンなどのモニター先、または録音・通話・配信に使う音声入力へ接続します。接続方法はデバイスや音声ルーティング機能によって異なります。アプリは外部の接続設定を自動変更しません。出力を入力へ戻す接続は避けてください。

Voicemeeter（Banana / Potato）や VB-Audio Matrix でルーティングする場合は、[設定ガイド（docs/）](docs/README.md) を参照してください（[Voicemeeter](docs/voicemeeter.md) / [VB-Matrix](docs/vbmatrix.md)）。

### 3. 設定を確認する

最初は **Frames = 512**、**Pitch = 0**、**Preserve formants = オン**、**Formant shift = 0** のままで **Inspect** を押します。

下部の表示が `Inspection OK` になれば、デバイスの形式・レート・バッファサイズが対応しています。Inspectでは音声処理は始まりません。エラーの場合は表示内容を確認し、設定を直してから再度Inspectを押してください。

### 4. 元の声で接続を確かめる

**Start** を押し、マイクに向かって話します。下部に `Running` と表示され、出力先から元の声が聞こえることを確認してください。PitchとFormant shiftが両方0で、Noise gateとAuto levelがオフのときは、入力をそのまま返します。

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
| **Device** | 使用するASIOデバイスを選択します |
| **Frames** | 音声バッファのサイズ。既定512。デバイスが対応する値を指定します |
| **Pitch** | 声の高さ。−12〜+12半音、小数可。既定0 |
| **Preserve formants** | オンにすると、ピッチ変更時に元の声の響きを保つ補正を行います。既定オン |
| **Formant shift** | 声の共鳴の移動量。−12〜+12半音、小数可。既定0。非ゼロではPreserve formantsをオンにします |

Preserve formantsをオフにすると、声の高さとともに響きも変わります。同じPitchでオン・オフを切り替えて聞き比べると違いを確認できます。オフのときにFormant shiftを非ゼロにするとエラーになります。PitchとFormant shiftが両方0で、Noise gateとAuto levelがオフのときは、この設定にかかわらず元の音声を返します。

大きなフォルマント移動では響きの幅や音量も変わり、歪みが生じる場合があります。まず±2〜3半音程度で聞きながら調整してください。

Framesを小さくすると音声処理の周期が短くなりますが、処理が間に合わず音が途切れる場合があります。まず対応するサイズで安定して使えることを確認し、必要に応じて小さくしてください。

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
.\target\release\rust-low-latency-voice-changer-cli.exe --device "ASIOデバイス名" --auto-level on
.\target\release\rust-low-latency-voice-changer-cli.exe --device "ASIOデバイス名" --auto-level on --level-target -18 --level-max-gain 6 --compress-ratio 4
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
device=Example ASIO Device
frames=512
pitch=0
formant_shift=0
preserve_formants=true
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
| Deviceの一覧にデバイスがない | 使用するデバイスのASIOドライバがインストールされているか確認し、アプリを起動し直します |
| Inspectでレートのエラーが出る | デバイス側を48 kHzに設定し直します。アプリはレートを変更しません |
| Framesが非対応と表示される | 下部に表示されるバッファ範囲・推奨値を確認し、対応するサイズを指定します |
| 形式が非対応と表示される | 全入出力チャンネルが同じlittle-endian f32、i32、またはi16形式に対応するデバイスが必要です |
| Startしても声が聞こえない | マイクが入力1に届いているか、出力1が聞きたい出力先へ接続されているか確認します |
| 元の声と変えた声が重なる | デバイスの直接モニターや、未処理マイクを出力する別経路を確認します |
| 音が途切れる・異常で停止する | 下部のエラーを確認し、対応範囲内でFramesを大きくして再開します |
| 声が歪む・揺れる | マイクの入力レベルを下げ、Pitchの変更幅を小さくして試します |

下部には入力信号の累計、処理回数、ドライバのレイテンシ、異常などが表示されます。`nonzero input samples` が増えていれば入力信号が届いていますが、出力先で聞こえるかは実際に確認してください。

PitchとFormant shiftが両方0で、Noise gateとAuto levelがオフならDSPは遅延なしのバイパスです。ゲートと自動音量調整も追加サンプル遅延は0ですが、オンの場合は音量を変更します。ピッチまたはフォルマント変換では約10.67 msのDSP遅延に、デバイスや音声の接続経路の遅延が加わります。Framesを小さくしてもDSPの合成遅延（512サンプル）は減りません。表示されるドライバのレイテンシは、マイクから出力先までの総遅延とは異なります。

## CLIでの操作

設定確認や時間を指定した実行には、補助ツールの `rust-low-latency-voice-changer-cli.exe` を使用できます。

```powershell
.\target\release\rust-low-latency-voice-changer-cli.exe --list-devices
.\target\release\rust-low-latency-voice-changer-cli.exe --device "ASIOデバイス名" --inspect
.\target\release\rust-low-latency-voice-changer-cli.exe --device "ASIOデバイス名" --pitch 3 --seconds 60
.\target\release\rust-low-latency-voice-changer-cli.exe --device "ASIOデバイス名" --pitch 0 --formant-shift -3
```

`--seconds` を省略するとEnterで停止します。`--frames`、`--pitch`、`--formant preserve` または `--formant off`、`--formant-shift` で設定を指定できます。フォルマントシフトは−12〜+12半音、小数可、既定0です。詳細は `--help` を参照してください。

GUIにも `--device`、`--frames`、`--pitch`、`--formant`、`--formant-shift` で起動時の値を渡せます。指定した項目は保存済みの値より優先し、指定しない項目は復元します。通常はウィンドウ内で設定できます。CLIはGUIの保存設定を使用・変更しません。

## ビルド

ソースコードからビルドするには、RustのWindows x64 MSVCツールチェーン、Visual C++ Build ToolsとWindows SDK、libclang、ASIO SDK、Node.js 26とWebView2 Runtimeが必要です。Rust、Node.js、pnpmのバージョンは `mise.toml` で固定しています。oxlintとoxfmtは開発依存としてpnpmでインストールします。

ASIO SDK は `.tools/asiosdk` に置きます（`common/asio.h` があること）。`.cargo/config.toml` と `mise.toml` の `CPAL_ASIO_DIR` がこのパスを指します。次のスクリプトで Steinberg から取得できます（SDK 自体は再配布しないでください）。libclang が PATH にない場合は `mise.local.toml` か環境変数で `LIBCLANG_PATH` を指定してください。

```powershell
mise trust
mise install
./scripts/fetch-asio-sdk.ps1
pnpm install --frozen-lockfile
pnpm desktop:build
```

Pull request と `master` への push では、GitHub Actions が `mise run check`、`cargo test`、Release の `spectral_quality` を実行します。`master` への push では、加えて GUI / CLI の Release バイナリを成果物としてアップロードします。

`target/release/` にGUIとCLIの実行ファイルが生成されます。配布するGUIにはReactの静的ファイルが埋め込まれます。

検証コマンドは `mise.toml` のタスクで管理します。`mise run check` でRustとフロントエンドの整形・lint・型を検証し、`mise run fmt` で整形、`mise run fmt:check` で整形チェック、`mise run lint` でClippyとoxlintを実行します。フロントエンドだけなら `mise run check:web`、型チェックだけなら `mise run typecheck` を使います。Clippyに必要なReactのビルドはタスクの依存関係で先に実行します。

React画面の開発には `pnpm desktop:dev` を使います。音声処理を含む動作確認はTauriで行い、`pnpm dev` は画面側の開発サーバーだけを起動します。CLIだけなら `cargo build --release --locked` でビルドできます。

操作画面は `frontend/src/App.tsx`、IPCの型と呼び出しは `frontend/src/api.ts`、Tauriのコマンドは `src/gui.rs`、ASIOセッションの管理は `src/session.rs` にあります。スペアナの描画は `native/spectrum.cpp`、Rustとの接続は `src/native_spectrum.rs`、解析は `src/spectrum.rs` に分離しています。音声サンプルとスペアナ画像をWebViewへ転送しません。

<details>
<summary>開発者向けの検証</summary>

GUIとCLIのDSPは `src/pitch.rs` です。2048サンプルの解析窓と512サンプルの右寄せ合成窓（積がHann）、128サンプルのホップ、スペクトルピークに対する位相同期を使い、各ピークの周囲の成分をまとめて移動します。前のフレームの近接ピークから位相を引き継ぎ、ピーク位置が変わっても発振の位相を連続させます。移動量には位相差から測定した周波数を使い、隣接する周波数ビンへ補間して丸めによる変化を抑えます。フォルマントは実ケプストラムの平滑化で包絡を推定し、指定した包絡に近づくよう補正します。包絡補正のゲインを制限し、最終出力は正規化範囲に制限します。大きな変換、高い入力レベル、子音や声の急変では歪みや不自然さが生じる可能性があります。解析・合成用領域とFFTの表は起動前に確保します。

`tests/spectral_quality.rs` は低域の不要成分、ピッチ精度、共鳴の移動、連続的なピッチ変化時の音量・位相の揺れ、ブロック分割、引数検証、バイパスを検証します。単音の結果は会話音声の品質保証ではありません。DSPの速度は `spectral` ベンチマークで測定します。

```powershell
cargo check --features desktop --locked
cargo test --features desktop --locked
cargo test --release --locked --test spectral_quality
mise run check
cargo bench --locked --bench spectral
```

実機の音声品質と総遅延は聴取・録音で確認し、ログはバージョン管理に含めません。

実声の比較には、48 kHz・モノラル・PCM16のWAVを用意し、次の例を実行します。既存の出力ファイルは上書きしません。

```powershell
cargo run --release --locked --example pitch_render -- voice.wav logs/listening-01 7 3
```

`input.wav` と変換後の `processed.wav` を出力します。変換後の音声は既知のDSP遅延を除いて書き出します。音量は自動調整しません。母音、サ行、息、低い発声から高い発声への変化を同じ箇所で比較し、入力レベルと変換量、クリッピング、聴取結果を記録してください。

</details>
