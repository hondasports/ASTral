# ADR 0002: Phase 1 の SQLite 索引と再構築

- Status: Accepted
- Date: 2026-07-18

## Context

Phase 1 では、OXC の normalized な解析結果を保存し、コード全文検索と symbol 検索を提供する必要がある。検索前に復元できる正本を持ち、解析途中の壊れたデータを active index として公開してはならない。

## Decision

- 永続化の正本は `rusqlite` の SQLite とする。
- `rusqlite` は `bundled-full` を有効にし、Windows・macOS・Linux で外部 SQLite の導入を必須にしない。
- schema version は v1 とし、v1 の schema 変更は migration を用意せず full rebuild とする。
- rebuild は一時 DB に `files`、`symbols`、`references_index`、`imports`、`exports`、`calls`、`diagnostics`、`chunks` を書き、FTS5 の `chunk_search` を構築する。
- 全処理と transaction が成功してから、一時 DB を active path へ置き換える。失敗時は active DB を変更しない。
- index は repository 内に作らず、OS の user data directory 配下の project id ディレクトリに保存する。
- OXC crate version は `0.140.0`、`oxc_resolver` は `11.24.2` を Phase 1 の基準とする。変更時は full rebuild を要求する。

## Alternatives considered

### 既存 DB への直接更新

解析途中の失敗で active index が部分的になるため採用しない。

### 外部 SQLite への依存

開発者ごとに native library の導入差分が発生するため、`bundled-full` を採用する。

### schema migration を先に実装する

Phase 1 の schema は再生成可能であり、migration の複雑さに対して得られる価値が小さいため full rebuild を採用する。

## Consequences

- 初回 rebuild は全ファイルを解析するが、失敗時の復旧が単純になる。
- schema や parser version の更新時に rebuild 時間が必要になる。
- FTS5 は SQLite の派生 index であり、正規化テーブルから再構築できる。
- watcher、incremental update、call graph、MCP はこの ADR の対象外である。
