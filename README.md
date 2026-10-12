# Rust Low-Latency Voice Changer

## DSP性能の検証

LPC自己相関を4 lagずつ計算し、各lagの加算順序を維持して処理時間を削減した。変更前DSPとのF32全ビット・I32/I16出力一致を `cargo test --release --locked --offline --test dsp_equivalence` で検証できる。既存の共鳴・ピッチ・再構成テストも継続する。

`. .\scripts\dev-env.ps1` の後、`cargo bench --bench dsp --locked --offline` で変更前後を同一プロセス内で比較する。48 kHz、512 frames、formant preserve、−12/−3/0/+3/+12半音、各11組を交互順序で測定し、各設定で処理時間の短縮率中央値10%以上を要求する。測定条件と結果はローカルに記録し、ログはバージョン管理に含めない。これはDSPのCPU処理時間であり、実機の総往復遅延や聴取品質は別途評価する。

Phase 4のLPCフォルマント補正を実装。`--pitch` で±12半音（小数可）を指定する。既定の0半音ではサンプルの全ビットを保持する遅延0のバイパス。補正は既定で有効。`--formant preserve` で元の声の共鳴を復元し、`--formant off` で無効にする。聴取品質は未検証。

入出力を同一のフルデュプレックスASIOコールバックで処理し、入力1を同じbuffer indexの出力1へ送る。追加FIFOはない。その他の出力チャンネルは無音。ピッチ処理有効時にはDSPの循環バッファによる遅延がある。

## 実行

48 kHzに設定したフルデュプレックスASIOデバイスを使用する。既定バッファは512 samples。CLIでは `--list-devices` で登録名を確認し、`--device "ASIOデバイス名"` を明示指定する。GUIではデバイス欄から選択する。

```powershell
# プロジェクトのルートディレクトリで実行
# 動作確認GUI
.\target\release\rust-low-latency-voice-changer-gui.exe
.\target\release\rust-low-latency-voice-changer-cli.exe --list-devices
.\target\release\rust-low-latency-voice-changer-cli.exe --device "ASIOデバイス名" --inspect
.\target\release\rust-low-latency-voice-changer-cli.exe --device "ASIOデバイス名" --seconds 60
# ±3半音を試す（通常実行の開始時に固定）
.\target\release\rust-low-latency-voice-changer-cli.exe --device "ASIOデバイス名" --pitch 3 --seconds 60
.\target\release\rust-low-latency-voice-changer-cli.exe --device "ASIOデバイス名" --pitch -3 --seconds 60
# 補正なしとの比較
.\target\release\rust-low-latency-voice-changer-cli.exe --device "ASIOデバイス名" --pitch 3 --formant off --seconds 60
# Enterを押すまで実行
.\target\release\rust-low-latency-voice-changer-cli.exe --device "ASIOデバイス名"
```

## 動作確認GUI

`rust-low-latency-voice-changer-gui.exe` でWindowsのネイティブGUIを起動する。コンソールウィンドウは表示しない。登録済みASIOデバイスを選択し、frames、±12半音のピッチ（小数可）、Preserve formantsを設定する。デバイスは未選択で起動し、その他の初期値はCLIと同じ。`--device`、`--frames`、`--pitch`、`--formant` で起動時の値を指定できる。`--help` と起動引数のエラーはダイアログで表示する。`--seconds`、`--inspect`、`--list-devices` はGUI起動引数として拒否する。

- **Inspect**：デバイス設定と指定framesの対応を確認する。音声バッファは作らない。
- **Start**：入力1→出力1の音声処理を開始する。その他の出力は無音。
- **Stop**：停止・ドライバ解放後に設定変更と再開ができる。実行中の設定欄は無効。

100 msごとに実行時間、デバイス形式・レート、frames、ドライバ報告レイテンシ、入力累計、コールバック数、最大処理時間、処理期限超過、sample-position不連続、異常フラグを表示する。終了後も最終統計を保持する。ウィンドウを閉じると停止を要求し、音声スレッドとドライバの解放を待つ。

`src/gui.rs` の専用スレッドがCOM初期化とASIOのロード・設定照会・開始・停止・解放を所有する。DSPは音声コールバックで処理し、GUIのロック・文字列生成・待機をコールバックに入れない。

GUI操作の実機スモークテストは `scripts/gui-smoke.ps1 -Device "ASIOデバイス名" -Frames 512 -RunSeconds 3`。対象デバイスを48 kHzに設定し、対応するframesを指定する。設定確認、不正frames、存在しないデバイス、0/+3/−3半音での開始・停止・再開、設定欄の無効化、再生中の終了を検証する。聴取・録音による音質や総遅延の評価は別途行う。

CLIは独立した実行ファイル `rust-low-latency-voice-changer-cli.exe`。`--list-devices` はドライバを開かずに登録名を取得する。`--inspect --device "ASIOデバイス名"` は指定ドライバを初期化してチャンネル形式・レート・バッファ範囲を照会し、音声バッファを作成せず、レートも変更しない。非対応設定はエラー終了し、他設定へ自動で切り替えない。

## 音声のルーティング

マイク信号を選択したASIOデバイスの入力1へ送り、出力1をモニターや録音・配信先へ接続する。接続方法は利用するデバイスや音声ルーティング環境に合わせて設定する。

アプリはモノラルの入力1→出力1を扱い、その他の出力は無音。出力を入力へ再送しない。直接モニターと未処理マイクの別経路がある場合、処理後の音声との二重再生を避ける。アプリは外部のルーティング設定を自動変更しない。

## 遅延

| 項目 | 現在の値 |
| --- | --- |
| DSPバイパス遅延 | 0 samples（0半音で全ビット保持） |
| ピッチ処理の読み出し遅延 | 2〜478 samples / 0.042〜9.958 ms。中心240 samples / 5 ms。固定遅延ではない |
| 追加FIFO遅延 | 0 samples。FIFOを除去 |
| ASIO処理周期 | frames / 48000秒（512 framesなら10.667 ms） |
| ドライバ報告入力レイテンシ | デバイス・設定に依存。起動後に表示 |
| ドライバ報告出力レイテンシ | デバイス・設定に依存。起動後に表示 |
| 実総往復遅延 | 利用環境での測定が必要 |

ドライバ報告値は起動後に `ASIOGetLatencies` から取得する。処理周期やこれらの値の単純加算を実総往復遅延とは扱わない。64 samplesでの動作可否はデバイスの対応範囲と設定に依存し、利用環境で検証する必要がある。DSP内部5 ms以下と総往復5 msは別の目標。

## ピッチ処理と制約

`src/pitch.rs` は48 kHz固定、512サンプルの事前確保循環バッファ、半周期ずらした2つの読み出しヘッド、線形補間、相補Hannクロスフェードを使う。比率は `2^(semitones/12)`。遅延の変化量を `1 - ratio` とすることで読み出し速度を変える。[可変遅延の原理](https://www.dsprelated.com/freebooks/pasp/Time_Varying_Delay_Line_Reads.html)を参照。

ライブラリの `set_semitones` は現在値から20 msで比率を平滑化し、0半音への出入りは10 msでdry/wetをフェードする。復帰後は遅延0のビット保持バイパス。CLI・GUIともストリーム開始時の値を固定する。GUIでは停止して設定変更・再開できる。ASIO制御スレッドからDSPを直接変更しない。

短い窓では低音、過渡音、大幅な変更に揺れ、振幅変調、副成分が出る。窓に周期が一致しない信号の主要周波数も理想比率からずれる場合がある。線形補間は高域を減衰させ、ピッチアップ時のエイリアシングを防ぐフィルタも未実装。±12半音の範囲対応は音質保証を意味しない。DSP最大遅延5 ms以下の目標は未達で、高音質モードは未実装。

wet側の履歴は初期値0で、開始直後は過去入力に対応する無音を読む。F32ではNaN/Infをwet側だけ0として扱い、履歴への伝播を防ぐ。整数のwet処理はf64正規化、丸め・飽和変換を使い、0半音の出力は数値変換しない。

## フォルマント補正

`src/formant.rs` は32次LPCの音源・フィルタ分離を行う。直前1024 samples（21.333 ms）のHann窓から128 samples（2.667 ms）ごとに自己相関とLevinson–Durbin法で共鳴モデルを推定する。逆ラティスで残差を取り出し、既存のピッチシフターで残差だけを変調し、元の共鳴モデルで再合成する。[LPCスペクトル包絡](https://www.dsprelated.com/freebooks/sasp/Spectral_Envelope_Extraction.html)と[分析・合成の例](https://www.mathworks.com/help/audio/ug/lpc-analysis-and-synthesis-of-speech.html)を参照。

反射係数を±0.995以内に制限し、128 samplesで線形補間する。IIR多項式の直接補間はしない。自己相関の対角正則化と35 HzのGaussian lag taperで鋭すぎる共鳴を抑える。無音の窓では係数を0へ戻す。wet出力は±1にクリップし、内部状態はクリップしない。過大入力では歪みが生じる。履歴・分析用配列・窓・lag taperは開始前に確保・計算し、コールバック内の動的確保・指数計算はない。

過去の入力のみを使い、先読みや追加ブロック待機はない。21.333 msは分析窓の長さで、声の変化への追従時間に影響する。フィルタには周波数依存の位相遅延と残響があり、補正時の全DSP応答が2〜478 samples以内で終わるという意味ではない。0半音では補正の内部状態を更新しつつ、元サンプルをそのまま返す。

独立したフォルマント上下移動パラメータはない。1500 Hz共鳴の合成雑音モデルで±3半音の補正を検証したが、実声の母音・子音・急変・大幅なピッチ変更の品質は未検証。既存ピッチシフターの副成分やエイリアシングは残る。

## 状態表示と停止

毎秒と終了時に、フルデュプレックスブロック数、frames、非ゼロ入力累計、最大コールバック時間、周期を超えた処理数、有効なsample-position付きブロック数、位置の不連続数、ドライバ通知フラグを表示する。

`timestamp blocks=0` ならsample-positionによる欠落判定は利用できていない。`nonzero input samples=0` ならマイク信号到達は未確認。非ゼロ入力や終了コード0だけで音声品質を保証しない。レイテンシ変更通知は制御スレッドで再照会し、reset/resync/レート変更/クロック変更/overload/不正なbuffer index/コールバック再入は停止理由として記録する。2秒のコールバック停止、処理期限超過、sample-position不連続も失敗として扱う。

終了時は処理を無効化→ASIOStop→状態ポインタ解除→実行中コールバックの終了待ち→ASIODisposeBuffers→ドライバ解放。初期化途中の失敗でも終了処理を行う。再設定通知を受けた場合は設定を確認して再起動する。

## ビルド

```powershell
. .\scripts\dev-env.ps1
cargo check --locked
cargo build --release --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --check
```

リリースビルドで `target/release/rust-low-latency-voice-changer-cli.exe` と `target/release/rust-low-latency-voice-changer-gui.exe` が生成される。個別にビルド/実行する場合は `cargo build --release --locked --bin rust-low-latency-voice-changer-gui`、`cargo run --release --locked --bin rust-low-latency-voice-changer-cli -- --inspect --device "ASIOデバイス名"` のように指定する。入口は `src/bin/cli.rs` と `src/bin/gui.rs`、共通コードは `src/lib.rs` のモジュールとして共有する。

ビルドにはWindows x64 Rust MSVC、rustfmt、Clippy、Visual C++ Build ToolsとWindows SDK、libclang、ASIO SDKを用意する。ASIO SDKのパスを `CPAL_ASIO_DIR`、libclang DLLのディレクトリを `LIBCLANG_PATH` に設定する。

上記の `dev-env.ps1` は `.tools` 内にローカルツールチェーンを配置した開発環境向けで、現在のPowerShellセッションだけを設定する。既存のツールチェーンを使用する場合はこの行を省略し、必要な環境変数を設定してCargoコマンドを実行する。

依存関係はasio-sys 0.2.6、ビルド用cc。Cargo.lockで固定。asio-sysはドライバのロード/解放とSDKバインディングに使用し、同クレートの高水準ストリームやMutex付きコールバックは使用しない。native/asio_extra.cppはレイテンシ照会のC++ ABIをC ABIへ橋渡しする小さなファイル。

src/lib.rsは共通モジュールを公開し、src/pitch.rs、src/formant.rsのDSPはASIOから独立する。src/asio.rsのunsafeはドライバ所有ダブルバッファ、SDK ABI、コールバック寿命、排他的なDSP状態アクセスの境界に限定する。DSPはUnsafeCellに置き、既存のprocessingアトミックガードでコールバック再入を拒否する。初期化時に全チャンネル形式、ポインタのNULL/整列/領域重複を検証する。対応形式は同一のlittle-endian f32/i32/i16。

アプリのデータコールバックには動的確保、Mutex、ログ、ファイルアクセス、待機を置かない。ドライバ内部の実装は制御できない。処理時間はInstantの壁時計計測で、OSスケジューリングの影響も含む。

## 自動テスト

自動テストはLPC再構成、共鳴保持、補正有効時のピッチ比・ブロック分割・無音・過大入力・非有限値・整数形式・設定再変更・バイパス復帰に加え、±3/±12半音の周波数、短窓の副成分を含む主要周波数、インパルス遅延範囲、無音、1/7/64/512/8192サンプルのブロック分割、パラメータ再変更、バイパス復帰、整数変換、不正なCLI値を検証する。低周波テスト信号での変化幅制約は、すべての入力でクリックが聞こえないことの保証ではない。
