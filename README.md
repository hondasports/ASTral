# ASTral

> AST-aware repository context engine for AI coding agents.

ASTral は、ソースコードを構文・シンボル・依存関係の単位で解析し、AI コーディングエージェントが必要な文脈を検索できるようにするローカルファーストの MCP サーバーです。

> [!IMPORTANT]
> 現在は設計・初期開発段階です。以下は目標アーキテクチャを示しており、すべての機能が実装済みという意味ではありません。

## なぜ ASTral を作るのか

AI エージェントへリポジトリ全体をそのまま渡すと、トークンを浪費し、関係の薄いコードが判断を邪魔します。一方、単純なベクトル検索だけでは、正確な定義・参照・呼び出し元・関連テストを取りこぼしやすくなります。

ASTral は次の情報を組み合わせます。

- AST に基づくシンボル単位の解析
- 関数名やエラー文字列に強い全文検索
- 定義・参照・呼び出し・import のコードグラフ
- 必要に応じた意味検索
- Working Tree を反映する差分更新
- MCP 経由の読み取り専用ツール

## 設計方針

1. **ローカルファースト** — ソースコードと解析結果を開発マシン内で扱う
2. **言語専用解析を優先** — JS / TS は OXC を使い、構文・scope・symbol・module resolution を取得する
3. **差分更新** — 毎回全件解析せず、変更ファイルと影響範囲だけ更新する
4. **再生成可能** — 検索インデックスは消えてもソースから再構築できる
5. **読み取り専用 MCP** — 編集やコマンド実行は接続先エージェントへ任せる
6. **段階的に拡張** — SQLite から開始し、必要になった時点で検索基盤や型解析を追加する

## 想定アーキテクチャ

```text
AI coding agent
    │ MCP (stdio / Streamable HTTP)
    ▼
ASTral MCP server
    ├─ search_code
    ├─ find_symbol
    ├─ find_references
    ├─ find_callers / find_callees
    ├─ find_related_tests
    └─ get_index_status
            │
            ▼
Repository index
    ├─ SQLite: files, symbols, edges, chunks
    ├─ FTS5: lexical search
    ├─ optional vector index
    └─ working-tree overlay
            ▲
            │
Indexer
    ├─ OXC: JavaScript / TypeScript / JSX / TSX
    ├─ Tree-sitter: optional analyzers for other languages
    ├─ language-specific analyzers / sidecars
    ├─ file watcher
    └─ Git diff / hooks
```

## 解析方針

JavaScript / TypeScript 系ファイルは OXC を第一級 analyzer として扱います。

```text
source
  ↓
oxc_parser
  ↓
oxc_semantic
  ↓
oxc_resolver
  ↓
ASTral normalized model
  ├─ symbols
  ├─ references
  ├─ imports / exports
  ├─ calls
  └─ chunks
```

OXC 固有の AST は永続化せず、共通の normalized model へ変換して SQLite に保存します。

完全な TypeScript 型推論や overload resolution が必要になった場合は、TypeScript Language Service、TypeScript Compiler API、typescript-go、LSP などを利用する sidecar を追加します。sidecar が停止していても、OXC ベースの検索へフォールバックできる設計にします。

## 想定技術スタック

- Rust
- Tokio
- MCP Rust SDK (`rmcp`)
- OXC (`oxc_parser`, `oxc_semantic`, `oxc_resolver` など)
- Tree-sitter（OXC 対象外言語の将来候補）
- SQLite
- SQLite FTS5
- `notify` によるファイル監視
- `git2` または Git CLI
- TypeScript の精密型解析用 sidecar（必要になった場合）

## 開発を始める

必要な Rust toolchain、OS ごとのネイティブビルドツール、任意の AI エージェント用スキルについては、[開発環境の構築](docs/development.md)を参照してください。

```bash
git clone https://github.com/hondasports/ASTral.git
cd ASTral
rustup toolchain install stable --component rustfmt clippy
rustup override set stable
```

AI コーディングエージェントを利用する場合は、`npx skills` による Rust・MCP 開発スキルの導入手順も用意しています。スキルは ASTral 自体のビルドには必須ではありません。

## 更新戦略

ASTral は次の三段構えでインデックスの鮮度を保ちます。

1. ファイル保存時に watcher が変更ファイルを差分更新
2. MCP 検索前に content hash を確認し、古ければ自己修復
3. checkout・merge・rebase・commit 後に Git 状態との整合性を確認

解析中の一時的な構文エラーでは、直前の正常な結果を残して `stale` として扱います。

## クイックスタート

### 1. ビルド

```bash
git clone https://github.com/hondasports/ASTral.git
cd ASTral
cargo build --release
```

`target/release/astral` が生成されます。PATH を通すか、`cargo install --path .` でインストールできます。

### 2. リポジトリを登録

```bash
astral register my-repo .
```

`my-repo` は検索で使う名前です。登録情報と索引は OS のユーザーデータ領域に保存されます。保存先を変えたい場合は `ASTRAL_DATA_DIR` 環境変数を設定してください。

### 3. 索引を作成

```bash
astral index my-repo
```

`status` で索引の状態を確認できます。

```bash
astral status my-repo
```

### 4. CLI で検索

```bash
# 自然言語 / エラー文字列で検索
astral search-code my-repo "owner を譲渡してから退会する処理"

# シンボル名で検索
astral find-symbol my-repo "transferOwnership"

# シンボル ID から本文を読む
astral read-symbol my-repo "symbol:my-repo:src/auth/session.ts:createSession:12:0"

# 関係をたどる
astral find-references my-repo "transferOwnership"
astral find-callers my-repo "transferOwnership"
astral find-related-tests my-repo "transferOwnership"
```

### 5. MCP サーバーとして使う

`astral serve` で stdio 接続の MCP サーバーを起動します。MCP クライアントが必要な間だけ子プロセスとして起動するため、常時起動は不要です。

利用可能なツールは `search_code`、`find_symbol`、`read_symbol`、`find_references`、`find_callers`、`find_callees`、`find_related_tests`、`get_index_status` です。

設定例（Windsurf / Claude Code など）:

```json
{
  "mcpServers": {
    "astral": {
      "command": "C:\\Users\\<username>\\Documents\\sourcecode\\ASTral\\target\\release\\astral.exe",
      "args": ["serve"],
      "env": {
        "ASTRAL_DATA_DIR": "C:\\Users\\<username>\\AppData\\Roaming\\astral"
      }
    }
  }
}
```

Unix 系の例:

```json
{
  "mcpServers": {
    "astral": {
      "command": "/path/to/astral",
      "args": ["serve"],
      "env": {
        "ASTRAL_DATA_DIR": "$HOME/.local/share/astral"
      }
    }
  }
}
```

AI エージェントからは「`my-repo` の `transferOwnership` 関数を探して」のように自然言語で頼むか、`mcp0_find_symbol` などのツールを直接呼び出せます。

### 6. 索引の鮮度を保つ

```bash
astral watch my-repo
```

ファイル保存を監視して、索引を差分更新します。`RUST_LOG=astral=info` を設定すると進捗が表示されます。

## ドキュメント

- [開発環境の構築](docs/development.md)
- [アーキテクチャ](docs/architecture.md)
- [ADR 0001: JavaScript / TypeScript 解析に OXC を採用する](docs/adr/0001-use-oxc-for-javascript-typescript.md)
- [インデックスと更新戦略](docs/indexing.md)
- [解析結果の保存方式](docs/storage.md)
- [MCP ツール設計](docs/mcp.md)
- [ADR 0008: Phase 7 semantic search provider 契約](docs/adr/0008-phase7-semantic-search.md)
- [ADR 0009: Phase 8 TypeScript precision sidecar](docs/adr/0009-phase8-typescript-precision-sidecar.md)
- [ADR 0010: Phase 9 self-hosted remote/team mode](docs/adr/0010-phase9-remote-team-mode.md)
- [ロードマップ](docs/roadmap.md)
- [コントリビューションガイド](CONTRIBUTING.md)

## スコープ外

初期段階では、次の機能を ASTral 自身の責務に含めません。

- ソースコードの直接編集
- 任意コマンドの実行
- lint・テストの実行環境
- AI モデルのホスティング
- 解析結果のクラウド同期
- 大規模な組織横断コード検索
- TypeScript compiler と同等の完全な型検査

## ライセンス

[MIT License](LICENSE)
