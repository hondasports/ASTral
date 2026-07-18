---
name: tdd-implement
description: GATE0 Go後に、Rustのunit・integration・fixtureテストをRED/GREENで進め、最小差分でASTralの受け入れ条件を満たす。
---

# TDD Implement

## 前提

- 大きな変更では`issue-gate-0`の`Go`とGATE0成果物がある。
- 対象範囲、公開契約、migration、評価方法が明確である。

## 実装計画の初稿

コード変更前に、次を短く仮置きする。

```text
観測可能な振る舞い:
最初に失敗させるテスト:
最小実装範囲:
異常系・境界値:
公開契約への影響:
未確定の判断分岐:
```

## `grill-me`による洗練

実装計画の初稿に次が残っている場合はコードを書かず、`grill-me`へ渡す。

- 期待する振る舞いを失敗テストとして書けない。
- partial result、stale、errorのどれを返すか決まっていない。
- OXC analyzerとnormalized modelの責務が決まっていない。
- 互換性を維持するかbreaking changeにするか決まっていない。
- fixture、unit、integration、evaluationのどこで証明するか曖昧。
- 実装を小さく切る境界が複数案に分かれている。

### 洗練手順

1. 既存コード、ADR、schema、fixtureから解決できる論点を調査する。
2. 残る分岐だけを`grill-me`へ渡す。
3. 一度に1問ずつ確認し、各質問に推奨回答を添える。
4. `GRILL RESULT`をテスト名、期待値、実装範囲、非対象へ変換する。
5. 洗練後の計画でREDを作れることを確認してから編集する。

利用中のエージェントがSkill chainingに対応しない場合は、ユーザーへ`/grill-me`の実行を依頼し、結果を受け取るまで実装を開始しない。

小さなバグ修正で失敗テストと修正範囲が既に一意なら、壁打ち省略理由を一行記録して進められる。

## TDD手順

1. 受け入れ条件を最小の観測可能な振る舞いへ分解する。
2. fixtureまたはテストを追加し、期待した理由で失敗することを確認する（RED）。
3. 最小の本体変更で対象テストを通す（GREEN）。
4. 対象範囲内だけをrefactorする。
5. OXC diagnostics、SQLite失敗、rename/delete、MCP出力上限など該当する異常系を追加する。
6. 関連文書・ADR・schemaを同じ変更で更新する。
7. 洗練後の計画と実装差分を照合し、逸脱を明示する。

## テスト優先順位

- pureな変換・ranking・hash: unit test
- parserからnormalized model: fixture test
- SQLite transaction・migration: integration test
- watcher・Git・MCP transport: integrationまたはprocess test
- 実リポジトリでの検索品質: evaluation scenario

## 完了条件

```text
実装計画の初稿:
GRILL RESULT（実施時）:
洗練後の実装計画:
壁打ち省略理由（省略時）:
RED:
GREEN:
変更ファイル:
追加・更新したテスト:
実行した検証:
GATE0との差異:
未解決事項:
```

同じ失敗を2回繰り返した場合は`stuck-advisor`へ移る。
