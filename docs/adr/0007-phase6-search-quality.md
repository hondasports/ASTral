# ADR 0007: Phase 6 の search quality evaluation と ranking

- Status: Accepted
- Date: 2026-07-18

## Decision

- evaluation dataset は repository内のJSONで管理し、caseごとに query、期待path集合、評価対象kを固定する。外部ソースや秘密情報はdatasetへ含めない。
- datasetには、既存の語彙・path・graph検索で解決できる成功例に加えて、`show downstream impact`のような自然言語queryと期待pathの語彙が一致しないsemantic gap例を含める。semantic gapは導入前の失敗値として記録し、semantic機能の導入根拠に使う。
- 成功指標は precision@k、recall@k、MRR とし、期待path集合に対する重複除外後のhitで計算する。期待が空のcaseはrecallを0とする。
- rankingは FTS rank を基礎に、camelCase / snake_caseを分解したtoken一致、path近接、symbol edgeの接続数を加点する。同点は score、path、byte offset の順で安定化する。
- 各結果に score、matched_by、reason を返し、順位の根拠を確認可能にする。graph edgeは接続数を上限付きで加点し、embeddingは今回対象外とする。
- astral evaluate <repository> [dataset] で同じdatasetを再実行し、変更前後のreportを比較できる。代表caseをCIで実行する。

## Consequences

- 検索品質の変更を数値と再現可能なfixtureで評価できる。
- 自然文の意味検索やembeddingは未対応で、token/pathが一致しないqueryは改善しない。
- scoreの絶対値はdataset内の比較用で、異なる検索実装間の絶対的な確率ではない。
