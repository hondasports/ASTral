# 開発環境の構築

このドキュメントでは、ASTral をローカルで開発するための環境構築手順を説明します。

> [!NOTE]
> ASTral は現在、設計・初期開発段階です。Cargo workspace や実行可能な CLI が追加されるまでは、一部のビルド・実行コマンドは将来の想定手順です。

## 必要なツール

### 必須

- Git
- Rust stable
- Cargo
- `rustfmt`
- Clippy
- OS 標準のネイティブビルドツール

ASTral は Tree-sitter や SQLite 関連 crate を利用する想定のため、環境によっては C/C++ コンパイラやリンカーが必要です。

### 任意

- Node.js 20 以降
  - `npx skills` で AI エージェント用スキルを導入する場合
  - 将来 TypeScript 解析 sidecar を開発する場合
- SQLite CLI
  - ローカルのインデックス内容を手動確認する場合

SQLite サーバーを常時起動する必要はありません。ASTral はローカルの SQLite ファイルを直接開く設計です。

## OS ごとの準備

### macOS

Command Line Tools を導入します。

```bash
xcode-select --install
```

### Ubuntu / Debian

```bash
sudo apt update
sudo apt install -y build-essential pkg-config git curl
```

### Windows

次を導入します。

- Git for Windows
- Visual Studio Build Tools の「Desktop development with C++」
- Rust の MSVC toolchain

Windows では PowerShell または Windows Terminal の利用を推奨します。

## リポジトリの取得

```bash
git clone https://github.com/hondasports/ASTral.git
cd ASTral
```

## Rust toolchain

Rust は `rustup` で管理します。

未導入の場合は、公式手順に従って `rustup` をインストールしてください。

- https://rustup.rs/

ASTral 用に stable toolchain と必要な component を設定します。

```bash
rustup toolchain install stable --component rustfmt clippy
rustup override set stable
```

確認します。

```bash
rustc --version
cargo --version
rustfmt --version
cargo clippy --version
```

将来 `rust-toolchain.toml` が追加された場合は、そのファイルを正とし、上記の override は不要になる可能性があります。

## Node.js

AI エージェント用スキルを導入する場合は Node.js 20 以降を用意します。

```bash
node --version
npx --version
```

Node.js のバージョン管理には Volta、mise、asdf、nvm など任意のツールを利用できます。プロジェクトにバージョン固定ファイルが追加された場合は、その指定を優先してください。

## AI エージェント用スキル

スキルの導入は ASTral 自体のビルドには必須ではありません。ただし、Codex、Claude Code、Cursor などを使って開発する場合は、Rust と MCP の実装方針をエージェントへ共有するために導入を推奨します。

`skills` CLI は `npx` から直接実行できます。グローバルインストールは不要です。

### 推奨スキル

Rust の設計・実装・レビュー向け:

```bash
npx skills add https://github.com/leonardomso/rust-skills \
  --skill rust-skills
```

MCP サーバーのツール設計・transport・品質管理向け:

```bash
npx skills add https://github.com/microsoft/skills \
  --skill mcp-builder
```

CLI が対象エージェントを尋ねた場合は、実際に利用するエージェントを選択してください。対象を明示する場合は `--agent` を利用できます。

Codex の例:

```bash
npx skills add https://github.com/leonardomso/rust-skills \
  --skill rust-skills \
  --agent codex

npx skills add https://github.com/microsoft/skills \
  --skill mcp-builder \
  --agent codex
```

Claude Code の例:

```bash
npx skills add https://github.com/leonardomso/rust-skills \
  --skill rust-skills \
  --agent claude-code

npx skills add https://github.com/microsoft/skills \
  --skill mcp-builder \
  --agent claude-code
```

匿名 telemetry を無効化する場合は、実行時に `DISABLE_TELEMETRY=1` を設定します。

```bash
DISABLE_TELEMETRY=1 npx skills add https://github.com/microsoft/skills \
  --skill mcp-builder
```

> [!CAUTION]
> Agent Skill はエージェントへ手順やルールを追加します。導入前に配布元と `SKILL.md` の内容を確認し、信頼できないスキルをプロジェクトへ追加しないでください。

Skills CLI の使い方:

- https://www.skills.sh/docs/cli

## ビルドと検証

Cargo workspace の実装後は、次を標準の検証コマンドとします。

```bash
cargo build --all-targets --all-features
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

ローカル修正時は、まず対象 crate のテストを実行し、Pull Request 前に workspace 全体を確認してください。

## 想定する開発時の実行

CLI 実装後は、次の形で対象リポジトリを解析します。

```bash
cargo run -- index /path/to/target-repository
```

stdio MCP サーバーとして起動する想定:

```bash
cargo run -- serve --project-root /path/to/target-repository
```

MCP クライアントが子プロセスとして起動する運用では、ASTral を別ターミナルで常時起動する必要はありません。

## ローカルデータ

解析結果は、リポジトリへ直接コミットせず、OS のユーザーデータ領域へ保存する方針です。

概念上の保存先:

```text
~/.local/share/astral/projects/<project-id>/
├─ index.sqlite
└─ state.json
```

macOS や Windows では、それぞれの標準的なユーザーデータディレクトリを使用します。実装では `directories` などの crate を使い、OS ごとの差を吸収します。

## ログ

実装後は `RUST_LOG` でログレベルを制御できる形を想定しています。

```bash
RUST_LOG=astral=debug cargo run -- serve --project-root .
```

ソースコード本文、秘密情報、認証情報を通常ログへ出力しないでください。

## トラブルシューティング

### `linker` または C compiler が見つからない

OS ごとのネイティブビルドツールが導入されているか確認してください。

### `rustfmt` または `clippy` が見つからない

```bash
rustup component add rustfmt clippy
```

### スキルのインストール先が想定と違う

対象エージェントを `--agent codex` や `--agent claude-code` で明示してください。既存の設定ファイルを上書きする可能性がある場合は、実行前に差分を確認してください。

### インデックスの状態がおかしい

CLI 実装後は、次の順に確認する想定です。

```bash
astral status .
astral refresh .
astral rebuild .
```

再構築可能な派生データとして扱い、手作業で SQLite を直接修正する運用は避けます。
