# ADR 0011: CLI動作履歴の構造化ログとローテーション

- 状態: Accepted
- 日付: 2026-07-18
- 対象: CLI、index/watchの進捗、ローカル運用ログ

## Context

`astral index` と `astral watch` の進捗を実行中の stderr で確認できるだけでなく、後から動作履歴を調査できる必要がある。ログは複数リポジトリを扱う同一ユーザー環境に共有されるため、標準出力の検索結果と混ぜず、機械処理可能な形式で保存する。一方、解析対象のソース本文、検索クエリ、credentials、private dataを動作履歴へ記録してはならない。

## Decision

### 保存場所とローテーション

- ログファイルは`ASTRAL_DATA_DIR`が設定されていればそのディレクトリ、未設定ならOS標準のASTralユーザーデータディレクトリへ保存する。
- ファイル名は`astral.log`、サイズ上限は10 MiBとする。
- 上限を超えるレコードを書き込む前に、`astral.log`を`astral.log.1`へ退避し、既存バックアップを`.2`〜`.5`へ繰り上げる。
- 保持するバックアップは最大5世代（`astral.log.1`〜`astral.log.5`）とし、`.6`以降は削除する。
- ローテーションはレコード単位で行い、JSON Linesの1行を分割しない。
- ローテーションやファイルオープンに失敗した場合は、ログ欠落を黙って許容せず、CLI起動エラーとして返す。

### 出力書式

- `astral.log`はUTF-8のJSON Lines（1イベント1行）とする。
- `timestamp`、`level`、`fields.message`、`target`を基本フィールドとする。イベント固有の値は`fields`へ追加する。
- stderrは従来どおり人間向けのテキストとし、CLIのstdout（検索結果・ステータス・JSON評価結果）の契約は変更しない。
- `RUST_LOG`でstderrとファイルのログレベルを同じように制御する。既定値は`astral=info`。

### 出力内容

次のイベントを記録する。

- コマンドの開始・完了・失敗（command、repository）
- 全件indexの開始・対象ファイル数・ファイル単位進捗・完了（repository、current、total、files、symbols、diagnostics、elapsed_ms）
- watchの差分更新開始・完了（repository、updated、stale、removed、elapsed_ms）
- 必要なエラー分類（command、repository、対象パス、エラー種別）

次の値は記録しない。

- ソースコード本文、snippet、検索クエリ、MCP payload
- credentials、token、authorization header、private data
- diagnosticsのメッセージを記録する場合もソース本文を含めない

相対ファイルパスは進捗特定に必要な場合だけ記録し、リポジトリを識別する`repository`と組み合わせる。絶対パスはコマンドの対象リポジトリを識別するために使用するが、ソース本文は保存しない。

## Alternatives considered

- stderrだけ: 実行後の監査・障害調査ができないため不採用。
- plain text file: JSON Linesより機械処理とフィールド追加の互換性が劣るため不採用。
- 日次ローテーション: CLIの実行量ではサイズ上限の方が予測しやすく、保持量を直接制御できるため不採用。
- 外部ログサービス: local-firstのCLI契約、credential境界、self-hosted方針に反するため不採用。

## Consequences

- ユーザーはstderrでリアルタイム進捗を見ながら、後から`astral.log`をJSONとして集計できる。
- 同じユーザーデータ領域を共有するため、ログにはrepositoryフィールドが必須となる。
- ログファイルはプロセスごとに開かれ、同一ログパスを複数プロセスが同時にローテーションする運用は現時点で保証しない。複数プロセスでの同時利用を正式サポートする前に、プロセス間ロックまたはログ出力先分離を追加する。
- 10 MiBの現行ログと最大5世代のバックアップを保持するため、ログ領域は最大およそ60 MiB（ファイルシステム上の追加分を除く）となる。

## Verification

- integration testで1行ごとのJSON解析、command/indexing lifecycle、repositoryフィールド、ソース本文非包含を確認する。
- unit testで10 MiB相当の閾値処理（テストでは小さい上限に縮小）、`.1`〜`.5`の世代繰り上げ、`.6`非保持を確認する。
- CLI stdoutは既存テストで確認し、stderr進捗は既存の`RUST_LOG=astral=info`テストで確認する。
