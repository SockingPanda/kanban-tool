fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protos = [
        "proto/kanban/v1/kanban.proto",
        "proto/kanban/v1/query.proto",
        "proto/kanban/extensions/v1/workspace.proto",
    ];
    for path in [
        "proto/kanban/v1/common.proto",
        "proto/kanban/v1/dto.proto",
        "proto/kanban/v1/kanban.proto",
        "proto/kanban/v1/query.proto",
        "proto/kanban/extensions/v1/workspace.proto",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    tonic_prost_build::configure()
        .btree_map(".")
        // 完整响应 oneof 只在编码期间存在；Hub 保留有字节预算的编码，不缓存此枚举。
        .enum_attribute(
            ".kanban.v1.QueryResult.result",
            "#[allow(clippy::large_enum_variant)]",
        )
        .file_descriptor_set_path(
            std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("kanban_v1.bin"),
        )
        .compile_protos(&protos, &["proto"])?;
    Ok(())
}
