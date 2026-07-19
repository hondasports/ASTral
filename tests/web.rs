use std::fs;

use astral::{index::IndexStore, repository::RepositoryRegistry, web::Args};
use tempfile::tempdir;

#[tokio::test]
async fn web_server_exposes_api_endpoints() {
    let repository = tempdir().expect("temporary repository");
    let data_dir = repository.path().join(".astral-data");
    std::env::set_var("ASTRAL_DATA_DIR", &data_dir);

    fs::create_dir(repository.path().join(".git")).expect("git directory");
    fs::write(
        repository.path().join("value.ts"),
        "export const value = 42;\n",
    )
    .expect("value source");
    fs::write(
        repository.path().join("app.tsx"),
        "import { value } from './value';\nexport function App() { return value; }\n",
    )
    .expect("app source");

    RepositoryRegistry::new()
        .register("web-test", repository.path(), false)
        .expect("register repository");

    let database = IndexStore::default_path(repository.path());
    IndexStore::rebuild_at("web-test", repository.path(), &database).expect("rebuild index");

    let args = Args {
        repository_name: "web-test".into(),
        host: "127.0.0.1".into(),
        port: 0,
        assets_dir: None,
    };
    let (addr, handle) = astral::web::start(args).await.expect("start web server");

    let client = reqwest::Client::new();
    let base = format!("http://{addr}");

    let status = client
        .get(format!("{base}/api/status?repository_name=web-test"))
        .send()
        .await
        .expect("status request")
        .json::<serde_json::Value>()
        .await
        .expect("status json");
    assert!(status["indexed"].as_bool().unwrap_or(false));

    let search = client
        .post(format!("{base}/api/search"))
        .json(&serde_json::json!({
            "repository_name": "web-test",
            "query": "value",
        }))
        .send()
        .await
        .expect("search request")
        .json::<serde_json::Value>()
        .await
        .expect("search json");
    assert!(!search["results"].as_array().unwrap().is_empty());

    let symbols = client
        .post(format!("{base}/api/find-symbol"))
        .json(&serde_json::json!({
            "repository_name": "web-test",
            "query": "App",
        }))
        .send()
        .await
        .expect("find-symbol request")
        .json::<serde_json::Value>()
        .await
        .expect("find-symbol json");
    let app_symbol = symbols["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"].as_str() == Some("App"))
        .expect("App symbol");
    let symbol_id = app_symbol["symbol_id"].as_str().unwrap();

    let read = client
        .post(format!("{base}/api/read-symbol"))
        .json(&serde_json::json!({
            "repository_name": "web-test",
            "symbol_id": symbol_id,
        }))
        .send()
        .await
        .expect("read-symbol request")
        .json::<serde_json::Value>()
        .await
        .expect("read-symbol json");
    assert!(read["source"].as_str().unwrap().contains("function App"));

    let graph = client
        .post(format!("{base}/api/graph"))
        .json(&serde_json::json!({
            "repository_name": "web-test",
            "symbol": "App",
        }))
        .send()
        .await
        .expect("graph request")
        .json::<serde_json::Value>()
        .await
        .expect("graph json");
    assert!(!graph["nodes"].as_array().unwrap().is_empty());

    let refresh = client
        .post(format!("{base}/api/refresh"))
        .json(&serde_json::json!({
            "repository_name": "web-test",
        }))
        .send()
        .await
        .expect("refresh request")
        .json::<serde_json::Value>()
        .await
        .expect("refresh json");
    assert!(refresh["updated_files"].is_i64());

    let index = client
        .get(format!("{base}/"))
        .send()
        .await
        .expect("index request");
    assert!(index.status().is_success());
    let body = index.text().await.expect("index body");
    assert!(body.contains("ASTral Web") || body.contains("astral"));

    handle.abort();
}
