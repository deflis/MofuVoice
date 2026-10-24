# ビルド・開発

[README](README.md)

ソースコードから実行ファイルを作成するための手順です。GUIの操作は[詳細な使い方](docs/usage.md)を参照してください。

## Windows（ASIO対応）

Windowsで既定のASIO対応ビルドを作るには、RustのWindows x64 MSVCツールチェーン、Visual C++ Build ToolsとWindows SDK、libclang、ASIO SDK、WebView2 Runtimeとmiseが必要です。

ASIO SDK は `mise install` で Steinberg から取得します（`http:asiosdk`。SDK 自体は再配布しないでください）。mise が `CPAL_ASIO_DIR` を SDK ルート（`common/asio.h` があるディレクトリ）に設定します。libclang が PATH にない場合は `mise.local.toml` か環境変数で `LIBCLANG_PATH` を指定してください。

```powershell
mise trust
mise install
pnpm install --frozen-lockfile
pnpm desktop:build
```

`target/release/` に実行ファイルが生成されます。React画面の開発には `pnpm desktop:dev`、CLIだけのビルドには `cargo build --release --locked` を使います。

整形は `mise run fmt`、チェックは `mise run check` で実行します。

## CSS Modules

画面の基本要素スタイルは `frontend/src/styles.css` に置き、画面単位の共通スタイルは `windows/main`、`windows/log`、`windows/settings` の CSS Module に分けています。EQ、デエッサー、チャンネル割り当て、詳細ポップアップなど固有のスタイルは各機能ディレクトリの CSS Module に置き、フィルタ概要のカード群は `overview/overview.module.css` を共有します。コンポーネントでは担当する CSS Module を `styles` として import して `className={styles.check}` のように参照します。

[CSS Modules Kit](https://github.com/mizdra/css-modules-kit) でクラス名の型を生成し、`*.module.css.d.ts` は対応する CSS Module と同じディレクトリに Git 管理します。`pnpm dev` は Vite と型生成の監視を同時に起動します。CSS Module を変更したら `pnpm generate:css` を実行して型定義を更新してください。`pnpm check:css` は生成結果がコミット済みの型定義と一致するか確認します。`pnpm typecheck` と `pnpm build` はコミット済みの型定義を使います。

Kit 1.4 は TypeScript 5/6 の JavaScript API を使うため、`frontend/tooling/css-modules-kit` に TypeScript 6 と一緒に分離しています。アプリの型チェックには引き続きルートの TypeScript 7 を使います。依存関係はルートの `pnpm install` でまとめてインストールされます。

VS Code では推奨拡張の CSS Modules Kit をインストールすると、クラス名の補完、定義ジャンプ、リネームを利用できます。

## その他のビルド構成

- ASIO SDKなしのCPAL専用CLI：`cargo build --release --locked --no-default-features --features cpal-backend`
- ASIO SDKなしのCPAL専用GUI：ビルド時に `--no-default-features --features cpal-backend,desktop` を指定します。
- 音声バックエンドを含まない共通DSP：`cargo build --release --locked --no-default-features`

LinuxではALSA開発パッケージ（Debian/Ubuntuでは `libasound2-dev`）が必要です。GUIのビルドには、OSに応じた[Tauriの前提条件](https://v2.tauri.app/start/prerequisites/)も用意してください。

## 音声処理の方針

DSP（ピッチ・フォルマント・FFT・ゲート・自動レベル）は既存ライブラリを使わず自前で実装します。音声入出力にはCPALを使用し、デバイス設定とDSPを分離します。品質検証を優先し、マイクからヘッドホンまでの総遅延10 ms以下を今後の目標としています。
