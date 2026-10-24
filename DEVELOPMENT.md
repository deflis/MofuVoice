# ビルド・開発・技術仕様

[READMEに戻る](README.md)

もふボイスのソースコードからのビルド手順、開発ワークフロー、CLIツール、およびDSPアルゴリズム仕様をまとめた開発者・上級者向けドキュメントです。

---

## 1. ビルド環境の構築

### 前提条件
- **Rust**: ツールチェーン（Windows x64 MSVC推奨）
- **mise**: 開発ツール・SDKマネージャー
- **Node.js & pnpm**: フロントエンド（React / Tauri UI）ビルド用
- **OS固有の要件**:
  - **Windows**: Visual C++ Build Tools、Windows SDK、WebView2 Runtime、libclang
  - **Linux**: ALSA開発パッケージ（Debian/Ubuntu: `libasound2-dev`）、WebKitGTKなど[Tauriの前提条件](https://v2.tauri.app/start/prerequisites/)
  - **macOS**: Xcode Command Line Tools

### Windows（ASIO対応）の標準セットアップ手順

ASIO SDK は `mise install` で自動的に Steinberg 公式から取得されます（SDK 自体の再配布は禁止されています）。

```powershell
# 1. ツールチェーンとSDKの導入
mise trust
mise install

# 2. フロントエンド依存関係のインストール
pnpm install --frozen-lockfile

# 3. リリースビルドの作成（GUI実行ファイル）
pnpm desktop:build
```

ビルドが完了すると、`target/release/MofuVoice.exe` に実行ファイルが生成されます。

### その他のビルド構成

- **CLIツールのみのビルド**:
  ```powershell
  cargo build --release --locked --bin mofu-voice-cli
  ```
- **ASIO非対応（CPAL専用）GUI**:
  ```powershell
  cargo build --release --locked --no-default-features --features cpal-backend,desktop
  ```
- **音声バックエンドを含まない純粋DSPライブラリ**:
  ```powershell
  cargo build --release --locked --no-default-features
  ```

---

## 2. 開発ワークフロー

- **フロントエンド開発サーバーの起動**:
  ```powershell
  pnpm desktop:dev
  ```
- **コード整形（Formatter）**:
  ```powershell
  mise run fmt
  ```
- **品質チェック（Linter & TypeCheck）**:
  ```powershell
  mise run check
  ```
- **テストの実行**:
  ```powershell
  cargo test --release
  ```

---

## 3. CLIツール（`mofu-voice-cli`）

ヘッドレス環境でのテストやパラメータの確認のために、コマンドラインツールが用意されています。

```powershell
# デバイス一覧を表示
.\target\release\mofu-voice-cli.exe --list-devices

# デバイスの詳細情報を検査
.\target\release\mofu-voice-cli.exe --backend asio --device "ASIOデバイス名" --inspect

# ピッチを+3半音上げて60秒間動作
.\target\release\mofu-voice-cli.exe --backend asio --device "ASIOデバイス名" --pitch 3 --seconds 60

# フォルマントのみを-3半音シフト
.\target\release\mofu-voice-cli.exe --backend asio --device "ASIOデバイス名" --pitch 0 --formant-shift -3

# 自動音量調整（Auto Level）を有効化
.\target\release\mofu-voice-cli.exe --backend asio --device "ASIOデバイス名" --auto-level on
```

主なCLIオプションの一覧は `mofu-voice-cli.exe --help` で確認できます。

---

## 4. 設定ファイル（`settings.ini`）

GUIの設定は、実行ファイルと同じディレクトリに `settings.ini`（UTF-8）として自動保存・復元されます。設定の変更はGUI上から行えるため、通常は手動で編集する必要はありません。設定を初期状態にリセットしたい場合は、`settings.ini` を削除してアプリを再起動してください。

---

## 5. DSPアーキテクチャ・技術方針

### 基本方針
- **DSPアルゴリズムの自前実装**: ピッチ/フォルマント処理、FFT、各種フィルター（EQ、ゲート、コンプレッサー、ディエッサー、リバーブ等）は外部のブラックボックスライブラリに頼らず、本リポジトリ内で自前実装・テストを行っています。
- **目標モニタリング遅延 10 ms以下**: 発話中の思考や会話を妨げない低遅延モニタリングを追求しています。

### ピッチ変換アルゴリズムの種別と遅延特性

| アルゴリズム名 | キー名 | DSP内部遅延（標準設定時） | 特徴 |
| :--- | :--- | :--- | :--- |
| **位相ロック（標準）** | `phase-locked` | **約 10.67 ms**（512サンプル / 48kHz） | 周波数成分の位相関係を保つフェーズボコーダー。低遅延での会話モニタリングに最適 |
| **スペクトル位相同期** | `pvsola-sync` | **約 10.67 ms**（512サンプル / 48kHz） | 直接スペクトル変換で位相誤差を補正する方式 |
| **PVSOLA** | `pvsola` | **96 ms**（4608サンプル / 48kHz） | 位相ボコーダーによる伸縮とケプストラム包絡によるフォルマント補正 |
| **TD-PSOLA** | `td-psola` | **112 ms〜**（5376サンプル〜 / 48kHz） | 波形の周期に沿った時間領域の重ね合わせ。フォルマント移動併用時は合成窓長が加算 |
| **TD-PSOLA（リサンプリング）** | `td-psola-resampled` | **112 ms**（5376サンプル / 48kHz） | 帯域制限リサンプラによる時間領域処理 |
| **FD-PSOLA** | `fd-psola` | **112 ms〜**（5376サンプル〜 / 48kHz） | ピッチ同期した短時間フレームの周波数領域（スペクトル）重ね合わせ |

> [!NOTE]
> 上記のDSP遅延に加え、オーディオインターフェースのハードウェア変換遅延、ドライババッファ遅延（例: 512サンプル = 約10.67 ms）がトータルの往復遅延として加わります。
