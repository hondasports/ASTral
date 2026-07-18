---
name: verify-pre-push
description: ASTralの変更パスと公開契約に応じてformat、clippy、test、build、fixture、migration、MCP確認を実行し、push可能か判定する。
---

# Verify Before Push

## 目的

必要な検証を証拠付きで完了し、`code-review`へ渡す。

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

## Grill Me連携

検証範囲が決められない場合は、コマンド実行前に`grill-me`で次を一問ずつ決める。

- 何を壊す可能性があるか。
- どのtest levelで証明するか。
- どのfixture・evaluation queryを代表例にするか。
- 実行不能項目をどこで補うか。

`GRILL RESULT`を検証マトリクスへ変換する。明確な標準検証に形式的な壁打ちは不要。

## 出力

```text
判定: PASS / FAIL / BLOCKED
変更範囲:
実行コマンドと結果:
追加確認:
実行不能項目と理由:
検索品質の比較:
次フェーズ: code-review
```

失敗を成功扱いにしない。同じ失敗を2回繰り返したら`stuck-advisor`を使う。
