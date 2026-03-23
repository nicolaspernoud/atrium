// backend/build.rs
use flate2::Compression;
use flate2::write::GzEncoder;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};
use walkdir::WalkDir;

/// Run a command in `dir` and abort the build on failure.
fn run_cmd(dir: &Path, prog: &str, args: &[&str]) -> ExitStatus {
    println!("cargo:warning=running: {} {}", prog, args.join(" "));
    let mut command = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(prog);
        c
    } else {
        Command::new(prog)
    };
    let status = command
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn `{}`: {}", prog, e));

    if !status.success() {
        panic!("command `{}` failed with status {:?}", prog, status);
    }
    status
}

/// Copy **the *contents* of** `src` into `dst` recursively.
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = dst.as_ref().join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(entry.path(), dest_path)?;
        } else {
            fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

/// Gzip every regular file under `base` except `*.gz` and `*.tmpl`.
fn gzip_assets(base: &Path) {
    for entry in WalkDir::new(base).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if file_name.ends_with(".gz") || file_name.ends_with(".tmpl") {
                continue;
            }

            let input_file = fs::File::open(path).expect("failed to open input file");
            let mut gz_path = path.to_path_buf();

            // Append .gz to the current filename
            let mut new_extension = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if !new_extension.is_empty() {
                new_extension.push_str(".gz");
            } else {
                new_extension = "gz".to_string();
            }
            gz_path.set_extension(new_extension);

            let output_file = fs::File::create(&gz_path).expect("failed to create gz file");
            let mut encoder = GzEncoder::new(output_file, Compression::best());
            let mut reader = io::BufReader::new(input_file);
            io::copy(&mut reader, &mut encoder).expect("failed to compress file");
            encoder.finish().expect("failed to finish compression");

            // Remove original file
            fs::remove_file(path).expect("failed to remove original file");
        }
    }
}

fn main() {
    // ---------------------------------------------------------------------------
    // Do nothing if the OUT_DIR environment variable contains "release_optimized"
    // ---------------------------------------------------------------------------
    if let Ok(out_dir) = env::var("OUT_DIR")
        && out_dir.contains("release_optimized")
    {
        println!(
            "cargo:warning=Building in release-optimized mode: skipping build script to avoid running Flutter build and asset copying steps since they are done with Dockerfile"
        );
        return;
    }

    // -----------------------------------------------------------------
    // Tell Cargo when to rerun this script
    // -----------------------------------------------------------------
    // Any change in the backend `web` folder or anything in the frontend
    // should trigger a rebuild.
    println!("cargo:rerun-if-changed=web");
    println!("cargo:rerun-if-changed=../frontend");

    // -----------------------------------------------------------------
    // Detect a Flutter build
    // -----------------------------------------------------------------
    let flutter_build = env::var("FLUTTER_BUILD").is_ok();

    // -----------------------------------------------------------------
    // Common path helpers (all relative to the backend crate directory)
    // -----------------------------------------------------------------
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo manifest dir"));
    let web_dir = manifest_dir.join("web"); // ./backend/web
    let dist_dir = manifest_dir.join("dist"); // ./backend/dist

    if !flutter_build {
        // -----------------------------------------------------------------
        // 1️⃣  Test mode – just copy the static assets
        // -----------------------------------------------------------------
        println!(
            "cargo:warning=Building in test mode: skipping Flutter build and copying web/. → dist/."
        );
        // Clean the old `dist` (if any) and copy `web/ → dist/`
        if dist_dir.exists() {
            fs::remove_dir_all(&dist_dir).ok();
        }
        fs::create_dir_all(&dist_dir).expect("cannot create backend/dist");
        copy_dir_all(&web_dir, &dist_dir).expect("failed to copy web → dist in test mode");
    } else {
        // -----------------------------------------------------------------
        // 2️⃣  Normal (non‑test) build – do what the original shell script did
        // -----------------------------------------------------------------
        println!(
            "cargo:warning=Building in Flutter mode: running Flutter build and copying web/. → dist/."
        );
        // -----------------------------------------------------------------
        // a) Move to the repository root (parent of `backend/`)
        // -----------------------------------------------------------------
        let repo_root = manifest_dir
            .parent()
            .expect("backend must be inside a repository root")
            .to_path_buf();

        // -----------------------------------------------------------------
        // b) Build the Flutter web app
        // -----------------------------------------------------------------
        let frontend_dir = repo_root.join("frontend");
        run_cmd(&frontend_dir, "flutter", &["pub", "get"]);
        run_cmd(&frontend_dir, "flutter", &["build", "web"]);

        // -----------------------------------------------------------------
        // c) Prepare a clean `backend/dist` directory
        // -----------------------------------------------------------------
        if dist_dir.exists() {
            fs::remove_dir_all(&dist_dir).ok();
        }
        fs::create_dir_all(&dist_dir).expect("cannot create backend/dist");

        // -----------------------------------------------------------------
        // d) Copy backend static assets (`backend/web/* → backend/dist/`)
        // -----------------------------------------------------------------
        copy_dir_all(&web_dir, &dist_dir).expect("failed to copy backend/web → backend/dist");

        // -----------------------------------------------------------------
        // e) Copy the Flutter build output (`frontend/build/web/* → backend/dist/`)
        // -----------------------------------------------------------------
        let flutter_build = frontend_dir.join("build").join("web");
        if flutter_build.is_dir() {
            copy_dir_all(&flutter_build, &dist_dir)
                .expect("failed to copy flutter build output into backend/dist");
        } else {
            eprintln!(
                "Error: {} not found. Flutter build might have failed.",
                flutter_build.display()
            );
            std::process::exit(1);
        }
    }

    // -----------------------------------------------------------------
    // 3️⃣ Gzip everything that isn't already gzipped and isn't a template
    // -----------------------------------------------------------------
    gzip_assets(&dist_dir);
}
