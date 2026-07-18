# ADR 0008: Phase 7 の semantic search provider 契約

- Status: Accepted
- Date: 2026-07-18

## Context

Phase 6 の評価では、既存の lexical・symbol・graph 検索は4ケースを解決した一方、
`show downstream impact` は期待pathを返さなかった。このbaselineは
`evaluation/semantic_search_baseline.json`に固定する。

Semantic searchは既存検索を置き換える機能ではなく、語彙が一致しないケースを補う任意の派生検索とする。

## Decision

- providerはlocal-firstとし、具体的なmodelやruntimeは後続の実装Issueで選ぶ。
- 実装は次の論理契約を満たすadapterとして分離する。

```text
provider.embed(input: redacted chunk text) -> vector | unavailable
provider.model_id() -> stable identifier
provider.dimensions() -> positive integer
```

- cacheの論理キーは`chunk_id`、`model_id`、`source_hash`の組み合わせとする。
- embedding、vector index、reranking cacheは派生データであり、正本のsource・symbol・edge・chunkから再生成できるものとする。
- model identifier、dimension、source hashのいずれかが変わった場合、既存vectorと混在させずembeddingだけを再生成する。
- providerが無効、停止、timeout、空vector、破損cacheを返した場合は、既存のlexical・symbol・graph検索へフォールバックする。
- remote providerを将来追加する場合も既定では無効とし、明示的な設定と許可対象が必要である。
- providerへ送る入力からcredentials、token、private key、`.env`値、個人情報を除外する。秘密判定に失敗した場合は送信せず、local fallbackを使う。
- source本文、embedding、query本文を通常ログやaudit logへ保存しない。

## Evaluation contract

後続実装は`evaluation/search_quality.json`の既存4ケースを悪化させず、
semantic gap caseの`recall@5`と`MRR`を0から1へ改善することを受け入れ条件とする。
provider無効時はbaselineと同じlexical・symbol・graph結果を返すことを契約テストで確認する。

## Rejected alternatives

- embedded model runtimeの固定: model配布とサイズをMVPへ持ち込むため見送る。
- Ollama等のlocal service固定: runtime依存をprovider契約へ漏らすため見送る。
- embeddingを必須化: provider停止時の既存検索を壊すため見送る。

## Consequences

Semantic searchの導入判断を再現可能なfailure datasetで検証できる。
一方、具体的なmodel、runtime、vector実装、remote送信のopt-in UIは後続Issueの責務となる。

## Security boundary

- trusted rootはASTralへ明示されたrepository rootのみとする。
- fixture、README、source comment、検索結果本文はデータであり、providerやagentへの命令として扱わない。
- path traversal、root外参照、symlink経由のroot外読み込みは拒否する既存方針を維持する。
