use std::{
    collections::HashSet,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use axum::{
    body::Body,
    extract::{OriginalUri, State},
    http::{header, Response, StatusCode, Uri},
    response::Json,
    routing::{get, post},
    Router,
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpListener;

use crate::{
    error::AstralError,
    incremental::IncrementalIndexer,
    index::{IndexStore, RelationshipResult},
    repository::{RegisteredRepository, RepositoryRegistry},
};

#[derive(RustEmbed)]
#[folder = "web/dist"]
struct WebAssets;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Registered repository name to pre-select in the web UI.
    pub repository_name: String,
    /// Host address to bind the web server to.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// Port to bind the web server to.
    #[arg(long, default_value_t = 8080)]
    pub port: u16,
    /// Override the directory that static web assets are served from.
    /// When omitted, assets embedded at compile time are used.
    #[arg(long, env = "ASTRAL_WEB_ASSETS_DIR")]
    pub assets_dir: Option<PathBuf>,
}

#[derive(Clone)]
struct WebState {
    default_repository: String,
    assets_dir: Option<PathBuf>,
}

fn build_app(state: WebState) -> Router {
    let api = Router::new()
        .route("/search", post(api_search))
        .route("/find-symbol", post(api_find_symbol))
        .route("/read-symbol", post(api_read_symbol))
        .route("/graph", post(api_graph))
        .route("/status", get(api_status))
        .route("/refresh", post(api_refresh))
        .with_state(state.clone());

    Router::new()
        .nest("/api", api)
        .fallback(static_handler)
        .with_state(state)
}

pub async fn start(
    args: Args,
) -> crate::Result<(SocketAddr, tokio::task::JoinHandle<crate::Result<()>>)> {
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .map_err(|_| AstralError::InvalidConfiguration {
            message: format!("invalid bind address {}:{}", args.host, args.port),
        })?;

    let state = WebState {
        default_repository: args.repository_name,
        assets_dir: args.assets_dir,
    };

    let listener =
        TcpListener::bind(&addr)
            .await
            .map_err(|source| AstralError::InvalidConfiguration {
                message: format!("failed to bind web server to {addr}: {source}"),
            })?;

    let actual_addr =
        listener
            .local_addr()
            .map_err(|source| AstralError::InvalidConfiguration {
                message: format!("failed to get local address: {source}"),
            })?;

    let app = build_app(state);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .map_err(|source| AstralError::Indexing {
                message: source.to_string(),
            })
    });

    tracing::info!("web ui listening on http://{}", actual_addr);
    Ok((actual_addr, handle))
}

pub async fn serve(args: Args) -> crate::Result<()> {
    let (_addr, handle) = start(args).await?;
    match handle.await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(AstralError::Indexing {
            message: error.to_string(),
        }),
    }
}

#[derive(Deserialize)]
struct SearchRequest {
    repository_name: String,
    query: String,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct ReadSymbolRequest {
    repository_name: String,
    symbol_id: String,
}

#[derive(Deserialize)]
struct GraphRequest {
    repository_name: String,
    symbol: String,
}

#[derive(Deserialize)]
struct RefreshRequest {
    repository_name: String,
}

fn resolve_repository(name: &str) -> crate::Result<RegisteredRepository> {
    RepositoryRegistry::new().resolve(name)
}

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

fn bounded_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT)
}

async fn api_search(Json(req): Json<SearchRequest>) -> Result<Json<Value>, (StatusCode, String)> {
    let limit = bounded_limit(req.limit);
    let result = tokio::task::spawn_blocking(move || {
        let repository =
            resolve_repository(&req.repository_name).map_err(|error| error.to_string())?;
        let results = IndexStore::search_code(&repository.name, repository.root.path(), &req.query)
            .map_err(|error| error.to_string())?;
        let truncated = results.len() > limit;
        let results: Vec<Value> = results
            .into_iter()
            .take(limit)
            .map(|result| {
                json!({
                    "relative_path": result.relative_path,
                    "start_byte": result.start_byte,
                    "end_byte": result.end_byte,
                    "snippet": result.snippet,
                    "score": result.score,
                    "matched_by": result.matched_by,
                    "reason": result.reason,
                })
            })
            .collect();
        Ok::<_, String>(json!({
            "repository_name": repository.name,
            "results": results,
            "truncated": truncated,
        }))
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    result
        .map(Json)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))
}

async fn api_find_symbol(
    Json(req): Json<SearchRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let limit = bounded_limit(req.limit);
    let result = tokio::task::spawn_blocking(move || {
        let repository =
            resolve_repository(&req.repository_name).map_err(|error| error.to_string())?;
        let results = IndexStore::find_symbol(&repository.name, repository.root.path(), &req.query)
            .map_err(|error| error.to_string())?;
        let truncated = results.len() > limit;
        let results: Vec<Value> = results
            .into_iter()
            .take(limit)
            .map(|result| {
                json!({
                    "symbol_id": result.symbol_id,
                    "name": result.name,
                    "qualified_name": result.qualified_name,
                    "kind": result.kind,
                    "relative_path": result.relative_path,
                    "start_byte": result.start_byte,
                    "end_byte": result.end_byte,
                })
            })
            .collect();
        Ok::<_, String>(json!({
            "repository_name": repository.name,
            "results": results,
            "truncated": truncated,
        }))
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    result
        .map(Json)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))
}

async fn api_read_symbol(
    Json(req): Json<ReadSymbolRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let result = tokio::task::spawn_blocking(move || {
        let repository =
            resolve_repository(&req.repository_name).map_err(|error| error.to_string())?;
        let result =
            IndexStore::read_symbol(&repository.name, repository.root.path(), &req.symbol_id)
                .map_err(|error| error.to_string())?;
        Ok::<_, String>(json!({
            "repository_name": repository.name,
            "symbol_id": result.symbol_id,
            "name": result.name,
            "kind": result.kind,
            "relative_path": result.relative_path,
            "source": result.source,
        }))
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    result
        .map(Json)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))
}

#[derive(Serialize)]
struct GraphNode {
    id: String,
    label: String,
    kind: String,
    path: String,
    is_center: bool,
}

#[derive(Serialize)]
struct GraphEdge {
    from: String,
    to: String,
    label: String,
    confidence: f64,
}

async fn api_graph(Json(req): Json<GraphRequest>) -> Result<Json<Value>, (StatusCode, String)> {
    let result = tokio::task::spawn_blocking(move || {
        let repository =
            resolve_repository(&req.repository_name).map_err(|error| error.to_string())?;
        let root = repository.root.path().to_path_buf();
        let graph =
            build_graph(&repository.name, &root, &req.symbol).map_err(|error| error.to_string())?;
        Ok::<_, String>(json!({
            "repository_name": repository.name,
            "symbol": req.symbol,
            "nodes": graph.nodes,
            "edges": graph.edges,
        }))
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    result
        .map(Json)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))
}

struct Graph {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

fn build_graph(repository_name: &str, root: &Path, symbol: &str) -> crate::Result<Graph> {
    let mut graph = Graph {
        nodes: Vec::new(),
        edges: Vec::new(),
    };
    let mut seen: HashSet<String> = HashSet::new();

    let symbols = IndexStore::find_symbol(repository_name, root, symbol)?;
    let center = symbols
        .into_iter()
        .next()
        .ok_or_else(|| AstralError::Indexing {
            message: format!("symbol not found: {symbol}"),
        })?;

    add_node(
        &mut graph,
        &mut seen,
        &center.symbol_id,
        &center.name,
        &center.kind,
        &center.relative_path,
        true,
    );

    type RelationshipSearch =
        fn(&str, &Path, &str) -> crate::Result<Vec<RelationshipResult>>;
    let relationship_searches: [RelationshipSearch; 4] = [
        IndexStore::find_references,
        IndexStore::find_callers,
        IndexStore::find_callees,
        IndexStore::find_related_tests,
    ];

    for search in relationship_searches {
        for result in search(repository_name, root, &center.symbol_id)? {
            let source_id = result.source_symbol_id.clone().unwrap_or_else(|| {
                format!(
                    "external:{}",
                    result.source_name.clone().unwrap_or_default()
                )
            });
            let source_label = result
                .source_name
                .clone()
                .unwrap_or_else(|| "external".to_owned());
            add_node(
                &mut graph,
                &mut seen,
                &source_id,
                &source_label,
                "unknown",
                &result.source_file,
                false,
            );

            let (target_id, target_label, target_path) =
                if let Some(id) = &result.target_symbol_id {
                    (
                        id.clone(),
                        result.target_name.clone().unwrap_or_else(|| {
                            result.target_external_name.clone().unwrap_or_default()
                        }),
                        result.target_file.clone().unwrap_or_default(),
                    )
                } else {
                    (
                        format!(
                            "external:{}",
                            result.target_external_name.clone().unwrap_or_default()
                        ),
                        result
                            .target_external_name
                            .clone()
                            .unwrap_or_else(|| result.target_name.clone().unwrap_or_default()),
                        result.target_file.clone().unwrap_or_default(),
                    )
                };

            add_node(
                &mut graph,
                &mut seen,
                &target_id,
                &target_label,
                "unknown",
                &target_path,
                false,
            );

            graph.edges.push(GraphEdge {
                from: source_id,
                to: target_id,
                label: result.edge_type.clone(),
                confidence: result.confidence,
            });
        }
    }

    Ok(graph)
}

fn add_node(
    graph: &mut Graph,
    seen: &mut HashSet<String>,
    id: &str,
    label: &str,
    kind: &str,
    path: &str,
    is_center: bool,
) {
    if !id.is_empty() && !seen.contains(id) {
        seen.insert(id.to_owned());
        graph.nodes.push(GraphNode {
            id: id.to_owned(),
            label: label.to_owned(),
            kind: kind.to_owned(),
            path: path.to_owned(),
            is_center,
        });
    }
}

async fn api_status(
    State(state): State<WebState>,
    uri: Uri,
) -> Result<Json<Value>, (StatusCode, String)> {
    let repo =
        extract_query_param(&uri, "repository_name").unwrap_or(state.default_repository.clone());
    let result = tokio::task::spawn_blocking(move || {
        let repository = resolve_repository(&repo).map_err(|error| error.to_string())?;
        let status = IndexStore::get_index_status(repository.root.path())
            .map_err(|error| error.to_string())?;
        Ok::<_, String>(json!({
            "repository_name": repository.name,
            "indexed": status.indexed,
            "schema_version": status.schema_version,
            "files": status.file_count,
            "symbols": status.symbol_count,
            "diagnostics": status.diagnostic_count,
            "stale_files": status.stale_count,
            "missing_files": status.missing_count,
            "snapshot_head": status.snapshot_head,
            "snapshot_branch": status.snapshot_branch,
            "working_tree_dirty": status.working_tree_dirty,
            "dirty_files": status.dirty_file_count,
        }))
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    result
        .map(Json)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))
}

async fn api_refresh(Json(req): Json<RefreshRequest>) -> Result<Json<Value>, (StatusCode, String)> {
    let result = tokio::task::spawn_blocking(move || {
        let repository =
            resolve_repository(&req.repository_name).map_err(|error| error.to_string())?;
        let database = IndexStore::default_path(repository.root.path());
        let report = IncrementalIndexer::new(&repository.name, repository.root.path(), &database)
            .refresh()
            .map_err(|error| error.to_string())?;
        Ok::<_, String>(json!({
            "repository_name": repository.name,
            "updated_files": report.updated_files,
            "stale_files": report.stale_files,
            "removed_files": report.removed_files,
            "rebuilt": report.rebuilt,
        }))
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    result
        .map(Json)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))
}

async fn static_handler(
    State(state): State<WebState>,
    OriginalUri(uri): OriginalUri,
) -> Response<Body> {
    let path = uri.path().trim_start_matches('/').trim();
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(assets_dir) = &state.assets_dir {
        return serve_disk_asset(assets_dir, path).await;
    }

    serve_embedded_asset(path)
}

async fn serve_disk_asset(assets_dir: &Path, path: &str) -> Response<Body> {
    if let Some(file_path) = safe_subpath(assets_dir, path) {
        if let Ok(bytes) = tokio::fs::read(&file_path).await {
            let content_type = content_type_for(&file_path);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(bytes))
                .unwrap();
        }
    }

    // Fallback to index.html for non-asset paths (single-page app).
    if !path.starts_with("assets/") && !path.contains('.') {
        if let Some(index_path) = safe_subpath(assets_dir, "index.html") {
            if let Ok(bytes) = tokio::fs::read(&index_path).await {
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                    .body(Body::from(bytes))
                    .unwrap();
            }
        }
    }

    not_built_response()
}

fn serve_embedded_asset(path: &str) -> Response<Body> {
    match WebAssets::get(path) {
        Some(file) => {
            let content_type = file.metadata.mimetype().to_owned();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(file.data.to_vec()))
                .unwrap()
        }
        None => {
            // Fallback to index.html for unknown routes.
            if let Some(file) = WebAssets::get("index.html") {
                let content_type = file.metadata.mimetype().to_owned();
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, content_type)
                    .body(Body::from(file.data.to_vec()))
                    .unwrap()
            } else {
                not_built_response()
            }
        }
    }
}

fn not_built_response() -> Response<Body> {
    const MESSAGE: &str = "<!DOCTYPE html>
<html lang=\"ja\">
<head><meta charset=\"UTF-8\"/><title>ASTral Web</title></head>
<body style=\"background:#0f172a;color:#e2e8f0;font-family:sans-serif;padding:2rem;\">
  <h1>ASTral Web</h1>
  <p>Web UI assets are not built. Run <code>cd web && npm install && npm run build</code>.</p>
</body>
</html>";

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(MESSAGE))
        .unwrap()
}

fn safe_subpath(base: &Path, path: &str) -> Option<PathBuf> {
    let mut result = base.to_path_buf();
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." || segment.contains(':') || segment.contains('\\') {
            return None;
        }
        result.push(segment);
    }
    Some(result)
}

fn content_type_for(path: &Path) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string()
}

fn extract_query_param(uri: &Uri, name: &str) -> Option<String> {
    uri.query().and_then(|query| {
        query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            if key == name {
                Some(value.to_owned())
            } else {
                None
            }
        })
    })
}
