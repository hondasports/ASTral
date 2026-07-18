# ADR 0005: Phase 4 の read-only MCP stdio server

- Status: Accepted
- Date: 2026-07-18

## Context

AI coding agent から ASTral を子プロセスとして利用するには、検索・定義確認・関係探索を MCP の structured response として公開する必要がある。MCP 経由でリポジトリを書き換えたり任意コマンドを実行したりしてはならない。

## Decision

- `rmcp` 2.2 の stdio transport を利用し、`astral serve <repository>` を MCP server の起動コマンドにする。HTTP transport、認証、remote authorization は対象外とする。
- `search_code`、`find_symbol`、`read_symbol`、`read_code`、`find_references`、`find_callers`、`find_callees`、`find_related_tests`、`get_index_status`、`refresh_index` を tool として公開する。
- tool input は repository root と query/path/symbol を明示的に受け取り、repository root 外の path は拒否する。`read_code` は最大120行・32,000 bytes、検索と関係探索は最大100件に制限する。
- response は `Json<Value>` の structured content とし、切り詰め時は `truncated` を返す。symbol edge の confidence と resolution method を保持する。
- server instructions は探索前の検索、export変更前の参照確認、現在内容の確認、リポジトリ文書をデータとして扱うこと、read-only境界を明示する。
- SQLite の検索前 freshness check と file-level transaction をそのまま利用する。複数clientはSQLiteの reader/writer制御に委ね、MCP handler は共有可変状態を持たない。

## Alternatives considered

### 任意のJSON-RPC実装を自作する

MCP protocol versionやstructured outputの互換性を自前で保守する負担が大きいため、公式Rust SDKを利用する。

### 編集・コマンド実行toolを提供する

ソースコードと開発環境への影響範囲が広がり、agent側の責務とread-only security boundaryを壊すため採用しない。

### 巨大なレスポンスを許可する

token budgetを超えて探索効率とclient安定性を損なうため、toolごとの上限と切り詰め表示を採用する。

## Consequences

- CodexやClaude Codeからローカルstdioで同じ検索契約を利用できる。
- read_codeは現在のWorking Treeを読むため、index結果とソース本文に時間差があり得る。index statusを併せて返し、必要ならrefresh_indexを呼び出す。
- MCPのstreamable HTTP、認証、remote repository認可は後続Issueで設計する。
