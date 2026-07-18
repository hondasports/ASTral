# ADR 0006: Phase 5 の Git snapshot と Working Tree overlay

- Status: Accepted
- Date: 2026-07-18

## Decision

- schema version は v4 とし、metadataへ HEAD OID、branch、Working Tree dirty状態、dirty file数、status hashを保存する。v3からはfull rebuildする。
- 現在のindexは常にWorking Treeを正本とする。commit済みsnapshotと未コミット差分はGit metadataで区別して報告する。
- 検索前のrefreshでHEAD OIDを比較し、checkout、merge、rebase、rewriteなどでHEADが変わった場合はfull rebuildする。同一HEADでの差分はPhase 2のhash・scanner更新へ委ねる。
- Git commandが利用できない、detached HEAD、hook未導入の場合もindexing自体は失敗させず、利用可能なGit metadataだけをstatusへ返す。
- Git hooksの自動導入やcommit・branch操作はこの実装では行わない。

## Consequences

- branch切替後に旧HEAD由来の検索結果を残しにくくなる。
- Working Treeの変更は既存のincremental indexで検索へ反映され、snapshotは状態説明に使われる。
- Git status取得の軽微なコストが検索前に追加される。
