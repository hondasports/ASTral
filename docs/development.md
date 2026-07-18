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

ASTral の主要な JavaScript / TypeScript analyzer は Rust 製の OXC を利用します。SQLite のビルド設定や、将来追加する Tree-sitter grammar などにより、環境によっては C/C++ コンパイラ、リンカー、`pkg-config` が必要です。

### AI エージェントを利用する場合に必須

- Node.js 20 以降
- `npx`
- `grill-me` Skill

Node.js は OXC analyzer 自体には不要です。AI エージェント用 Skill をセットアップする場合だけ必要です。

### 任意

- SQLite CLI
  - ローカルのインデックス内容を手動確認する場合
- Node.js または別バイナリ
  - 将来 TypeScript の精密型解析 sidecar を開発する場合

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

Rust は `rustup` で管理します。未導入の場合は公式手順に従ってインストールしてください。

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

実装時は workspace の `Cargo.toml` と `Cargo.lock` に記録されたバージョンを正としてください。ドキュメント内の `...` をそのまま利用しないでください。

OXC の AST 型は analyzer crate 内でのみ扱い、store、search、MCP crate へ直接公開しません。共通の `AnalysisResult` へ変換してから後段へ渡します。

将来、OXC 対象外の言語を追加する場合は、Tree-sitter または言語専用の compiler / language server を analyzer abstraction の背後へ追加します。

TypeScript compiler 相当の型情報が必要になった場合のみ、Node.js または別バイナリで動作する optional sidecar を追加します。

関連する設計判断:

- [ADR 0001: JavaScript / TypeScript 解析に OXC を採用する](adr/0001-use-oxc-for-javascript-typescript.md)

## Node.js

AI エージェント用 Skill を導入する場合は Node.js 20 以降を用意します。

```bash
node --version
npx --version
```

Node.js のバージョン管理には Volta、mise、asdf、nvm など任意のツールを利用できます。プロジェクトにバージョン固定ファイルが追加された場合は、その指定を優先してください。

## AI エージェント用 Skill

リポジトリ固有の開発 Skill は `.agents/skills/` に保持します。一方、汎用の壁打ち Skill は upstream を利用し、リポジトリへ複製しません。

### `grill-me` のセットアップ

ASTral の要件・設計・実装・検証・レビュー Skill は、重要な判断分岐を洗練させるために `grill-me` を利用します。AI エージェントを使って開発する場合は、最初に次を実行してください。

```bash
npx skills add https://github.com/mattpocock/skills \
  --skill grill-me
```

CLI が対象エージェントを尋ねた場合は、実際に利用するエージェントを選択してください。ASTral は Codex 専用サブエージェント設定をリポジトリへ持ち込みません。

対象を明示したい場合は `--agent` を利用できます。例:

```bash
npx skills add https://github.com/mattpocock/skills \
  --skill grill-me \
  --agent claude-code
```

```bash
npx skills add https://github.com/mattpocock/skills \
  --skill grill-me \
  --agent cursor
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
  --skill grill-me
```

> [!CAUTION]
> Agent Skill はエージェントへ手順やルールを追加します。導入前に配布元と `SKILL.md` の内容を確認し、信頼できない Skill を追加しないでください。

Skills CLI の使い方:

- https://www.skills.sh/docs/cli

`grill-me` の upstream:

- https://github.com/mattpocock/skills/tree/main/skills/productivity/grill-me

## ビルドと検証

Cargo workspace の実装後は、次を標準の検証コマンドとします。

```bash
cargo build --workspace --all-targets --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

ローカル修正時は、まず対象 crate のテストを実行し、Pull Request 前に workspace 全体を確認してください。

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
  --skill grill-me
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
