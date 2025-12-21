fn main() {
    ensure_windows_icon();
    tauri_build::build()
}

fn ensure_windows_icon() {
    if !cfg!(target_os = "windows") {
        return;
    }

    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let icons_dir = manifest_dir.join("icons");
    let dest_ico = icons_dir.join("icon.ico");

    if dest_ico.exists() {
        return;
    }

    let _ = std::fs::create_dir_all(&icons_dir);

    // Avoid committing binary assets: generate the required .ico during build
    // from an existing PNG already present in the repo.
    let source_png = manifest_dir
        .join("../../wardenly-go/resources/icons/app_256.png")
        .canonicalize();

    let source_png = match source_png {
        Ok(p) => p,
        Err(_) => {
            // If the source icon isn't available, we don't hard fail here; tauri-build
            // will still error with a clear message, which is fine for CI environments.
            println!("cargo:warning=tap-tauri build: missing source PNG for icon generation");
            return;
        }
    };

    let img = match image::open(&source_png) {
        Ok(i) => i.to_rgba8(),
        Err(e) => {
            println!("cargo:warning=tap-tauri build: failed to read source PNG: {e}");
            return;
        }
    };

    let (w, h) = img.dimensions();
    let icon_image = ico::IconImage::from_rgba_data(w, h, img.into_raw());

    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    match ico::IconDirEntry::encode(&icon_image) {
        Ok(entry) => dir.add_entry(entry),
        Err(e) => {
            println!("cargo:warning=tap-tauri build: failed to encode ICO: {e}");
            return;
        }
    }

    match std::fs::File::create(&dest_ico) {
        Ok(mut file) => {
            if let Err(e) = dir.write(&mut file) {
                println!("cargo:warning=tap-tauri build: failed to write ICO: {e}");
            } else {
                println!("cargo:warning=tap-tauri build: generated {}", dest_ico.display());
            }
        }
        Err(e) => println!("cargo:warning=tap-tauri build: failed to create ICO file: {e}"),
    }
}


