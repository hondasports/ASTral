# ADR 0010: Phase 9 の self-hosted remote/team mode

- Status: Accepted
- Date: 2026-07-18

## Context

ASTralのlocal stdio modeはソースとindexを開発マシン内で扱う。team利用を追加する場合も、
認証なしendpointやrepository境界を越える参照を許可してはならない。

## Decision

- remote modeはself-hostedを前提とし、local stdio modeの既存契約を変更しない。
- 認証は外部IdPが発行するOIDC/JWT bearer tokenの検証を必須とする。static tokenのみの運用、認証なしendpoint、credentialの永続保存は許可しない。
- 認可は`tenant -> repository_id -> allowed operation`の順に評価する。
- server設定で許可されたtrusted rootとrepository registryだけを参照対象とし、path traversal、symlink経由のroot外参照、別tenantのrepository ID指定を拒否する。
- backendは`SQLite per repository`を採用する。global SQLiteへ複数tenantのsourceを混在させず、repositoryごとにindexとwriter lockを分離する。
- remote transportはStreamable HTTPの後続実装であり、今回の設計確定ではendpointやcredentialを公開しない。

## Security and operations contract

- audit eventにはprincipal、tenant、repository_id、operation、result、request_id、timestamp、latencyのみを保存し、source本文、query本文、token、credential、embeddingは保存しない。
- repository削除要求はindex、derived data、repository registryを削除対象として扱い、削除結果をauditへ記録する。auditの保持期間は運用者設定に従う。
- request size、result count、同時client数、indexing時間、index容量に上限を設け、超過時は明示的なresource errorを返す。
- 認証失敗、権限不足、repository越境、削除要求、resource exhaustionを受け入れテストの必須シナリオとする。
- source本文を含むdiagnosticやlogを通常ログへ出力しない既存方針をremoteでも維持する。

## Backend trade-off

`SQLite per repository`はlocal-firstの保存モデルと実装を共有でき、repository単位の削除と境界分離が容易である。
一方、PostgreSQLと比べてcross-repository集計、水平スケール、高い同時書き込みには制約がある。
同時client数、容量、indexing待ち時間が運用上限を超えた場合に限り、PostgreSQLを再評価する。

## Compatibility

- stdio transport、既存read-only MCP tool、local filesystemのtrusted rootは変更しない。
- remote modeはlocal modeからsourceを自動同期せず、repository authorizationを通過したindexだけを公開する。
- migration、credential発行、本番公開、managed service運用は今回の対象外とする。

## Rejected alternatives

- static token認証: rotationとtenant分離が弱いため見送る。
- mTLS固定: 閉域用途には有効だが、初期の外部IdP連携を阻害するため第一方式にはしない。
- PostgreSQL先行: local-firstの小規模self-hosted要件に対して運用負荷が大きいため後段へ送る。
