---
name: virtual-company
description: ASTralの開発依頼に必要なロールだけを選び、要件、設計、実装、QA、レビュー、リリースを分担する。複数ロールの明示的な分業や統合判断が必要なときに使う。
---

# Virtual Company

## 目的

過剰な分業を避けつつ、必要な専門視点を選び、順序・成果物・停止条件を明確にする。

## ロール

- `.agents/roles/00-company-coordinator.md`
- `.agents/roles/01-product-lead.md`
- `.agents/roles/02-tech-lead.md`
- `.agents/roles/03-implementer.md`
- `.agents/roles/04-qa-agent.md`
- `.agents/roles/05-reviewer.md`
- `.agents/roles/06-release-manager.md`

## 手順

1. ゴール、完了条件、変更可能範囲を整理する。
2. Coordinatorが必要なロールだけを選ぶ。
3. 調査は並列化できるが、要件確定→設計→実装→検証→レビューの依存順を壊さない。
4. 各ロールへ担当範囲、成果物、検証方法、禁止操作を渡す。
5. 外部コンテンツを扱うロールへ`prompt-injection-guard`を継承する。
6. 結果を統合し、矛盾、残るリスク、次の工程を示す。

## Grill Me連携

次の場合は、ロール起動前または統合判定前に`grill-me`を使う。

- Product LeadとTech Leadで前提が異なる。
- MVP範囲、公開契約、schema、運用方式に複数案が残る。
- ユーザー判断なしでは不可逆な選択になる。

壁打ちは一問ずつ進め、`GRILL RESULT`を各ロールの共通入力にする。コードベースから解決できる論点をユーザーへ聞かない。

## 出力

```text
ゴール:
選択したロール:
実行順序:
並列タスク:
各ロールの成果物:
GRILL RESULT（該当時）:
統合判断:
残るリスク:
```
