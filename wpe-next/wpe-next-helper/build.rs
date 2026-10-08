fn main() {
    pkg_config::Config::new()
        .atleast_version("3.24")
        .probe("gtk+-3.0")
        .unwrap();

    pkg_config::Config::new()
        .atleast_version("2.50")
        .probe("webkit2gtk-4.0")
        .unwrap();

    pkg_config::Config::new()
        .atleast_version("0.1")
        .probe("gtk-layer-shell-0")
        .unwrap();

    println!("cargo:rustc-link-lib=gtk-layer-shell");
}
