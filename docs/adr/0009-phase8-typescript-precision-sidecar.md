# ADR 0009: Phase 8 の TypeScript precision sidecar

- Status: Accepted
- Date: 2026-07-18

## Context

OXCは初期の構文、scope、symbol、reference、module resolutionに適しているが、
genericの具体化、overload resolution、conditional type、project-wide inferenceは
TypeScript compiler相当の精度を保証しない。対象ケースは
`evaluation/precision_sidecar.json`とfixtureに固定する。

## Decision

- 最初のprecision拡張候補はTypeScript Language Serviceだけとする。
- sidecarはASTralのRust processへ型を公開せず、normalized modelへ変換するadapterとして扱う。
- 実装時はrepository rootと明示されたtsconfigを対象に、変更・依存関係から必要なファイルだけを問い合わせる。
- sidecarの論理出力は、既存のnormalized `symbol`、`reference`、`edge`、`diagnostic`と、
  `analyzer_name`、`analyzer_version`、`resolution_method`、`confidence`を含む。
- TypeScript Language Serviceで解決した事実は`confidence=1.0`、OXCの構文・semantic結果は既存方針どおり`confidence=0.7`相当として由来を保持する。
- sidecarのtimeout、crash、protocol error、不完全入力、対象外projectでは、OXC結果を保持して検索を継続する。
- sidecarが返した結果とOXC結果が一致しない場合、無言で上書きせず、analyzer由来とdiagnosticをstatusへ残す。
- sidecarはlocal processとして動作し、repository sourceを外部networkへ送信しない。

## Contract tests

次のケースをOXC baselineとsidecar候補の比較単位とする。

- generic instantiationの定義・reference
- overload signatureと実装の解決
- conditional typeの分岐結果
- 複数fileをまたぐ型・symbolの推論
- sidecar停止、timeout、不完全入力時のOXC fallback

既存のJS/TS/TSX fixtureとrelationship検索は回帰対象とし、sidecar停止時にも結果を失わないことを必須とする。

## Rejected alternatives

- TypeScript Compiler API、typescript-go、LSPの同時導入: 比較対象を増やしすぎるため初回候補から除外する。
- 他言語analyzerの同時導入: OXC不足の測定結果と無関係な範囲を広げるため見送る。
- OXCをsidecarで置き換える設計: 既存のlocal-first fallbackを壊すため採用しない。

## Migration and compatibility

今回の設計確定ではSQLite schema、MCP schema、CLI契約を変更しない。
将来sidecar結果を保存する場合は、既存のanalyzer metadataとschema世代を更新し、
旧indexを無言で混在させず必要な再解析またはfull rebuildを実行する。
