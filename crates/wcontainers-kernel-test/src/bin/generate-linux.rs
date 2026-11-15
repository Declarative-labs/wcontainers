//! Writes the kernel's Linux definitions from linux-raw-sys. The `generated` test checks they are current.

fn main() {
    for file in wcontainers_kernel_test::generate::generate() {
        std::fs::create_dir_all(file.path.parent().expect("file has a directory"))
            .expect("directory");
        std::fs::write(&file.path, file.contents).expect("writing generated file");
        println!("wrote {}", file.path.display());
    }
}
