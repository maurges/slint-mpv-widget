fn main() {
    slint_build::compile("ui/main.slint").unwrap();

    println!("cargo:rustc-link-lib=mpv");
}
