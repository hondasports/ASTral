# ADR 0003: Phase 2 の incremental indexing

- Status: Accepted
- Date: 2026-07-18

## Context

Phase 1 は repository 全体を一時 SQLite へ再構築する。Working Tree の保存を短時間で反映するには、変更ファイルだけを置き換えつつ、watcher の取りこぼしや編集中の構文エラーに耐える必要がある。

## Decision

- file watcher は `notify` を利用し、OS 固有 API は watcher 層へ閉じ込める。
- 既定の debounce は 500ms、batch の最大待機時間は 1秒とする。dirty path は正規化した相対 path の集合として重複排除する。
- 変更ファイルごとに `BEGIN IMMEDIATE` から commit までを1 transactionとする。symbol、reference、import、export、call、diagnostic、chunk、FTS の古い行と file 行を同じ transaction で置換する。
- `file_states` に `fresh`、`stale`、`missing` を記録する。構文診断または読み込み失敗時は直前の正常データを削除せず、対象を `stale` として検索結果から除外する。新規ファイルは状態だけを保持する。
- delete と rename は、現在の scanner 結果と DB の path 差分から判定する。旧 path の正本を削除し、新 path は通常の追加として解析する。
- watcher の取りこぼしを前提に、検索前に全対象ファイルの hash と DB hash を比較する。差分があれば同じ file-level update を実行する。
- schema version は v2 とする。schema version、indexer version、除外設定が一致しない場合は incremental update ではなく Phase 1 の full rebuild を行う。
- active DB の世代は metadata に保持し、file-level update 中の未commit 行は検索から見えない SQLite transaction 境界を使う。Git snapshot、working-tree overlay、relationship graph は対象外とする。

## Alternatives considered

### 変更ファイルを active DB へ直接書き込む

子テーブルや FTS の一部だけが更新されると検索結果が壊れるため採用しない。

### 構文診断のある新しい結果へ置換する

保存途中の不完全コードで直前の正常結果を失うため、stale 状態を記録して正常版を保持する。

### watcher だけを鮮度の根拠にする

OS通知の取りこぼしやプロセス停止があるため、検索前 hash check を併用する。

## Consequences

- 通常の保存では変更ファイルだけを解析するため、full rebuild より短時間で反映できる。
- 検索前の scanner と hash 比較には一定の I/O コストがある。
- stale file は検索から一時的に除外され、次回の保存または検索前 refresh で復旧する。
- schema v1 の index は v2 と互換にせず、安全のため full rebuild を要求する。
