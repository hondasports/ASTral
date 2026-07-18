# ロードマップ

## 方針

ASTral は、最初からベクトル検索や複数リポジトリ対応まで作り込まず、コード探索に必要な最小機能から段階的に実装します。

各フェーズは、デモではなく実際のリポジトリで評価できる状態を完了条件にします。

## Phase 0: Project foundation

目的: Rust プロジェクトと開発規約を整える。

- Cargo workspace の作成
- CLI エントリポイント
- logging / tracing
- error model
- configuration loading
- test framework
- CI

想定コマンド:

```bash
astral --help
astral status .
```

完了条件:

- ローカルと CI で build / test / lint が通る
- repository root を安全に解決できる

## Phase 1: File and symbol index

目的: TypeScript / TSX を中心に、全文・symbol 検索を成立させる。

- repository scanner
- `.gitignore` 対応
- Tree-sitter integration
- file / symbol / chunk extraction
- SQLite schema
- SQLite FTS5
- initial full indexing
- `search_code`
- `find_symbol`
- `read_symbol`
- `get_index_status`

完了条件:

- 中規模 TypeScript リポジトリを索引できる
- 関数名・コンポーネント名・エラー文字列から目的のコードを取得できる
- インデックスを削除して再構築できる

## Phase 2: Incremental indexing

目的: AI がコードを書き換えている最中も検索結果を追従させる。

- file watcher
- debounce / batching
- file-level transactional replacement
- dirty file set
- stale state
- search-time freshness check
- added / modified / renamed / deleted file handling

完了条件:

- 保存後、変更ファイルの解析結果が短時間で反映される
- 一時的な構文エラーでも直前の正常結果を失わない
- watcher の取りこぼしを検索前チェックで修復できる

## Phase 3: Code relationships

目的: 定義・参照・呼び出し・関連テストを探索できるようにする。

- import / export edges
- references
- callers / callees
- public API hash
- 1-hop dependent reindexing
- `find_references`
- `find_callers`
- `find_callees`
- `find_related_tests`

完了条件:

- exported symbol 変更前に主要な影響範囲を取得できる
- 実装と関連テストを同じ探索フローで取得できる

## Phase 4: MCP integration

目的: Codex、Claude Code などから標準的に利用できるようにする。

- `rmcp` server
- stdio transport
- structured tool responses
- token budget enforcement
- MCP instructions
- graceful shutdown
- multi-client safety

完了条件:

- MCP クライアントが ASTral を子プロセスとして起動できる
- エージェントが検索 → 定義確認 → 参照確認を実行できる
- MCP 終了時に DB を破損しない

> 実装順としては Phase 1 の早い段階から MCP の薄い縦切りを入れても構いません。フェーズ番号は責務の成熟順を示します。

## Phase 5: Git integration

目的: commit と Working Tree の状態を明確に扱う。

- HEAD snapshot metadata
- working-tree overlay
- checkout / merge / rewrite detection
- optional Git hooks
- `astral hooks install`
- snapshot status reporting

完了条件:

- branch 切り替え後に古いコードが検索結果へ残らない
- commit 済み状態と未コミット変更を区別して報告できる

## Phase 6: Search quality

目的: 実際の探索失敗データを基に検索精度を改善する。

- camelCase / snake_case tokenizer
- field weighting
- path proximity
- graph-aware ranking
- query explanation
- evaluation dataset
- recall / precision measurement

完了条件:

- 代表的な開発タスクごとに期待コードを上位へ返せる
- 検索品質の変更を自動評価できる

## Phase 7: Semantic search

目的: 語彙が一致しない要求から関連実装を見つける。

- chunk summaries
- embedding provider abstraction
- embedding cache
- vector index
- hybrid reranking

開始条件:

- 全文・symbol・graph 検索の失敗事例が十分に収集されている
- 意味検索が必要なクエリを評価データで説明できる

完了条件:

- embedding が無効でも基本機能が動作する
- model identifier の変更時に安全に再生成できる

## Phase 8: Language-specific precision

目的: Tree-sitter だけでは不足する意味解析を補う。

候補:

- TypeScript Compiler API sidecar
- LSP integration
- SCIP ingestion
- Rust Analyzer integration

完了条件:

- 対象言語で definition / reference の精度が測定可能に改善する
- sidecar が停止しても構文ベース検索へフォールバックできる

## Phase 9: Remote and team mode

目的: 複数人・複数リポジトリで共有する。

- Streamable HTTP
- authentication
- repository authorization
- centralized registry
- audit logs
- resource limits
- PostgreSQL or dedicated search backend evaluation

開始条件:

- ローカルモードの利用価値とデータモデルが安定している
- チーム共有の具体的な要求がある

## MVP の切り方

最初の公開可能な MVP は次を含みます。

- Rust CLI
- TypeScript / TSX の構文解析
- SQLite + FTS5
- symbol-based chunks
- file watcher
- `search_code`
- `find_symbol`
- `read_symbol`
- `get_index_status`
- stdio MCP

次は MVP に含めません。

- embeddings
- remote server
- graph database
- organization-wide search
- code editing tools
- automatic test execution

## 評価シナリオ

開発中は、少なくとも次のタスクで評価します。

1. エラーメッセージから発生箇所を探す
2. UI コンポーネントから利用 API をたどる
3. exported function の参照箇所を列挙する
4. 実装に関連するテストを探す
5. 自然言語の機能説明から主要な実装候補を探す
6. ファイル保存後に新しい symbol を検索する
7. branch 切り替え後に削除済み symbol が出ないことを確認する

ロードマップは実装結果と計測データに応じて更新します。
