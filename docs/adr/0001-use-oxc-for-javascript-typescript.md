# ADR 0001: JavaScript / TypeScript 解析に OXC を採用する

- Status: Accepted
- Date: 2026-07-18

## Context

ASTral の最初の主要対象は JavaScript、TypeScript、JSX、TSX のリポジトリです。

コード索引には、単なる構文木だけでなく次の情報が必要です。

- declaration と reference の区別
- lexical scope
- symbol
- import / export
- call expression
- source range
- module resolution
- 編集途中の不完全なコードに対する耐性

当初は複数言語を同一の仕組みで扱いやすい Tree-sitter を標準 parser として想定していました。しかし JavaScript / TypeScript については、Rust ネイティブで parser、AST、semantic analysis、module resolution を同じエコシステムから利用できる OXC の方が、初期実装の目的に合います。

## Decision

JavaScript / TypeScript 系ファイルの第一級 analyzer として OXC を採用します。

対象:

- `.js`
- `.jsx`
- `.mjs`
- `.cjs`
- `.ts`
- `.tsx`
- `.mts`
- `.cts`

想定する OXC crate:

- `oxc_allocator`
- `oxc_ast`
- `oxc_ast_visit`
- `oxc_parser`
- `oxc_semantic`
- `oxc_span`
- `oxc_resolver`

ASTral 内では OXC 固有型を永続化層や MCP 層へ漏らさず、共通の normalized model へ変換します。

```text
source file
    ↓
OXC parser / semantic / resolver
    ↓
AnalysisResult
    ├─ symbols
    ├─ references
    ├─ imports / exports
    ├─ calls
    ├─ diagnostics
    └─ chunks
    ↓
SQLite normalized store
```

複数言語対応のため、解析処理は analyzer abstraction の背後に置きます。

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

想定実装:

- `OxcAnalyzer`: JavaScript / TypeScript / JSX / TSX
- `TreeSitterAnalyzer`: 将来追加するその他言語
- `MarkdownAnalyzer`: README、ADR、設計文書

## TypeScript 型解析の境界

OXC の semantic analysis は、scope、binding、symbol、reference などの構文・語彙的な意味情報に利用します。

一方、次の機能は初期 OXC analyzer の責務に含めません。

- generic の具体化
- overload resolution
- conditional type の評価
- project 全体をまたぐ完全な型推論
- TypeScript compiler と同等の型検査

これらが検索品質へ明確に寄与すると確認できた場合、TypeScript Language Service、TypeScript Compiler API、typescript-go、LSP などを利用する sidecar を追加します。

sidecar が停止していても、OXC ベースの構文・symbol検索へフォールバックできることを必須とします。

## Consequences

### Positive

- Rust プロセス内で JS / TS の解析を完結しやすい
- parser と semantic model の対応を取りやすい
- declaration と reference を構造的に区別しやすい
- module resolution を同一エコシステムで扱える
- TypeScript / TSX を優先する MVP と一致する
- Node.js sidecar を初期必須要件にせずに済む

### Negative

- OXC が対応しない言語には別 analyzer が必要
- analyzer ごとの差異を normalized model で吸収する必要がある
- 完全な TypeScript 型情報は別実装になる
- OXC crate の更新に伴う API 変更を追従する必要がある

## Alternatives considered

### Tree-sitter を全言語の標準 parser にする

複数言語への展開は容易ですが、JS / TS の symbol、scope、module resolution を別途組み立てる作業が増えるため、MVPでは採用しません。

Tree-sitter 自体は不採用ではなく、OXC が対象としない言語の analyzer 候補として残します。

### TypeScript Compiler API を最初から必須にする

高精度な型情報を得られますが、Node.js プロセス管理、通信、起動コスト、障害時フォールバックが必要になります。

初期のコード探索に型検査相当の精度が本当に必要かを測定する前に導入するのは避けます。

### SWC を採用する

Rust 製の JS / TS parser として有力ですが、ASTral が必要とする semantic analysis と resolver を含む全体構成を比較し、OXC を採用します。

## Validation

OXC analyzer の実装時には、少なくとも次を fixture として検証します。

1. JavaScript / TypeScript / JSX / TSX を解析できる
2. function、class、interface、type、component を抽出できる
3. import / export を解決できる
4. declaration と reference を区別できる
5. 同名 symbol を scope ごとに区別できる
6. 編集途中の不完全なファイルで panic しない
7. source range が元ソースと一致する
8. parser / analyzer version の変更時に再索引できる
9. OXC diagnostics があっても取得可能な部分情報を安全に扱える
