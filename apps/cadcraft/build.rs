//! Windows only: embed the app icon and version info into `cadcraft.exe`. Elsewhere a no-op.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/cadcraft.ico");
    println!("cargo:rerun-if-env-changed=CADCRAFT_REQUIRE_WINRES");
    println!("cargo:rerun-if-env-changed=CADCRAFT_BUILD_SHA");
    println!("cargo:rerun-if-env-changed=CADCRAFT_BRAND_NAME");
    println!("cargo:rerun-if-env-changed=CADCRAFT_BRAND_PUBLISHER");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    if std::path::Path::new("../../assets/app-icon/cadcraft.ico").exists() {
        res.set_icon("../../assets/app-icon/cadcraft.ico");
    }
    let brand = std::env::var("CADCRAFT_BRAND_NAME").ok().filter(|s| !s.is_empty());
    let publisher = std::env::var("CADCRAFT_BRAND_PUBLISHER").ok().filter(|s| !s.is_empty());
    let product = brand.clone().unwrap_or_else(|| "CADCraft".into());
    let description = match &brand {
        Some(name) => format!("{name} (based on CADCraft)"),
        None => "CADCraft computer-aided design".into(),
    };
    let company = publisher.clone().unwrap_or_else(|| "Learning Machines LLC".into());
    let copyright = match &publisher {
        Some(p) => format!("Copyright (c) 2026 ArtCraft Team and the CADCraft contributors; modifications (c) 2026 {p}. MIT OR Apache-2.0."),
        None => "Copyright (c) 2026 ArtCraft Team and the CADCraft contributors. MIT OR Apache-2.0.".into(),
    };
    res.set("ProductName", &product)
        .set("FileDescription", &description)
        .set("CompanyName", &company)
        .set("LegalCopyright", &copyright)
        .set("OriginalFilename", "cadcraft.exe")
        .set("InternalName", "cadcraft");
    // A rebranded build's own version (CADCraft's plus the publisher's revision, 0.18.0.2).
    println!("cargo:rerun-if-env-changed=CADCRAFT_BRAND_VERSION");
    if let Some(v) = std::env::var("CADCRAFT_BRAND_VERSION").ok().filter(|v| !v.is_empty()) {
        res.set("FileVersion", &v).set("ProductVersion", &v);
        let n: Vec<u64> = v.split('.').map(|p| p.parse().unwrap_or(0)).chain(std::iter::repeat(0)).take(4).collect();
        let packed = (n[0] << 48) | (n[1] << 32) | (n[2] << 16) | n[3];
        res.set_version_info(winresource::VersionInfo::FILEVERSION, packed);
        res.set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);
    }
    if let Err(e) = res.compile() {
        if std::env::var_os("CADCRAFT_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=cadcraft.exe built without icon/version resources: {e}");
    }
}
