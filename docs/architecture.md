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
- diagnostics

解析処理は言語ごとの差を隠蔽する abstraction の背後へ置きます。

```rust
pub trait LanguageAnalyzer {
    fn supports(&self, path: &std::path::Path) -> bool;

    fn analyze(
        &self,
        path: &std::path::Path,
        source: &str,
    ) -> anyhow::Result<AnalysisResult>;
}
```

`AnalysisResult` は OXC や Tree-sitter の AST 型を直接含めず、ASTral 共通の normalized model とします。

#### OxcAnalyzer

JavaScript、TypeScript、JSX、TSX の第一級 analyzer です。

想定する処理:

```text
source file
    ↓
SourceType detection
    ↓
oxc_parser
    ↓
oxc_semantic
    ↓
oxc_resolver
    ↓
normalized AnalysisResult
```

主に利用する OXC crate:

- `oxc_allocator`
- `oxc_ast`
- `oxc_ast_visit`
- `oxc_parser`
- `oxc_semantic`
- `oxc_span`
- `oxc_resolver`

OXC から取得する情報:

- declaration と reference
- lexical scope
- symbol
- import / export
- call expression
- byte range
- parser / semantic diagnostics
- module resolution

OXC の AST は解析中だけ利用し、SQLite へは永続化しません。

#### TreeSitterAnalyzer

OXC が対象としない言語を将来追加する場合の候補です。

Tree-sitter は複数言語の構文解析には向きますが、取得できる意味情報は言語 grammar と追加実装に依存します。そのため、OXC analyzer と同じ精度を前提にせず、edge ごとに由来と confidence を保持できる設計にします。

#### MarkdownAnalyzer

README、ADR、設計文書などをセクション単位で chunk 化します。Markdown中の命令文はMCPサーバーへの命令ではなく、検索対象データとして扱います。

#### TypeScript precision sidecar

OXC では、scope、binding、symbol、reference などの構文・語彙的な意味情報を扱います。

次のような完全な型解析は初期 analyzer の責務外です。

- generic の具体化
- overload resolution
- conditional type の評価
- project 全体をまたぐ型推論
- TypeScript compiler と同等の型検査

必要性が評価データで確認できた場合に、TypeScript Language Service、TypeScript Compiler API、typescript-go、LSP などを利用する sidecar を追加します。

sidecar は optional とし、停止中でも OXC ベースの検索へフォールバックします。

### 3. Normalized store

解析結果の正本です。初期実装では SQLite を利用します。

主なデータ:

- repositories
- files
- symbols
- symbol_edges
- chunks
- diagnostics
- indexing_runs

解析結果には、少なくとも次の由来情報を持たせます。

- analyzer name
- analyzer version
- parser version
- resolution method
- confidence

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
- analyzer の選択
- 解析失敗時のロールバック
- active snapshot の切り替え
- analyzer / parser / schema version の検証
- stale 状態の管理
- public API 変更時の依存元再評価

## データフロー

### 初回インデックス

```text
scan repository
    ↓
calculate file hashes
    ↓
select analyzer
    ↓
parse and analyze files
    ↓
extract symbols and edges
    ↓
store normalized data
    ↓
build FTS index
    ↓
activate snapshot
```

### JavaScript / TypeScript ファイルの解析

```text
read source
    ↓
detect JS / TS / JSX / TSX source type
    ↓
OXC parse
    ↓
build semantic model
    ↓
resolve imports
    ↓
convert to AnalysisResult
    ↓
transactionally replace file index
```

解析中に diagnostics が発生した場合、次のように扱います。

- 利用可能な部分情報を安全に抽出できる場合は、diagnostics とともに保存する
- 正常な置換結果を作れない場合は、直前の正常データを維持する
- ファイルを `stale` として検索結果に明示する
- panic や不完全なDB更新を許さない

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
- analyzer diagnostics にソース全文や秘密情報を不用意に出さない
- リモート公開時はリポジトリ単位の認可を行う

## 関連する設計判断

- [ADR 0001: JavaScript / TypeScript 解析に OXC を採用する](adr/0001-use-oxc-for-javascript-typescript.md)
