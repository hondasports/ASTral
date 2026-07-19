use std::{fs, path::Path};

fn main() {
    let dist = Path::new("web/dist");
    let index = dist.join("index.html");

    if !index.exists() {
        fs::create_dir_all(dist).ok();
        fs::write(
            &index,
            concat!(
                "<!DOCTYPE html>",
                "<html lang=\"ja\">",
                "<head><meta charset=\"UTF-8\"/><title>ASTral Web</title></head>",
                "<body style=\"background:#0f172a;color:#e2e8f0;font-family:sans-serif;padding:2rem;\">",
                "<h1>ASTral Web</h1>",
                "<p>Web UI assets are not built. Run <code>cd web && npm install && npm run build</code> first.</p>",
                "</body></html>"
            ),
        )
        .ok();
    }

    println!("cargo:rerun-if-changed=web/dist/index.html");
    println!("cargo:rerun-if-changed=web/package.json");
}
