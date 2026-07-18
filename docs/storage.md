# 解析結果の保存方式

## 結論

初期実装では SQLite を解析結果の正本にします。

全文検索やベクトル検索は、SQLite の正規化データから再構築できる派生インデックスとして扱います。

Phase 3 の schema version は `3` とし、schema・OXC analyzer version・除外設定が変わった場合は migration ではなく full rebuild を実行します。

Phase 1 では `rusqlite` の `bundled-full` を利用するため、SQLite の native library を別途インストールしなくても Windows・macOS・Linux で同じ手順を実行できます。Phase 2 の schema version は `2` とし、schema・OXC analyzer version・除外設定が変わった場合は migration ではなく full rebuild を実行します。

rebuild は一時 DB と FTS5 を作成し、transaction の commit 後に active DB と置き換えます。解析またはファイル読み込みが失敗した場合、一時 DB だけを破棄し、直前の active DB は保持します。

差分更新では `file_states` が file 単位の鮮度を表します。`stale` は直前の正常な正規化データを保持したまま検索対象から除外する状態、`missing` は Working Tree から削除された状態です。

`symbol_edges` は定義・参照・呼び出し・import/export・関連テストの関係を保存します。未解決対象も外部名と confidence を保持し、検索結果で推定関係と解決済み関係を区別します。

```text
source repository
    ↓ parse
SQLite
    ├─ repositories
    ├─ snapshots
    ├─ files
    ├─ symbols
    ├─ symbol_edges
    ├─ chunks
    └─ indexing_runs

Derived indexes
    ├─ SQLite FTS5
    └─ vector index (optional)
```

## 保存場所

既定ではリポジトリ内へ生成物を置かず、OS のユーザーデータディレクトリへ保存します。

Linux の例:

```text
~/.local/share/astral/
├─ registry.sqlite
└─ projects/
   └─ <project-id>/
      ├─ index.sqlite
      └─ state.json
```

macOS では Application Support、Windows では Local AppData 相当を利用します。

`project-id` は canonical path と Git remote を組み合わせて生成します。複数 worktree の識別方法は実装時に確定します。

## データ分類

### 正本

- repository metadata
- file metadata
- symbol definitions
- symbol relationships
- code chunks
- index state

### 派生データ

- FTS index
- embeddings
- vector index
- reranking cache
- query cache

派生データは削除しても再生成できる必要があります。

## 想定スキーマ

以下は方向性を示す論理スキーマです。実装時に型や制約を調整します。

### repositories

```sql
CREATE TABLE repositories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL,
    git_remote TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### snapshots

```sql
CREATE TABLE snapshots (
    id TEXT PRIMARY KEY,
    repository_id TEXT NOT NULL,
    commit_sha TEXT,
    working_tree_hash TEXT,
    parser_version TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    configuration_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (repository_id) REFERENCES repositories(id)
);
```

`status` の候補:

- `building`
- `active`
- `failed`
- `obsolete`

### files

```sql
CREATE TABLE files (
    id INTEGER PRIMARY KEY,
    snapshot_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    language TEXT,
    content_hash TEXT NOT NULL,
    public_api_hash TEXT,
    size_bytes INTEGER NOT NULL,
    modified_at INTEGER,
    index_status TEXT NOT NULL,
    is_generated INTEGER NOT NULL DEFAULT 0,
    UNIQUE(snapshot_id, relative_path)
);
```

### symbols

```sql
CREATE TABLE symbols (
    id INTEGER PRIMARY KEY,
    snapshot_id TEXT NOT NULL,
    file_id INTEGER NOT NULL,
    symbol_key TEXT NOT NULL,
    name TEXT NOT NULL,
    qualified_name TEXT,
    kind TEXT NOT NULL,
    signature TEXT,
    visibility TEXT,
    documentation TEXT,
    start_byte INTEGER NOT NULL,
    end_byte INTEGER NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    content_hash TEXT NOT NULL
);
```

### symbol_edges

```sql
CREATE TABLE symbol_edges (
    id INTEGER PRIMARY KEY,
    snapshot_id TEXT NOT NULL,
    source_symbol_id INTEGER NOT NULL,
    target_symbol_id INTEGER,
    target_external_name TEXT,
    edge_type TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    metadata_json TEXT
);
```

`edge_type` の候補:

- `references`
- `calls`
- `imports`
- `exports`
- `extends`
- `implements`
- `tests`
- `reads_table`
- `writes_table`
- `uses_env`

`confidence` は解析手法の確度を示します。

- Compiler / LSP で確定: `1.0`
- AST から推定: `0.7`
- 文字列規則から推定: `0.4`

### chunks

```sql
CREATE TABLE chunks (
    id INTEGER PRIMARY KEY,
    snapshot_id TEXT NOT NULL,
    file_id INTEGER NOT NULL,
    symbol_id INTEGER,
    chunk_type TEXT NOT NULL,
    start_byte INTEGER NOT NULL,
    end_byte INTEGER NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    content TEXT NOT NULL,
    summary TEXT,
    content_hash TEXT NOT NULL,
    token_count INTEGER
);
```

MVP ではチャンク本文も保存します。これにより、検索結果の返却時に毎回ファイルを開く必要がなくなり、解析時点の内容も保持できます。

## AST 全体を保存しない理由

AST 全体は原則保存しません。

- データ量が大きい
- parser version に強く依存する
- 言語ごとに形式が異なる
- ソースから再生成できる
- 検索時に必要なのは抽出された一部の事実である

必要な場合だけ対象ファイルを再解析します。

## 全文検索

MVP は SQLite FTS5 を利用します。

```sql
CREATE VIRTUAL TABLE chunk_search USING fts5(
    chunk_id UNINDEXED,
    path,
    symbol_name,
    qualified_name,
    content,
    summary,
    tokenize = 'unicode61'
);
```

コード識別子は検索用に正規化します。

```text
createUserSession
→ createUserSession create user session

create_user_session
→ create_user_session create user session
```

SQLite FTS5 で性能が不足した場合に Tantivy などを追加しますが、正本にはしません。

## Embedding

Embedding は任意機能です。保存する場合は model identifier と対象テキストの hash を必須にします。

```sql
CREATE TABLE embeddings (
    chunk_id INTEGER NOT NULL,
    model_id TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    source_hash TEXT NOT NULL,
    vector_blob BLOB,
    created_at TEXT NOT NULL,
    PRIMARY KEY (chunk_id, model_id)
);
```

モデルが変わった場合、既存 vector と混在させず再生成します。

## トランザクション

ファイル単位の更新は一つのトランザクションで行います。

```text
parse into memory
    ↓ success
BEGIN
    delete old file-derived records
    insert new symbols / edges / chunks
    update FTS
    update file hash
COMMIT
```

解析に失敗した場合は DB を変更せず、既存レコードを保持します。

## 世代管理

最低限、次の二世代を保持します。

- active: 現在検索に利用する正常版
- previous: 直前の正常版

大規模な full rebuild は別 DB または別 snapshot として構築し、完成後に active を切り替えます。

## 保守 CLI

```bash
astral status .
astral refresh .
astral rebuild .
astral clean .
astral gc
```

`clean` は対象プロジェクトの再生成可能なデータを削除します。`gc` は長期間アクセスされていないプロジェクトインデックスを整理します。

## ファイル権限

解析結果にはソースコードが含まれるため、保存先はユーザー専用権限とします。

Unix 系の目安:

```text
directory: 0700
files:     0600
```
