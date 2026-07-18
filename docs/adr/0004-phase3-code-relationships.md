# ADR 0004: Phase 3 の code relationships

- Status: Accepted
- Date: 2026-07-18

## Context

Phase 1/2 は symbol、reference、call、import、export の事実を保存するが、影響範囲を辿るための統一 edge と検索契約をまだ持たない。完全な TypeScript 型推論を導入せず、OXC の semantic 情報と module resolver を利用して、開発者と AI coding agent が 1-hop の関係を安定して取得できる必要がある。

## Decision

- schema version は v3 とし、`symbol_edges` と `files.public_api_hash` を追加する。v2 からの自動 migration は行わず full rebuild する。
- edge は `reference`、`call`、`import`、`export`、`test` の5種類とする。edge は `source_file_id` / `source_symbol_id` と `target_file_id` / `target_symbol_id` を持ち、解決できない対象は `target_external_name` に保持する。
- `confidence` と `resolution_method` を必須にする。OXC の semantic または resolver で symbol まで到達した edge は `1.0` / `semantic`、AST上の同一ファイル名解決は `0.7` / `ast`、未解決名やテスト命名規則は `0.4` / `heuristic` とする。
- edge は file-level transaction の中で全件再生成する。既存 edge を一部だけ残さず、commit 前の edge は検索から見えない状態を保つ。循環は重複排除と検索側の visited 集合で扱う。
- `public_api_hash` は sorted export 名と local 名から計算する。公開APIが変わった場合だけ、`imports.resolved_path` で直接参照する importer を最大1-hop再解析する。private body変更では依存元を再解析しない。
- 関連テストは `tests/`、`__tests__/`、`.test.`、`.spec.`、`_test` の命名規則で同名の実装ファイルへ紐付け、推定 edge として扱う。

## Alternatives considered

### graph databaseを導入する

1〜3 hop のローカル探索には SQLite の edge table で足り、配布と transaction の複雑性が増えるため採用しない。

### 未解決edgeを捨てる

dynamic import、外部 package、編集中の不完全コードで影響範囲が欠落するため、external name と低い confidence を保持する。

### 全importerを再解析する

privateな実装変更でも全体を再解析することになり、incremental indexing の利点を失うため、public API hash と直接 importer に限定する。

## Consequences

- 定義・参照・caller/callee・関連テストを同じ edge model から検索できる。
- 型推論や overload resolution は行わないため、confidence を結果に表示して推定結果と確定結果を区別する。
- edge 全件再生成は file-level update より計算量が増えるが、整合性と実装の単純性を優先する。
