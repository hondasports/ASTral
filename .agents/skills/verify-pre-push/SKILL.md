---
name: verify-pre-push
description: ASTralの変更パスと公開契約に応じてformat、clippy、test、build、fixture、migration、MCP確認を実行し、push可能か判定する。
---

# Verify Before Push

## 目的

必要な検証を証拠付きで完了し、`code-review`へ渡す。

## 検証マトリクスの初稿

コマンド実行前に変更範囲から検証項目を整理する。

```text
変更範囲:
壊れる可能性がある振る舞い:
必須コマンド:
追加fixture・integration・evaluation:
実行不能になりうる項目:
未確定の検証判断:
```

## 基本4本

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-targets --all-features
```

## 変更別の追加確認

- OXC analyzer変更: JS/JSX/TS/TSX、syntax error、scope、import/export fixture
- normalized model・SQLite変更: migrationまたはrebuild、transaction rollback、旧indexの扱い
- watcher・Git変更: create/modify/rename/delete、branch切替、重複イベント
- ranking・tokenizer変更: evaluation queryの変更前後順位と回帰
- MCP変更: tool schema、read-only境界、token budget、stdio終了
- CLI変更: help、exit code、既存引数との互換性

## `grill-me`による洗練

検証マトリクスの初稿に次が残る場合は、コマンド実行前に`grill-me`へ渡す。

- 何を壊す可能性があるか説明できない。
- unit、fixture、integration、process、evaluationの責務が曖昧。
- 代表fixture・evaluation queryの選び方が複数案に分かれる。
- compatibility、migration、rebuildの証明方法が決まっていない。
- 実行不能項目をCI・手動確認・follow-upのどこで補うか決まっていない。

### 洗練手順

1. 差分、GATE0、TDD結果、既存テストから確認済み事項を埋める。
2. 残る検証判断を`grill-me`で一問ずつ確認する。
3. 各質問に推奨回答と、見逃した場合のリスクを添える。
4. `GRILL RESULT`を検証マトリクスへ反映する。
5. 洗練後のマトリクスに従ってコマンドを実行する。
6. 結果を受け、必要ならマトリクスを再改訂する。

利用中のエージェントがSkill chainingに対応しない場合は、ユーザーへ`/grill-me`の実行を依頼する。

変更範囲と標準検証が一意な場合は、壁打ち省略理由を一行記録して進められる。

## 出力

```text
判定: PASS / FAIL / BLOCKED
検証マトリクス初稿:
GRILL RESULT（実施時）:
洗練後の検証マトリクス:
壁打ち省略理由（省略時）:
実行コマンドと結果:
追加確認:
実行不能項目と理由:
検索品質の比較:
次フェーズ: code-review
```

失敗を成功扱いにしない。同じ失敗を2回繰り返したら`stuck-advisor`を使う。
