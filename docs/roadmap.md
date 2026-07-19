# ロードマップ

## 方針

ASTral は、最初からベクトル検索や複数リポジトリ対応まで作り込まず、コード探索に必要な最小機能から段階的に実装します。

各フェーズは、デモではなく実際のリポジトリで評価できる状態を完了条件にします。

JavaScript / TypeScript 系ファイルは OXC を第一級 analyzer とし、他言語は analyzer abstraction を通じて段階的に追加します。

## Phase 0: Project foundation

目的: Rust プロジェクトと開発規約を整える。

- Cargo workspace の作成
- CLI エントリポイント
- logging / tracing
- error model
- configuration loading
- analyzer abstraction
- normalized `AnalysisResult`
- test framework
- CI

想定コマンド:

```bash
astral --help
astral register my-repo .
astral status my-repo
```

完了条件:

- ローカルと CI で build / test / lint が通る
- repository root を安全に解決できる
- analyzer 固有型を永続化層へ漏らさずに解析結果を表現できる

## Phase 1: OXC file and symbol index

目的: TypeScript / TSX を中心に、全文・symbol 検索を成立させる。

- repository scanner
- `.gitignore` 対応
- OXC integration
  - `oxc_parser`
  - `oxc_semantic`
  - `oxc_resolver`
- JS / JSX / TS / TSX source type detection
- file / symbol / chunk extraction
- declaration / reference extraction
- import / export extraction
- parser diagnostics handling
- SQLite schema
- SQLite FTS5
- initial full indexing
- `search_code`
- `find_symbol`
- `read_symbol`
- `get_index_status`

完了条件:

- 中規模 TypeScript リポジトリを索引できる
- JavaScript / TypeScript / JSX / TSX の主要構文を解析できる
- 関数名・コンポーネント名・エラー文字列から目的のコードを取得できる
- declaration と reference を scope ごとに区別できる
- import先を主要な tsconfig / package 構成で解決できる
- diagnostics を含むファイルで panic しない
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
- OXC diagnostics 発生時の部分結果または直前正常版の維持

完了条件:

- 保存後、変更ファイルの解析結果が短時間で反映される
- 一時的な構文エラーでも直前の正常結果を失わない
- watcher の取りこぼしを検索前チェックで修復できる

## Phase 3: Code relationships

目的: 定義・参照・呼び出し・関連テストを探索できるようにする。

- import / export edges
- OXC semantic reference edges
- callers / callees
- module resolution metadata
- public API hash
- 1-hop dependent reindexing
- edge source / confidence
- `find_references`
- `find_callers`
- `find_callees`
- `find_related_tests`

完了条件:

- exported symbol 変更前に主要な影響範囲を取得できる
- 実装と関連テストを同じ探索フローで取得できる
- 構文上の推定edgeとsemanticに解決されたedgeを区別できる

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

要件・provider境界は[ADR 0008](adr/0008-phase7-semantic-search.md)で確定した。具体的なmodel/runtimeの実装は、
`evaluation/semantic_search_baseline.json`の受け入れ条件を満たす後続Issueで行う。

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

## Phase 8: Language-specific precision and expansion

TypeScript precision sidecarの第一候補とfallback契約は[ADR 0009](adr/0009-phase8-typescript-precision-sidecar.md)で確定した。
OXC不足fixtureは`evaluation/precision_sidecar.json`で管理する。

目的: OXC の範囲を超える型解析を補い、他言語を追加する。

TypeScript 型解析候補:

- TypeScript Language Service sidecar
- TypeScript Compiler API sidecar
- typescript-go based sidecar
- LSP integration
- SCIP ingestion

他言語 analyzer 候補:

- Tree-sitter based analyzer
- Rust Analyzer integration
- 言語専用 compiler / language server

開始条件:

- OXC ベース検索で不足する具体的な評価ケースがある
- 型情報や他言語対応が検索順位・参照精度を改善すると測定できる

完了条件:

- 対象言語で definition / reference の精度が測定可能に改善する
- sidecar や追加analyzerが停止しても既存の検索機能を破壊しない
- analyzerごとの精度差をmetadataで説明できる

## Phase 9: Remote and team mode

self-hosted、OIDC/JWT、repository単位SQLiteの境界は[ADR 0010](adr/0010-phase9-remote-team-mode.md)で確定した。
remote transportの実装・credential発行・本番公開は後続Issueの対象とする。

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
- analyzer abstraction
- OXC による JavaScript / TypeScript / JSX / TSX 解析
- SQLite + FTS5
- symbol-based chunks
- file watcher
- `search_code`
- `find_symbol`
- `read_symbol`
- `get_index_status`
- stdio MCP

次は MVP に含めません。

- TypeScript compiler と同等の完全な型解析
- Tree-sitter による他言語対応
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
8. 同名 symbol を lexical scope ごとに区別する
9. `.ts`, `.tsx`, `.js`, `.jsx` の解析結果を共通モデルで取得する
10. 編集途中の構文エラー後も直前の正常な検索結果を利用できる

ロードマップは実装結果と計測データに応じて更新します。
