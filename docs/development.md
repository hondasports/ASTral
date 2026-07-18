# 開発環境の構築

このドキュメントでは、ASTral をローカルで開発するための環境構築手順を説明します。

> [!NOTE]
> ASTral は初期開発段階です。現在は OXC による JS/TS 索引、SQLite/FTS5 検索、`astral index`、`astral watch`、`astral search-code`、`astral find-symbol`、`astral read-symbol`、`astral find-references`、`astral find-callers`、`astral find-callees`、`astral find-related-tests` を提供します。

## 必要なツール

### 必須

- Git
- Rust stable
- Cargo
- `rustfmt`
- Clippy
- OS 標準のネイティブビルドツール

ASTral の主要な JavaScript / TypeScript analyzer は Rust 製の OXC を利用します。SQLite のビルド設定や、将来追加する Tree-sitter grammar などにより、環境によっては C/C++ コンパイラ、リンカー、`pkg-config` が必要です。

### AI エージェントを利用する場合に必須

- Node.js `24.18.0`（現在の最新LTS）
- `npx`
- `grill-me` Skill

Node.js は OXC analyzer 自体には不要です。AI エージェント用 Skill をセットアップする場合だけ必要です。

### 任意

- SQLite CLI
  - ローカルのインデックス内容を手動確認する場合
- Node.js または別バイナリ
  - 将来 TypeScript の精密型解析 sidecar を開発する場合

SQLite サーバーを常時起動する必要はありません。ASTral はローカルの SQLite ファイルを直接開く設計です。

## プラットフォーム別のネイティブ依存

Rustのコードと検証コマンドは、Windows・macOS・Linuxで共通です。OSごとの差分は、Rust crateをコンパイルするためのC/C++コンパイラ、リンカー、SDKの導入だけです。

### Windows

- Git for Windows
- Visual Studio Build Tools の `Desktop development with C++`
- Windows SDK

`rust-toolchain.toml` がWindowsではMSVC toolchainを選択します。PowerShell、Windows Terminal、Git BashのいずれからでもCargoコマンドを実行できます。

### macOS

Xcode Command Line Toolsを導入します。

```bash
xcode-select --install
```

### Ubuntu / Debian

```bash
sudo apt update
sudo apt install -y build-essential pkg-config git curl
```

### Fedora

```bash
sudo dnf group install -y "Development Tools"
sudo dnf install -y pkgconf-pkg-config git curl
```

### Arch Linux

```bash
sudo pacman -S --needed base-devel pkgconf git curl
```

その他のUnix系OSでは、利用するパッケージマネージャーで同等のC/C++ビルドツール、`pkg-config`、Gitを導入してください。

## リポジトリの取得

```bash
git clone https://github.com/hondasports/ASTral.git
cd ASTral
```

## 共通セットアップ

Rustはプラットフォーム共通で`rustup`から導入します。未導入の場合は公式インストーラーを利用してください。

- https://rustup.rs/

リポジトリには [`rust-toolchain.toml`](../rust-toolchain.toml) があり、stable toolchain、`rustfmt`、`clippy`をプロジェクト単位で指定しています。リポジトリルートでCargoまたはrustupを実行すると、この設定が自動的に適用されます。

既存のtoolchainを明示的に準備する場合は、次を実行します。

```text
rustup toolchain install stable --component rustfmt clippy
```

確認します。

```text
rustc --version
cargo --version
rustfmt --version
cargo clippy --version
rustup show active-toolchain
```

`rustup show active-toolchain` の結果がstableで、対象リポジトリのdirectory overrideが表示されれば準備完了です。PATHを変更した直後は、ターミナルやIDEを再起動してください。

## Analyzer 関連の依存関係

JavaScript / TypeScript 系ファイルは OXC を第一級 analyzer として扱います。

想定する主な crate:

```toml
[dependencies]
oxc_allocator = "..."
oxc_ast = "..."
oxc_ast_visit = "..."
oxc_parser = "..."
oxc_semantic = "..."
oxc_span = "..."
oxc_resolver = "..."
```

現在の実装で使用する OXC crate は `0.140.0`、`oxc_resolver` は `11.24.2`、SQLite は `rusqlite 0.40.1` の `bundled-full` です。workspace の `Cargo.toml` と `Cargo.lock` を正とし、バージョン更新時は index の再構築が必要です。

OXC の AST 型は analyzer crate 内でのみ扱い、store、search、MCP crate へ直接公開しません。共通の `AnalysisResult` へ変換してから後段へ渡します。

将来、OXC 対象外の言語を追加する場合は、Tree-sitter または言語専用の compiler / language server を analyzer abstraction の背後へ追加します。

TypeScript compiler 相当の型情報が必要になった場合のみ、Node.js または別バイナリで動作する optional sidecar を追加します。

関連する設計判断:

- [ADR 0001: JavaScript / TypeScript 解析に OXC を採用する](adr/0001-use-oxc-for-javascript-typescript.md)
- [ADR 0002: Phase 1 の SQLite 索引と再構築](adr/0002-phase1-sqlite-index.md)

## Node.js（AIエージェント用）

AI エージェント用 Skill を導入する場合は、リポジトリの [`.nvmrc`](../.nvmrc) に記載したNode.js `24.18.0`を使用します。CIも同じファイルを読み込みます。

```text
nvm install 24.18.0
nvm use 24.18.0
node --version
npx --version
```

`nvm`以外のNode.js version managerを使う場合も、`.nvmrc`の値を指定してください。Windowsのnvm-windowsでは`nvm install 24.18.0`と`nvm use 24.18.0`を明示的に実行します。`node --version`が`v24.18.0`になっていることを確認してください。

## AI エージェント用 Skill

リポジトリ固有の開発 Skill は `.agents/skills/` に保持します。一方、汎用の壁打ち Skill は upstream をユーザー環境へグローバル導入し、リポジトリへ複製しません。

### `grill-me` のセットアップ

ASTral の要件・設計・実装・検証・レビュー Skill は、重要な判断分岐を洗練させるために `grill-me` を利用します。AI エージェントを使って開発する場合は、最初に次を実行してください。

```bash
npx skills add https://github.com/mattpocock/skills \
  --skill grill-me \
  --global
```

CLI が対象エージェントを尋ねた場合は、実際に利用するエージェントを選択してください。ASTral は Codex 専用サブエージェント設定をリポジトリへ持ち込みません。

対象を明示したい場合は `--agent` を利用できます。例:

```bash
npx skills add https://github.com/mattpocock/skills \
  --skill grill-me \
  --agent claude-code \
  --global
```

```bash
npx skills add https://github.com/mattpocock/skills \
  --skill grill-me \
  --agent cursor \
  --global
```

利用するエージェントが Skill chaining に対応していない場合は、各 Skill が壁打ちを要求した時点でユーザーが `/grill-me` を起動し、終了結果を元の作業へ戻してください。

### 追加の推奨 Skill

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

これらは ASTral 自体のビルドには必須ではありません。

### Telemetry

匿名 telemetry を無効化する場合は、実行時に `DISABLE_TELEMETRY=1` を設定します。

```bash
DISABLE_TELEMETRY=1 npx skills add https://github.com/mattpocock/skills \
  --skill grill-me \
  --global
```

> [!CAUTION]
> Agent Skill はエージェントへ手順やルールを追加します。導入前に配布元と `SKILL.md` の内容を確認し、信頼できない Skill を追加しないでください。

Skills CLI の使い方:

- https://www.skills.sh/docs/cli

`grill-me` の upstream:

- https://github.com/mattpocock/skills/tree/main/skills/productivity/grill-me

## ビルドと検証

次の検証コマンドは、Windows・macOS・Linuxで共通です。

```text
cargo build --workspace --all-targets --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

ローカル修正時は、まず対象 crate のテストを実行し、Pull Request 前に workspace 全体を確認してください。

## 索引の作成と検索

リポジトリ内の生成物を増やさないよう、SQLite index は OS のユーザーデータ領域へ保存します。リポジトリのパスから project id を生成するため、Windows・macOS・Linux で同じ CLI を利用できます。

```bash
cargo run -- index .
cargo run -- search-code . "RepositoryRoot"
cargo run -- find-symbol . "RepositoryRoot"
cargo run -- read-symbol . "src/repository.rs:RepositoryRoot:..."
cargo run -- status .
```

`index` は一時 SQLite を構築し、全ファイルの解析が成功した場合だけ active index と置き換えます。解析または読み込みに失敗した場合は直前の index を保持します。schema version、OXC analyzer version、include/exclude 設定を変えた場合は `index` を再実行してください。

GitHub Actions の `CI` workflow は、push・Pull Request・手動実行に対して Ubuntu、Windows、macOS（Apple Silicon）の各runnerで上記4コマンドを実行し、最後に `astral --help` と `astral status .` のCLI smoke testを実行します。Cargo registry、Git依存、`target`はOS別にキャッシュされます。

OXC analyzer を変更した場合は、少なくとも次の fixture を確認します。

- `.js`
- `.jsx`
- `.ts`
- `.tsx`
- import / export
- nested lexical scope
- 同名 symbol
- 編集途中の不完全なコード
- parser diagnostics を含むコード

## 想定する開発時の実行

CLI 実装後は、次の形で対象リポジトリを解析します。

```text
cargo run -- index .
```

別の対象リポジトリを指定する場合は、各OSの通常のpath表記でrepository rootを渡してください。

stdio MCP サーバーとして起動する想定:

```text
cargo run -- serve --project-root .
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

POSIX shell（macOS / Linux / Git Bash）:

```sh
RUST_LOG=astral=debug cargo run -- serve --project-root .
```

PowerShell:

```powershell
$env:RUST_LOG = "astral=debug"
cargo run -- serve --project-root .
```

ソースコード本文、秘密情報、認証情報を通常ログへ出力しないでください。OXC diagnostics を記録する場合も、必要以上にソース本文を含めないようにします。

## トラブルシューティング

### `linker` または C compiler が見つからない

OS ごとのネイティブビルドツールが導入されているか確認してください。

### `rustfmt` または `clippy` が見つからない

```bash
rustup component add rustfmt clippy
```

### OXC crate の API がドキュメント例と異なる

OXC の API は、workspace に固定されたバージョンの型定義と公式ドキュメントを正としてください。依存を更新する場合は、parser fixture と normalized model の差分を確認します。

### `grill-me` が利用できない

開発環境のリポジトリルートで、セットアップコマンドを再実行してください。

```bash
npx skills add https://github.com/mattpocock/skills \
  --skill grill-me \
  --global
```

インストール先が想定と違う場合は、利用中のエージェントを `--agent` で明示してください。

### インデックスの状態がおかしい

CLI 実装後は、次の順に確認する想定です。

```bash
astral status .
astral refresh .
astral rebuild .
```

再構築可能な派生データとして扱い、手作業で SQLite を直接修正する運用は避けます。
