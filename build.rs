#[cfg(windows)]
fn main() {
    use std::io::Cursor;
    use std::path::PathBuf;

    let png_path = "crates/gpui/examples/image/app-icon.png";
    println!("cargo:rerun-if-changed={}", png_path);
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-search=native=deps/libmpv");

    let png_bytes = std::fs::read(png_path).expect("read app-icon.png");
    let img = image::load_from_memory(&png_bytes).expect("decode app-icon.png");

    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16u32, 24, 32, 48, 64, 128, 256] {
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3).to_rgba8();
        let (w, h) = resized.dimensions();
        let image_data = ico::IconImage::from_rgba_data(w, h, resized.into_raw());
        let entry = ico::IconDirEntry::encode(&image_data).expect("encode ico entry");
        icon_dir.add_entry(entry);
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let ico_path = PathBuf::from(&out_dir).join("app-icon.ico");
    let mut buf = Cursor::new(Vec::new());
    icon_dir.write(&mut buf).expect("write ico");
    std::fs::write(&ico_path, buf.into_inner()).expect("save ico");

    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().expect("ico path utf-8"));
    res.compile().expect("compile windows resource");
}

#[cfg(not(windows))]
fn main() {}
