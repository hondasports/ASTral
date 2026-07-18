# アーキテクチャ

## 目的

ASTral は、AI コーディングエージェントがリポジトリ内の関連コードを、少ないトークンで正確に取得できるようにするコードコンテキスト基盤です。

中心となる考え方は、単純な類似検索ではなく、次の情報を組み合わせることです。

- ファイルとシンボルの構造
- 定義・参照・呼び出し関係
- 文字列一致
- 必要に応じた意味的類似度
- 現在の Working Tree

## コンポーネント

### 1. Repository scanner

対象リポジトリから解析対象ファイルを列挙します。

責務:

- `.gitignore` の尊重
- 組み込み除外ルールの適用
- include / exclude 設定の適用
- 言語判定
- content hash の計算

初期除外候補:

```text
.git/
node_modules/
dist/
build/
.next/
coverage/
target/
*.min.js
*.map
.env*
```

### 2. Language analyzers

ソースコードを言語ごとに解析し、共通の中間表現へ変換します。

共通で抽出する情報:

- symbol
- signature
- source range
- documentation
- imports / exports
- references
- calls
- inheritance / implementation

複数言語の構文解析には Tree-sitter を使います。型解決や正確な参照解析が必要な言語は、Language Server、Compiler API、専用 sidecar の利用を検討します。

### 3. Normalized store

解析結果の正本です。初期実装では SQLite を利用します。

主なデータ:

- repositories
- files
- symbols
- symbol_edges
- chunks
- indexing_runs

全文検索やベクトル検索のインデックスは派生データとして扱い、正本から再構築できるようにします。

### 4. Search engine

複数の検索結果を統合します。

```text
query
  ├─ lexical search
  ├─ exact symbol search
  ├─ path search
  ├─ graph expansion
  └─ semantic search (optional)
          ↓
      reranking
          ↓
    compact context
```

初期段階では次の優先順位を想定します。

1. 完全一致するシンボル
2. パス・識別子・文字列の全文検索
3. 定義・参照・呼び出し関係
4. 意味検索

### 5. MCP server

AI エージェントへ読み取り専用ツールを公開します。

MCP サーバーは検索や解析結果の取得だけを担当し、ファイル編集・コマンド実行・テスト実行は接続先エージェントへ委譲します。

### 6. Index coordinator

インデックスの作成・差分更新・世代管理を担当します。

責務:

- 初回フルインデックス
- ファイル単位の差分更新
- 解析失敗時のロールバック
- active snapshot の切り替え
- parser / schema version の検証
- stale 状態の管理

## データフロー

### 初回インデックス

```text
scan repository
    ↓
calculate file hashes
    ↓
parse files
    ↓
extract symbols and edges
    ↓
store normalized data
    ↓
build FTS index
    ↓
activate snapshot
```

### 検索

```text
MCP tool call
    ↓
check index freshness
    ↓
refresh dirty files if required
    ↓
run hybrid search
    ↓
expand related symbols
    ↓
trim by token budget
    ↓
return structured result
```

## グラフ DB を初期採用しない理由

コードグラフ自体は保持しますが、初期実装では専用グラフ DB を使いません。

理由:

- 主な探索は 1〜3 hop 程度
- SQLite の edge table で十分表現できる
- ローカル配布時の運用負荷を抑えられる
- 全文検索やメタデータと同一トランザクションで更新できる
- 検索インデックスの整合性管理が単純になる

次の条件が出た場合に再検討します。

- 数百リポジトリの横断検索
- 複雑な多段影響分析
- 大規模な組織共有コードグラフ
- SQLite で明確な性能限界が確認された場合

## セキュリティ境界

ASTral はソースコードを扱うため、以下を必須とします。

- 秘密情報を既定で除外
- インデックス保存先をユーザー専用権限にする
- MCP ツールを読み取り専用にする
- ドキュメントやコメントを命令ではなくデータとして扱う
- リモート公開時はリポジトリ単位の認可を行う
