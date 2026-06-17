use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let web_root = manifest_dir.join("../../web");
    let client_root = web_root.clone();
    let source = client_root.join("dist");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let destination = out_dir.join("web-assets");

    emit_rerun_triggers(&web_root, &client_root);

    if client_root.join("package.json").is_file() {
        build_web_client_if_needed(&web_root, &client_root, &source);
    }

    let _ = fs::remove_dir_all(&destination);
    fs::create_dir_all(&destination).expect("create embedded web asset directory");
    if source.join("index.html").is_file() {
        copy_dir(&source, &destination).expect("copy kanban web assets");
    } else {
        write_fallback_index(&destination).expect("write fallback kanban web asset");
        println!(
            "cargo:warning=kanban web/dist not found after build; embedding fallback web page"
        );
    }

    println!(
        "cargo:rustc-env=KANBAN_WEB_ASSET_DIR={}",
        destination.display()
    );
}

fn emit_rerun_triggers(web_root: &Path, client_root: &Path) {
    for path in [
        client_root.join("package.json"),
        client_root.join("package-lock.json"),
        client_root.join("tsconfig.json"),
        client_root.join("vite.config.ts"),
        client_root.join("index.html"),
        client_root.join("dist"),
        client_root.join("src"),
        web_root.join("shared"),
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn build_web_client_if_needed(web_root: &Path, client_root: &Path, dist: &Path) {
    let index = dist.join("index.html");
    if index.is_file() && !web_sources_newer_than(web_root, client_root, &index) {
        return;
    }

    if !client_root.join("node_modules").is_dir() {
        run_npm(client_root, &["install"]);
    }
    run_npm(client_root, &["run", "build"]);
}

fn web_sources_newer_than(web_root: &Path, client_root: &Path, target: &Path) -> bool {
    let target_modified = modified_time(target).unwrap_or(SystemTime::UNIX_EPOCH);
    [
        client_root.join("package.json"),
        client_root.join("package-lock.json"),
        client_root.join("tsconfig.json"),
        client_root.join("vite.config.ts"),
        client_root.join("index.html"),
        client_root.join("src"),
        web_root.join("shared"),
    ]
    .iter()
    .filter_map(|path| newest_modified_time(path).ok().flatten())
    .any(|modified| modified > target_modified)
}

fn newest_modified_time(path: &Path) -> io::Result<Option<SystemTime>> {
    if !path.exists() {
        return Ok(None);
    }
    if path.is_file() {
        return modified_time(path).map(Some);
    }

    let mut newest = modified_time(path).ok();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        if child
            .file_name()
            .is_some_and(|name| name == "node_modules" || name == "dist")
        {
            continue;
        }
        if let Some(modified) = newest_modified_time(&child)?
            && newest.is_none_or(|current| modified > current)
        {
            newest = Some(modified);
        }
    }
    Ok(newest)
}

fn modified_time(path: &Path) -> io::Result<SystemTime> {
    fs::metadata(path)?.modified()
}

fn run_npm(current_dir: &Path, args: &[&str]) {
    let status = Command::new(npm_program())
        .args(args)
        .current_dir(current_dir)
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "failed to run npm {} in {}: {error}",
                args.join(" "),
                current_dir.display()
            )
        });
    if !status.success() {
        panic!(
            "npm {} failed with status {status} in {}",
            args.join(" "),
            current_dir.display()
        );
    }
}

fn npm_program() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

fn copy_dir(source: &Path, destination: &Path) -> io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&destination_path)?;
            copy_dir(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

fn write_fallback_index(destination: &Path) -> io::Result<()> {
    fs::write(
        destination.join("index.html"),
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>kanban web assets missing</title>
  </head>
  <body>
    <main>
      <h1>kanban web assets missing</h1>
      <p>Build the Vite web app before compiling the kanban binary.</p>
    </main>
  </body>
</html>
"#,
    )
}
