---
name: tdd-implement
description: GATE0 Go後に、Rustのunit・integration・fixtureテストをRED/GREENで進め、最小差分でASTralの受け入れ条件を満たす。
---

# TDD Implement

## 前提

- 大きな変更では`issue-gate-0`の`Go`とGATE0成果物がある。
- 対象範囲、公開契約、migration、評価方法が明確である。

## Grill Me連携

実装直前に次が残っている場合はコードを書かず、`grill-me`へ戻る。

- 期待する振る舞いを失敗テストとして書けない。
- partial result、stale、errorのどれを返すか決まっていない。
- OXC analyzerとnormalized modelの責務が決まっていない。
- 互換性を維持するかbreaking changeにするか決まっていない。

壁打ち後は`GRILL RESULT`をテスト名・実装範囲へ変換する。

## 手順

1. 受け入れ条件を最小の観測可能な振る舞いへ分解する。
2. fixtureまたはテストを追加し、期待した理由で失敗することを確認する（RED）。
3. 最小の本体変更で対象テストを通す（GREEN）。
4. 対象範囲内だけをrefactorする。
5. OXC diagnostics、SQLite失敗、rename/delete、MCP出力上限など該当する異常系を追加する。
6. 関連文書・ADR・schemaを同じ変更で更新する。

## テスト優先順位

- pureな変換・ranking・hash: unit test
- parserからnormalized model: fixture test
- SQLite transaction・migration: integration test
- watcher・Git・MCP transport: integrationまたはprocess test
- 実リポジトリでの検索品質: evaluation scenario

## 完了条件

```text
RED:
GREEN:
変更ファイル:
追加・更新したテスト:
実行した検証:
GATE0との差異:
未解決事項:
```

同じ失敗を2回繰り返した場合は`stuck-advisor`へ移る。
