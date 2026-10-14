# ビルド・開発

[README](README.md)

ソースコードから実行ファイルを作成するための手順です。GUIの操作は[詳細な使い方](docs/usage.md)を参照してください。

## Windows（ASIO対応）

Windowsで既定のASIO対応ビルドを作るには、RustのWindows x64 MSVCツールチェーン、Visual C++ Build ToolsとWindows SDK、libclang、ASIO SDK、WebView2 Runtimeとmiseが必要です。

ASIO SDK は `.tools/asiosdk` に置きます（`common/asio.h` があること）。次のmiseタスクで Steinberg から取得できます（SDK 自体は再配布しないでください）。libclang が PATH にない場合は `mise.local.toml` か環境変数で `LIBCLANG_PATH` を指定してください。

```powershell
mise trust
mise install
mise run setup:asio
pnpm install --frozen-lockfile
pnpm desktop:build
```

`target/release/` に実行ファイルが生成されます。React画面の開発には `pnpm desktop:dev`、CLIだけのビルドには `cargo build --release --locked` を使います。

整形は `mise run fmt`、チェックは `mise run check` で実行します。

## その他のビルド構成

- ASIO SDKなしのCPAL専用CLI：`cargo build --release --locked --no-default-features --features cpal-backend`
- ASIO SDKなしのCPAL専用GUI：ビルド時に `--no-default-features --features cpal-backend,desktop` を指定します。
- 音声バックエンドを含まない共通DSP：`cargo build --release --locked --no-default-features`

LinuxではALSA開発パッケージ（Debian/Ubuntuでは `libasound2-dev`）が必要です。GUIのビルドには、OSに応じた[Tauriの前提条件](https://v2.tauri.app/start/prerequisites/)も用意してください。

## 音声処理の方針

DSP（ピッチ・フォルマント・FFT・ゲート・自動レベル）は既存ライブラリを使わず自前で実装します。音声入出力にはCPALを使用し、デバイス設定とDSPを分離します。品質検証を優先し、マイクからヘッドホンまでの総遅延10 ms以下を今後の目標としています。
