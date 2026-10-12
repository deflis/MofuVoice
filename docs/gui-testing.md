# GUI検証

## 自動検証

Node.js 26とRust/MSVC、ASIO SDK、libclangを用意して実行します。

~~~powershell
pnpm install --frozen-lockfile
mise run build:web
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --features desktop --locked
cargo test --release --locked --test spectral_quality
pnpm desktop:build
~~~

## Windowsでの操作確認

- 起動時は保存済み設定を復元し、自動で音声処理を開始しない。
- Start / Inspectでデバイスを選択する必要がある。Inspectは音声を開始せず、結果と対応バッファを表示する。
- 小数のPitch / Formant shift、Frames、ゲート設定を編集できる。不正値はStart / 保存 / 終了でエラーになり、処理を開始しない。
- Start中とStopの完了待ちでは設定を変更できない。停止後に再開できる。
- スペアナを開くとネイティブの別ウィンドウが出る。再度開くと既存ウィンドウを前面に出す。
- Wideband / Narrowband / Freezeが動く。閉じて開き直せる。スペアナを閉じても音声処理は継続する。
- Startせず設定を変更して終了し、再起動すると変更が復元される。
- 不正値、保存先への書き込み失敗では終了を中止し、画面上のエラーを直して閉じ直せる。
- 音声処理とスペアナの両方を開いたまま終了し、ASIOと解析ワーカーが解放される。

## 実機の音声・負荷確認

48 kHz、ASIOドライバ名とバージョン、Frames、Pitch、Formant shift、ゲート設定、スペアナの開閉・表示モードを logs/ に記録します。
スペアナ閉・開・Freezeそれぞれでオーバーラン、サンプル位置の不連続、処理時間を比較します。
DSP遅延、ドライバ報告レイテンシ、録音で実測した往復遅延は別の値として記録します。
処理音を聞きながら実際に会話し、発話や思考の妨げ、母音・子音・息の自然さを記録してください。
