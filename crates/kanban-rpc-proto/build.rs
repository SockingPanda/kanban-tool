fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = "../../proto";
    let proto = "../../proto/kanban/framework/v1/board.proto";
    println!("cargo:rerun-if-changed={proto}");
    tonic_prost_build::configure()
        .file_descriptor_set_path(
            std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("kanban.bin"),
        )
        .compile_protos(&[proto], &[root])?;
    Ok(())
}
