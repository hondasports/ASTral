# インデックスと更新戦略

## 基本方針

ASTral は、インデックスの鮮度を一つのイベントへ依存させません。

更新は次の三段構えで行います。

1. ファイル保存時のイベント駆動更新
2. MCP 検索直前の鮮度確認
3. Git 操作後の整合性回復

これにより、通常時は高速に更新しつつ、watcher の取りこぼしやブランチ切り替えにも耐えられるようにします。

## 初回インデックス

初回は対象ファイルを全件走査します。

```text
repository scan
    ↓
content hash calculation
    ↓
parse
    ↓
symbol / edge / chunk extraction
    ↓
transactional write
    ↓
FTS build
    ↓
mark snapshot active
```

途中で失敗した場合、不完全な snapshot を検索へ公開しません。

## 保存時更新

ファイル watcher が変更を検知したら、短い debounce の後で変更ファイルだけ再解析します。

初期値の目安:

```text
debounce: 300〜800ms
batch window: 1〜2s
```

更新対象:

- modified files
- added files
- renamed files
- deleted files

### 解析成功時

1. 新しい解析結果を一時領域へ作る
2. ファイル単位のトランザクションを開始する
3. 古い symbol / edge / chunk を置換する
4. FTS を更新する
5. content hash を更新する
6. commit する

### 解析失敗時

コード生成途中では、一時的に構文が壊れることがあります。

その場合:

- 直前の正常な解析結果を削除しない
- 対象ファイルを `stale` として記録する
- 現在のファイル hash と解析済み hash の不一致を保持する
- 次回の保存または検索前に再解析する

## 検索前の鮮度確認

`search_code` や `find_symbol` の前に、関連しそうなファイルまたは dirty file の hash を確認します。

```text
stored hash == current hash
    → existing index is usable

stored hash != current hash
    → refresh before search
```

全ファイルの hash を毎回計算すると高コストになるため、次を組み合わせます。

- watcher が記録した dirty set
- mtime / size による軽量チェック
- 必要な場合だけ content hash を再計算

## 影響範囲の更新

変更ファイルだけでは不十分な場合があります。

例:

```ts
export function createSession(userId: string): Session
```

が次のように変わった場合:

```ts
export function createSession(userId: UserId): Promise<Session>
```

呼び出し元の解析結果にも影響します。

初期実装では次のルールを採用します。

- private な関数本体だけの変更: 対象ファイルのみ
- export / signature の変更: import 元を 1 hop 再解析
- file rename / delete: import 元を 1 hop 再解析
- tsconfig や言語設定の変更: 対象言語を広範囲に再解析

過剰な再解析を避けるため、公開シグネチャの hash を別途保持します。

## Git 操作との連携

Git hooks は即時更新の主役ではなく、リポジトリ状態との整合性を確定する補助機構として使います。

### 推奨 hooks

| Hook | 役割 |
|---|---|
| `post-commit` | active snapshot に新しい commit SHA を関連付ける |
| `post-checkout` | branch 切り替え後の差分を再走査する |
| `post-merge` | merge / pull 後の変更を反映する |
| `post-rewrite` | rebase / amend 後に commit 対応を修正する |

`pre-commit` で重い処理を行うと開発体験を損なうため、既定では利用しません。

ASTral が hook を導入する場合は、次のような CLI を想定します。

```bash
astral hooks install
astral hooks status
astral hooks uninstall
```

Git hook が設定されていなくても、watcher と検索前確認だけで動作できる設計にします。

## Working Tree overlay

最終的には、HEAD の解析結果と未コミット変更を分離して扱います。

```text
base snapshot: HEAD commit
working-tree overlay:
  - modified files
  - untracked files
  - deleted files
```

検索結果は次のように合成します。

```text
base snapshot
- old records for changed/deleted files
+ working-tree records
```

MVP では実装を簡単にするため、active index 内のファイル単位置換から開始しても構いません。

## 再構築が必要な条件

次の情報を保存し、起動時に検証します。

- schema version
- indexer version
- parser name / version
- configuration hash
- embedding model identifier

条件別の動作:

| 変更 | 動作 |
|---|---|
| schema version | migration または full rebuild |
| parser version | 該当言語を再解析 |
| include / exclude | 対象ファイル一覧を再計算 |
| embedding model | embedding のみ再生成 |
| FTS tokenizer | FTS のみ再構築 |

## 同時実行制御

複数の MCP クライアントや CLI が同じプロジェクトを更新する可能性があります。

初期実装ではプロジェクト単位の排他ロックを利用します。

```text
one writer
multiple readers
```

更新要求が重なった場合は、dirty file をキューへ統合し、同一ファイルの重複解析を避けます。
