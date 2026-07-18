# Contributing to ASTral

ASTral は現在、設計・初期開発段階です。大きな実装へ入る前に、責務と評価方法を明確にすることを重視します。

## 開発原則

- 小さい縦切りで実装する
- 検索品質を印象ではなく再現可能なケースで評価する
- 解析結果の正本と派生インデックスを分離する
- インデックスが古い可能性を常に扱う
- 解析失敗時に直前の正常データを破壊しない
- MCP ツールは初期段階では読み取り専用にする
- 高度な基盤を、実測なしで先に導入しない

## 変更を始める前に

次の変更は、実装前に Issue または設計ドキュメントで方針を整理してください。

- SQLite schema の大幅変更
- 新しい parser / language analyzer の追加
- MCP tool の追加・破壊的変更
- remote transport の追加
- embedding provider の追加
- 新しい永続化・検索バックエンドの導入

小さなバグ修正、テスト追加、ドキュメント改善は直接 Pull Request にして構いません。

## 開発環境

詳細なセットアップ手順は、[開発環境の構築](docs/development.md)を参照してください。

実装開始後は Rust stable を基本とし、次を標準の検証コマンドとします。

```bash
cargo build --all-targets --all-features
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Cargo workspace や `rust-toolchain.toml` が追加された場合は、リポジトリ内の設定を優先してください。

## AI エージェント用スキル

Codex、Claude Code、Cursor などを利用する場合は、Rust と MCP の設計判断をエージェントへ共有するため、`npx skills` による推奨スキルの導入を検討してください。

```bash
npx skills add https://github.com/leonardomso/rust-skills \
  --skill rust-skills

npx skills add https://github.com/microsoft/skills \
  --skill mcp-builder
```

スキルは ASTral のビルドには必須ではありません。導入前に配布元と `SKILL.md` を確認し、信頼できるものだけを利用してください。対象エージェントの指定や telemetry の無効化を含む詳細は、[開発環境の構築](docs/development.md#ai-エージェント用スキル)に記載しています。

## Pull Request

Pull Request には次を含めてください。

- 変更の目的
- 採用した設計と代替案
- 影響するコンポーネント
- 確認したテスト
- index schema / parser / MCP contract への影響
- 性能や検索品質への影響がある場合、その測定結果

## 検索品質を変更する場合

ranking、tokenizer、chunking、graph expansion を変更する場合、最低限次を記録してください。

1. 入力 query
2. 期待する symbol / file
3. 変更前の順位
4. 変更後の順位
5. 他の代表 query への悪影響

特定の一例だけを改善し、全体の検索品質を落とす変更は避けます。

## Parser を変更する場合

次を確認してください。

- 正常なファイルを解析できる
- 編集途中の不完全なファイルで panic しない
- byte / line range が妥当である
- 同名 symbol を区別できる
- parser version の変更を検出できる
- 再解析対象を必要以上に広げない

## Schema migration

永続化 schema を変更する場合、以下のどちらかを明記してください。

- migration を提供する
- index の再構築を要求する

コードインデックスは再生成可能ですが、無言で互換性を壊さないようにします。

## セキュリティ

次のデータをテスト fixture や Issue に含めないでください。

- API keys
- access tokens
- private keys
- `.env` の実データ
- 非公開リポジトリのソースコード
- 顧客・本番環境のデータ

秘密情報の索引除外は、機能追加より優先して扱います。

## Commit message

Conventional Commits に近い短い形式を推奨します。

```text
feat: add TypeScript symbol extraction
fix: preserve previous index on parse failure
docs: clarify working-tree overlay
refactor: separate normalized store from FTS
```

## ドキュメント

設計と実装が食い違った場合、Pull Request 内で関連ドキュメントも更新してください。

主要ドキュメント:

- `README.md`
- `docs/development.md`
- `docs/architecture.md`
- `docs/indexing.md`
- `docs/storage.md`
- `docs/mcp.md`
- `docs/roadmap.md`
