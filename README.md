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
2. **コンパイラ情報優先** — 類似度だけでなく、定義・参照・型情報を重視する
3. **差分更新** — 毎回全件解析せず、変更ファイルと影響範囲だけ更新する
4. **再生成可能** — 検索インデックスは消えてもソースから再構築できる
5. **読み取り専用 MCP** — 編集やコマンド実行は接続先エージェントへ任せる
6. **段階的に拡張** — SQLite から開始し、必要になった時点で検索基盤を追加する

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
    ├─ Tree-sitter
    ├─ language-specific analyzers
    ├─ file watcher
    └─ Git diff / hooks
```

## 想定技術スタック

- Rust
- Tokio
- MCP Rust SDK (`rmcp`)
- Tree-sitter
- SQLite
- SQLite FTS5
- `notify` によるファイル監視
- `git2` または Git CLI
- TypeScript の精密解析用 sidecar（必要になった場合）

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

## 想定 CLI

```bash
astral index .
astral serve --project-root .
astral status .
astral refresh .
astral rebuild .
astral clean .
```

## MCP 接続イメージ

```toml
[mcp_servers.astral]
command = "astral"
args = ["serve", "--project-root", "."]
```

stdio 接続では、Codex や Claude Code などの MCP クライアントが ASTral を必要な間だけ子プロセスとして起動します。常時起動は不要です。

## ドキュメント

- [開発環境の構築](docs/development.md)
- [アーキテクチャ](docs/architecture.md)
- [インデックスと更新戦略](docs/indexing.md)
- [解析結果の保存方式](docs/storage.md)
- [MCP ツール設計](docs/mcp.md)
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

## ライセンス

[MIT License](LICENSE)
