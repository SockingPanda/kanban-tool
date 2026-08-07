//! CLI release package 构建器。
//!
//! 这里故意不引入 `tempfile`、`libc` 或其他新的依赖。package 命令只在
//! `scripts/cargo-build-lock.sh` 已经持有共享 Cargo lock 时运行，并把所有
//! 可写的 target 数据限制到 wrapper 传入的共享 target root。该边界依赖
//! cooperative/dedicated target owner：no-follow、inode 和 prefix 校验可以
//! 防止误写及普通协作式漂移，但不承诺阻止同 UID 恶意进程、CAP_DAC_OVERRIDE
//! 或同 inode ABA 替换。

use std::{
    env,
    ffi::OsStr,
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use xtask::ToolResult;

const PACKAGE_NAME: &str = "kanban-tool-cli";
const BINARY_NAME: &str = "kanban";
const CLI_PACKAGE: &str = "kanban-cli";
const REVISION: &str = "1";
const STAGE_PREFIX: &str = ".kanban-cli-deb.";
const TEMP_PREFIX: &str = ".kanban-cli-package.";

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

#[derive(Debug, Default)]
struct PackageOptions {
    root: Option<PathBuf>,
    format: String,
    build_args: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
    #[serde(default)]
    version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
}

#[derive(Debug)]
struct OwnedDirectory {
    path: PathBuf,
    parent: PathBuf,
    parent_identity: Identity,
    identity: Identity,
    prefix: &'static str,
}

impl OwnedDirectory {
    fn create(parent: &Path, prefix: &'static str) -> ToolResult<Self> {
        validate_absolute_directory(parent, "private directory parent", true)?;
        if prefix.is_empty()
            || prefix == "."
            || prefix == ".."
            || prefix.contains('/')
            || prefix.contains('\\')
        {
            return Err(error(format!(
                "private directory prefix 不安全: {prefix:?}"
            )));
        }

        let parent_metadata = regular_directory_metadata(parent, "private directory parent")?;
        let parent_identity = identity(&parent_metadata);
        for attempt in 0..128u32 {
            let nonce = unique_nonce(attempt);
            let path = parent.join(format!("{prefix}{nonce:016x}"));
            match fs::create_dir(&path) {
                Ok(()) => {
                    let metadata = fs::symlink_metadata(&path)?;
                    if !metadata.is_dir() {
                        return Err(error(format!(
                            "new private directory is not a directory: {}",
                            path.display()
                        )));
                    }
                    set_mode(&path, 0o700)?;
                    let owned = Self {
                        path,
                        parent: parent.to_owned(),
                        parent_identity,
                        identity: identity(&metadata),
                        prefix,
                    };
                    owned.verify()?;
                    return Ok(owned);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error.into());
                }
            }
        }
        Err(error(format!(
            "无法在 {} 下分配 private directory",
            parent.display()
        )))
    }

    fn verify(&self) -> ToolResult<()> {
        if self.path.parent() != Some(self.parent.as_path()) {
            return Err(error(format!(
                "private directory parent 发生漂移: {}",
                self.path.display()
            )));
        }
        let parent = regular_directory_metadata(&self.parent, "private directory parent")?;
        if identity(&parent) != self.parent_identity {
            return Err(error(format!(
                "private directory parent identity 发生漂移: {}",
                self.parent.display()
            )));
        }
        let metadata = fs::symlink_metadata(&self.path).map_err(|error| {
            error_with_path("private directory 不存在或不可检查", &self.path, error)
        })?;
        if !metadata.is_dir() || identity(&metadata) != self.identity {
            return Err(error(format!(
                "private directory identity 发生漂移: {}",
                self.path.display()
            )));
        }
        let name = self
            .path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| error("private directory 名称不是 UTF-8"))?;
        if !name.starts_with(self.prefix) {
            return Err(error(format!(
                "private directory prefix 发生漂移: {}",
                self.path.display()
            )));
        }
        Ok(())
    }

    fn cleanup(&self) -> ToolResult<()> {
        if !self.path.exists() && !self.path.is_symlink() {
            return Ok(());
        }
        self.verify()?;
        validate_tree(&self.path, "owned private directory")?;
        fs::remove_dir_all(&self.path).map_err(|error| {
            error_with_path("清理 owned private directory 失败", &self.path, error)
        })
    }
}

impl Drop for OwnedDirectory {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!(
                "warning: 保留未验证的 private directory {}: {error}",
                self.path.display()
            );
        }
    }
}

/// 入口：`xtask package cli [options]`。
pub(crate) fn run(arguments: &[String]) -> ToolResult<()> {
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print_usage();
        return Ok(());
    }
    let options = parse_options(arguments)?;
    if options.format != "deb" {
        return Err(error("目前仅支持 --format deb"));
    }

    let root = options.root.unwrap_or(env::current_dir()?);
    let root = fs::canonicalize(&root)
        .map_err(|error| error_with_path("workspace root 无法 canonicalize", &root, error))?;
    validate_workspace_root(&root)?;
    let target_root = verify_inherited_build_lock(&root)?;
    reject_build_environment()?;

    let release = target_root.join("release");
    validate_tree_if_present(&release, "Cargo release tree")?;
    ensure_directory(&release, 0o755)?;
    for path in [
        release.join(".fingerprint"),
        release.join("build"),
        release.join("deps"),
    ] {
        ensure_directory(&path, 0o755)?;
    }
    validate_tree(&release, "Cargo release tree")?;

    let temp_parent = env::var_os("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    validate_absolute_directory(&temp_parent, "package temp parent", true)?;
    let temp_parent = fs::canonicalize(&temp_parent)?;
    let temp = OwnedDirectory::create(&temp_parent, TEMP_PREFIX)?;

    let workspace = workspace_metadata(&root)?;
    let version = workspace
        .iter()
        .find(|package| package.name == CLI_PACKAGE)
        .and_then(|package| package.version.clone())
        .or_else(|| cargo_package_version(&root).ok())
        .ok_or_else(|| error("cargo metadata 缺少 kanban-cli version"))?;
    let binary = release.join(BINARY_NAME);
    let dep_info = release.join(format!("{BINARY_NAME}.d"));
    invalidate_workspace_artifacts(&release, &workspace)?;
    remove_owned_file_if_present(&binary, "旧 CLI binary")?;
    remove_owned_file_if_present(&dep_info, "旧 CLI dep-info")?;
    build_binary(&root, &options.build_args)?;
    validate_executable(&binary, "CLI binary")?;
    verify_dep_info(&root, &dep_info)?;

    let output_dir = release.join("bundle/cli/deb");
    ensure_directory(&release.join("bundle"), 0o755)?;
    ensure_directory(&release.join("bundle/cli"), 0o755)?;
    ensure_directory(&output_dir, 0o755)?;
    validate_tree(&release.join("bundle"), "CLI bundle tree")?;
    let output = build_deb(&root, &temp, &binary, &output_dir, &version)?;
    println!("{}", output.display());
    Ok(())
}

fn parse_options(arguments: &[String]) -> ToolResult<PackageOptions> {
    let mut options = PackageOptions {
        format: "deb".to_owned(),
        ..PackageOptions::default()
    };
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--root" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| error("--root 需要一个路径"))?;
                options.root = Some(PathBuf::from(value));
            }
            "--format" => {
                index += 1;
                options.format = arguments
                    .get(index)
                    .ok_or_else(|| error("--format 需要一个值"))?
                    .clone();
            }
            "--features" => {
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| error("--features 需要一个值"))?;
                options
                    .build_args
                    .extend(["--features".to_owned(), value.clone()]);
                index += 2;
                continue;
            }
            "--all-features" | "--no-default-features" => {
                options.build_args.push(arguments[index].clone());
            }
            "-h" | "--help" => {
                print_usage();
                return Err(error("help"));
            }
            unknown => return Err(error(format!("未知 package 参数: {unknown}"))),
        }
        index += 1;
    }
    Ok(options)
}

fn print_usage() {
    println!(
        "用法：xtask package cli [--format deb] [--features FEATURES] [--all-features] [--no-default-features] [--root PATH]"
    );
}

fn validate_workspace_root(root: &Path) -> ToolResult<()> {
    validate_absolute_directory(root, "workspace root", true)?;
    if !root.join("Cargo.toml").is_file() {
        return Err(error(format!(
            "workspace root 缺少 Cargo.toml: {}",
            root.display()
        )));
    }
    Ok(())
}

fn verify_inherited_build_lock(root: &Path) -> ToolResult<PathBuf> {
    if env::var("KANBAN_CARGO_BUILD_LOCK_HELD").ok().as_deref() != Some("1") {
        return Err(error(
            "CLI package 必须拥有 inherited Cargo build lock proof",
        ));
    }
    let raw_target = env::var_os("CARGO_TARGET_DIR")
        .ok_or_else(|| error("CLI package 必须使用 inherited CARGO_TARGET_DIR"))?;
    let raw_target = PathBuf::from(raw_target);
    validate_target_path(&raw_target, root)?;
    let target = lexical_normalize(&raw_target);

    let lock_path = env::var_os("KANBAN_CARGO_BUILD_LOCK_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| target.join(".build.lock"));
    let expected_lock = target.join(".build.lock");
    if lock_path != expected_lock {
        return Err(error(format!(
            "inherited Cargo build lock path 不匹配: {}",
            lock_path.display()
        )));
    }
    let lock = fs::symlink_metadata(&lock_path).map_err(|error| {
        error_with_path("inherited Cargo build lock 无法检查", &lock_path, error)
    })?;
    ensure_single_regular(&lock, &lock_path, "Cargo build lock")?;

    let fd = env::var("KANBAN_CARGO_BUILD_LOCK_FD")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|fd| *fd >= 3)
        .ok_or_else(|| error("inherited Cargo build lock fd 无效"))?;
    let proc_fd = PathBuf::from(format!("/proc/self/fd/{fd}"));
    let inherited = fs::metadata(&proc_fd).map_err(|error| {
        error_with_path("inherited Cargo build lock fd 无法检查", &proc_fd, error)
    })?;
    ensure_single_regular(&inherited, &proc_fd, "inherited Cargo build lock fd")?;
    if identity(&lock) != identity(&inherited) {
        return Err(error(
            "Cargo build lock path 与 inherited fd identity 不一致",
        ));
    }

    let lock_script = root.join("scripts/cargo-build-lock.sh");
    if !lock_script.is_file() || lock_script.is_symlink() {
        return Err(error(format!(
            "cargo-build-lock helper 缺失或不安全: {}",
            lock_script.display()
        )));
    }
    let output = Command::new(&lock_script)
        .arg("--verify-inherited-lock")
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(error(format!(
            "inherited Cargo build lock proof 无效: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    if !resource_environment_is_internal() {
        return Err(error(
            "inherited Cargo build lock 的 resource environment policy 无效",
        ));
    }
    Ok(target)
}

fn resource_environment_is_internal() -> bool {
    let build_policy = env::var("KANBAN_CARGO_BUILD_JOBS").unwrap_or_default();
    let test_policy = env::var("KANBAN_TEST_THREADS").unwrap_or_default();
    if !matches!(build_policy.as_str(), "" | "2" | "auto" | "AUTO")
        || !matches!(test_policy.as_str(), "" | "2" | "auto" | "AUTO")
    {
        return false;
    }

    let build_set = env::var_os("CARGO_BUILD_JOBS").is_some();
    let nextest_set = env::var_os("NEXTEST_TEST_THREADS").is_some();
    let rust_set = env::var_os("RUST_TEST_THREADS").is_some();
    if !build_set && !nextest_set && !rust_set {
        return build_policy != "2" && test_policy != "2";
    }
    build_set
        && nextest_set
        && rust_set
        && env::var("CARGO_BUILD_JOBS").ok().as_deref() == Some("2")
        && env::var("NEXTEST_TEST_THREADS").ok().as_deref() == Some("2")
        && env::var("RUST_TEST_THREADS").ok().as_deref() == Some("2")
        && build_policy != "auto"
        && build_policy != "AUTO"
        && test_policy != "auto"
        && test_policy != "AUTO"
}

fn validate_target_path(target: &Path, root: &Path) -> ToolResult<()> {
    validate_absolute_no_parent(target, "Cargo target root")?;
    if target == Path::new("/") {
        return Err(error("Cargo target root 不得是 filesystem root"));
    }
    let target = lexical_normalize(target);
    if target == Path::new("/") {
        return Err(error("Cargo target root 不得是 filesystem root"));
    }
    if target == *root || target.starts_with(root) {
        return Err(error(
            "CLI package 拒绝位于 source tree 内的 Cargo target root",
        ));
    }
    validate_existing_components(&target, "Cargo target root")?;
    Ok(())
}

fn reject_build_environment() -> ToolResult<()> {
    if env::var_os("CARGO_HOME").is_some() {
        return Err(error(
            "release package 拒绝 CARGO_HOME override；沿用默认 Cargo home",
        ));
    }
    let inherited = env::var("KANBAN_CARGO_BUILD_LOCK_HELD").ok().as_deref() == Some("1");
    for (name, _value) in env::vars_os() {
        let name = name.to_string_lossy();
        let reject = match name.as_ref() {
            "RUSTC_WRAPPER"
            | "RUSTC_WORKSPACE_WRAPPER"
            | "RUSTFLAGS"
            | "CARGO_ENCODED_RUSTFLAGS"
            | "RUSTDOCFLAGS"
            | "CARGO_ENCODED_RUSTDOCFLAGS"
            | "RUSTC_BOOTSTRAP"
            | "SOURCE_DATE_EPOCH"
            | "RUSTUP_TOOLCHAIN"
            | "RUSTUP_HOME"
            | "RUSTUP_DIST_SERVER"
            | "RUSTUP_UPDATE_ROOT"
            | "CARGO_BUILD_RUSTC"
            | "CARGO_BUILD_RUSTC_WRAPPER"
            | "CARGO_BUILD_RUSTDOC"
            | "CC"
            | "CXX"
            | "AR"
            | "CFLAGS"
            | "CXXFLAGS"
            | "CPPFLAGS"
            | "LDFLAGS"
            | "PKG_CONFIG_PATH"
            | "PKG_CONFIG_LIBDIR" => true,
            "CARGO_BUILD_JOBS" | "NEXTEST_TEST_THREADS" | "RUST_TEST_THREADS" => !inherited,
            "CARGO_TARGET_DIR" => false,
            value if value.starts_with("CARGO_TARGET_") => true,
            value
                if value.starts_with("CARGO_BUILD_")
                    || value.starts_with("CARGO_HTTP_")
                    || value.starts_with("CARGO_NET_")
                    || value.starts_with("CARGO_PROFILE_")
                    || value.starts_with("CARGO_REGISTRIES_")
                    || value.starts_with("CARGO_SOURCE_")
                    || value.starts_with("RUSTUP_")
                    || value.starts_with("CC_")
                    || value.starts_with("CXX_")
                    || value.starts_with("PKG_CONFIG_") =>
            {
                true
            }
            _ => false,
        };
        if reject {
            return Err(error(format!(
                "release package 拒绝会影响构建的 environment override: {name}"
            )));
        }
    }
    Ok(())
}

fn workspace_metadata(root: &Path) -> ToolResult<Vec<CargoPackage>> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(error(format!(
            "cargo metadata 失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let metadata: CargoMetadata = serde_json::from_slice(&output.stdout)
        .map_err(|parse_error| error(format!("cargo metadata 输出解析失败: {parse_error}")))?;
    if metadata.packages.is_empty() {
        return Err(error("cargo metadata workspace package 列表为空"));
    }
    Ok(metadata.packages)
}

fn cargo_package_version(root: &Path) -> ToolResult<String> {
    let output = Command::new("cargo")
        .args(["pkgid", "--locked", "-p", CLI_PACKAGE])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(error(format!(
            "cargo pkgid 失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let id = text.trim().rsplit('#').next().unwrap_or_default();
    let version = id.rsplit('@').next().unwrap_or(id).trim();
    if version.is_empty() {
        return Err(error("cargo pkgid 未返回 package version"));
    }
    Ok(version.to_owned())
}

fn invalidate_workspace_artifacts(release: &Path, packages: &[CargoPackage]) -> ToolResult<()> {
    let fingerprint = release.join(".fingerprint");
    let build = release.join("build");
    let deps = release.join("deps");
    validate_tree(&fingerprint, "Cargo fingerprint tree")?;
    validate_tree(&build, "Cargo build tree")?;
    validate_tree(&deps, "Cargo deps tree")?;

    let package_names = packages
        .iter()
        .map(|package| package.name.as_str())
        .collect::<Vec<_>>();
    let entries = fs::read_dir(&fingerprint)?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            error(format!(
                "fingerprint entry 不是 UTF-8: {}",
                entry.path().display()
            ))
        })?;
        let Some(package) = package_names
            .iter()
            .find(|package| name.strip_prefix(&format!("{package}-")).is_some())
        else {
            continue;
        };
        let suffix = name
            .strip_prefix(&format!("{package}-"))
            .unwrap_or_default();
        if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_dir() {
            return Err(error(format!(
                "workspace fingerprint entry 不是 directory: {}",
                path.display()
            )));
        }
        remove_owned_stale_directory(&path, "workspace fingerprint")?;

        let build_path = build.join(name);
        if path_exists(&build_path)? {
            let metadata = fs::symlink_metadata(&build_path)?;
            if !metadata.is_dir() {
                return Err(error(format!(
                    "workspace build entry 不是 directory: {}",
                    build_path.display()
                )));
            }
            remove_owned_stale_directory(&build_path, "workspace build")?;
        }
        let crate_name = package.replace('-', "_");
        for dep in fs::read_dir(&deps)? {
            let dep = dep?;
            let dep_name = dep.file_name();
            let dep_name = dep_name.to_string_lossy();
            if !dependency_artifact_matches(&dep_name, &crate_name, suffix) {
                continue;
            }
            let dep_path = dep.path();
            let metadata = fs::symlink_metadata(&dep_path)?;
            ensure_single_regular(&metadata, &dep_path, "workspace deps stale entry")?;
            fs::remove_file(&dep_path).map_err(|error| {
                error_with_path("删除 workspace deps stale entry 失败", &dep_path, error)
            })?;
        }
    }
    Ok(())
}

fn dependency_artifact_matches(name: &str, crate_name: &str, hash: &str) -> bool {
    let marker = format!("-{hash}");
    [crate_name, &format!("lib{crate_name}")]
        .iter()
        .any(|prefix| {
            name.strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with(&marker))
        })
}

fn remove_owned_stale_directory(path: &Path, label: &str) -> ToolResult<()> {
    validate_tree(path, label)?;
    fs::remove_dir_all(path)
        .map_err(|error| error_with_path("删除 stale directory 失败", path, error))?;
    Ok(())
}

fn remove_owned_file_if_present(path: &Path, label: &str) -> ToolResult<()> {
    if !path_exists(path)? {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path)?;
    ensure_single_regular(&metadata, path, label)?;
    fs::remove_file(path).map_err(|error| error_with_path("删除旧 artifact 失败", path, error))?;
    Ok(())
}

fn build_binary(root: &Path, build_args: &[String]) -> ToolResult<()> {
    let mut command = Command::new("cargo");
    command
        .args(["build", "--locked", "-p", CLI_PACKAGE, "--release"])
        .args(build_args)
        .current_dir(root);
    let status = command.status()?;
    if !status.success() {
        return Err(error(format!("cargo build 失败: {status}")));
    }
    Ok(())
}

fn verify_dep_info(root: &Path, dep_info: &Path) -> ToolResult<()> {
    let metadata = fs::symlink_metadata(dep_info)
        .map_err(|error| error_with_path("dep-info 不存在或不可检查", dep_info, error))?;
    ensure_single_regular(&metadata, dep_info, "CLI dep-info")?;
    let mut content = Vec::new();
    File::open(dep_info)?.read_to_end(&mut content)?;
    let prerequisites = parse_dep_info_prerequisites(&content)?;
    let crates_root = fs::canonicalize(root.join("crates")).map_err(|error| {
        error_with_path(
            "canonical crates root 无法检查",
            &root.join("crates"),
            error,
        )
    })?;
    let mut found = false;
    for prerequisite in prerequisites {
        let raw = PathBuf::from(&prerequisite);
        let path = if raw.is_absolute() {
            raw
        } else {
            root.join(raw)
        };
        let canonical = fs::canonicalize(&path).map_err(|error| {
            error_with_path(
                "dep-info prerequisite 不存在或无法 canonicalize",
                &path,
                error,
            )
        })?;
        if canonical.starts_with(&crates_root) {
            found = true;
        }
    }
    if !found {
        return Err(error("dep-info 不包含当前 canonical crates/ prerequisite"));
    }
    Ok(())
}

/// 解析 Cargo makefile dep-info。支持 continuation、反斜杠转义、`$$` 和
/// `#` 注释，而不是使用易误报的文本 grep。
fn parse_dep_info_prerequisites(content: &[u8]) -> ToolResult<Vec<String>> {
    let mut prerequisites = Vec::new();
    let mut word = Vec::new();
    let mut in_prerequisites = false;
    let mut index = 0;
    let finish = |word: &mut Vec<u8>, in_prerequisites: bool, output: &mut Vec<String>| {
        if in_prerequisites && !word.is_empty() {
            output.push(String::from_utf8_lossy(word).into_owned());
        }
        word.clear();
    };
    while index < content.len() {
        match content[index] {
            b'\\' => {
                if index + 1 >= content.len() {
                    word.push(b'\\');
                    index += 1;
                } else if content[index + 1] == b'\n' {
                    index += 2;
                } else if content[index + 1] == b'\r'
                    && index + 2 < content.len()
                    && content[index + 2] == b'\n'
                {
                    index += 3;
                } else {
                    word.push(content[index + 1]);
                    index += 2;
                }
            }
            b'$' if index + 1 < content.len() && content[index + 1] == b'$' => {
                word.push(b'$');
                index += 2;
            }
            b':' if !in_prerequisites => {
                word.clear();
                in_prerequisites = true;
                index += 1;
            }
            b' ' | b'\t' | b'\r' | b'\n' => {
                finish(&mut word, in_prerequisites, &mut prerequisites);
                if content[index] == b'\n' {
                    in_prerequisites = false;
                }
                index += 1;
            }
            b'#' => {
                finish(&mut word, in_prerequisites, &mut prerequisites);
                in_prerequisites = false;
                while index < content.len() && content[index] != b'\n' {
                    index += 1;
                }
            }
            byte => {
                word.push(byte);
                index += 1;
            }
        }
    }
    finish(&mut word, in_prerequisites, &mut prerequisites);
    Ok(prerequisites)
}

fn build_deb(
    root: &Path,
    temp: &OwnedDirectory,
    binary: &Path,
    output_dir: &Path,
    version: &str,
) -> ToolResult<PathBuf> {
    if command_missing("dpkg-deb") {
        return Err(error("--format deb 需要 dpkg-deb"));
    }
    let architecture = deb_architecture()?
        .ok_or_else(|| error("target triple 的 Debian architecture 不受支持"))?;
    temp.verify()?;
    let package_root = temp.path.join("deb-root");
    ensure_directory(&package_root.join("DEBIAN"), 0o755)?;
    install_payload(root, binary, &package_root)?;
    let installed_size = installed_size(&package_root.join("usr"))?;
    let depends = deb_dependencies(&package_root, temp)?;

    let control = format!(
        "Package: {PACKAGE_NAME}\nVersion: {version}-{REVISION}\nSection: utils\nPriority: optional\nArchitecture: {architecture}\nDepends: {depends}\nMaintainer: SockingPanda <42059910+SockingPanda@users.noreply.github.com>\nInstalled-Size: {installed_size}\nDescription: Local-first Kanban CLI\n Standalone kanban command line client for the Kanban Tool local work queue.\n"
    );
    let control_path = package_root.join("DEBIAN/control");
    write_regular_file(&control_path, control.as_bytes(), 0o644)?;

    let output_name = format!("{PACKAGE_NAME}_{version}-{REVISION}_{architecture}.deb");
    let output = output_dir.join(output_name);
    let output_parent = regular_directory_metadata(output_dir, "CLI package output directory")?;
    let stage = OwnedDirectory::create(output_dir, STAGE_PREFIX)?;
    let staged = stage.path.join(
        output
            .file_name()
            .ok_or_else(|| error("CLI package output name 无效"))?,
    );
    if identity(&output_parent)
        != identity(&regular_directory_metadata(
            output_dir,
            "CLI package output directory",
        )?)
    {
        return Err(error("CLI package output directory identity 发生漂移"));
    }
    let status = Command::new("dpkg-deb")
        .args(["--root-owner-group", "--build"])
        .arg(&package_root)
        .arg(&staged)
        .status()?;
    if !status.success() {
        return Err(error(format!("dpkg-deb 构建失败: {status}")));
    }
    let staged_metadata = fs::symlink_metadata(&staged)?;
    ensure_single_regular(&staged_metadata, &staged, "staged CLI package")?;
    publish_staged(&stage, &staged, output_dir, &output)?;
    // stage 目录只含刚刚发布的文件，且是本进程创建；显式校验后清理，避免
    // 出错路径误删同名的其他目录。
    stage.cleanup()?;
    let final_metadata = fs::symlink_metadata(&output)?;
    ensure_single_regular(&final_metadata, &output, "published CLI package")?;
    Ok(output)
}

fn publish_staged(
    stage: &OwnedDirectory,
    staged: &Path,
    output_dir: &Path,
    output: &Path,
) -> ToolResult<()> {
    if staged.parent() != Some(stage.path.as_path()) {
        return Err(error(format!(
            "staged package 不在 owned stage 根下: {}",
            staged.display()
        )));
    }
    if output.parent() != Some(output_dir) {
        return Err(error(format!(
            "package destination 不在 output directory 下: {}",
            output.display()
        )));
    }
    stage.verify()?;
    if metadata_dev(&stage.path)? != metadata_dev(output_dir)? {
        return Err(error("CLI package staging 与 output 不在同一 filesystem"));
    }
    if path_exists(output)? {
        let metadata = fs::symlink_metadata(output)?;
        ensure_single_regular(&metadata, output, "CLI package destination")?;
    }
    let staged_metadata = fs::symlink_metadata(staged)?;
    ensure_single_regular(&staged_metadata, staged, "staged CLI package")?;
    if path_exists(output)? {
        let metadata = fs::symlink_metadata(output)?;
        ensure_single_regular(&metadata, output, "CLI package destination")?;
    }
    fs::rename(staged, output)
        .map_err(|error| error_with_path("发布 CLI package 失败", output, error))?;
    Ok(())
}

fn install_payload(root: &Path, binary: &Path, package_root: &Path) -> ToolResult<()> {
    let bin = package_root.join("usr/bin").join(BINARY_NAME);
    let readme = package_root
        .join("usr/share/doc")
        .join(PACKAGE_NAME)
        .join("README.md");
    ensure_directory(bin.parent().expect("binary parent exists"), 0o755)?;
    ensure_directory(readme.parent().expect("README parent exists"), 0o755)?;
    copy_regular(binary, &bin, 0o755)?;
    copy_regular(&root.join("README.md"), &readme, 0o644)
}

fn deb_dependencies(package_root: &Path, temp: &OwnedDirectory) -> ToolResult<String> {
    if command_missing("dpkg-shlibdeps") {
        eprintln!("warning: 未找到 dpkg-shlibdeps；使用保守的 Debian runtime dependencies");
        return Ok("libc6, libgcc-s1".to_owned());
    }
    let workspace = temp.path.join("shlibdeps");
    let control = workspace.join("debian/control");
    ensure_directory(control.parent().expect("control parent exists"), 0o755)?;
    write_regular_file(
        &control,
        format!("Source: {PACKAGE_NAME}\nPackage: {PACKAGE_NAME}\nArchitecture: any\n").as_bytes(),
        0o644,
    )?;
    let binary = package_root.join("usr/bin").join(BINARY_NAME);
    let output = Command::new("dpkg-shlibdeps")
        .arg("-O")
        .arg(format!("-S{}", package_root.display()))
        .arg(&binary)
        .current_dir(&workspace)
        .output()?;
    if !output.status.success() {
        return Err(error(format!(
            "dpkg-shlibdeps 生成 shared-library dependencies 失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("shlibs:Depends="))
        .filter(|depends| !depends.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| error("dpkg-shlibdeps 未返回 shlibs:Depends 值"))
}

fn deb_architecture() -> ToolResult<Option<&'static str>> {
    let output = Command::new("rustc").arg("-vV").output()?;
    if !output.status.success() {
        return Err(error("rustc -vV 失败"));
    }
    let host_output = String::from_utf8_lossy(&output.stdout);
    let host = host_output
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap_or_default();
    let arch = host.split('-').next().unwrap_or_default();
    Ok(match arch {
        "x86_64" => Some("amd64"),
        "aarch64" => Some("arm64"),
        value if value.starts_with("armv7") || value == "arm" => Some("armhf"),
        "i686" | "i586" => Some("i386"),
        _ => None,
    })
}

fn installed_size(root: &Path) -> ToolResult<u64> {
    let bytes = installed_bytes(root)?;
    Ok(bytes.saturating_add(1023) / 1024)
}

fn installed_bytes(root: &Path) -> ToolResult<u64> {
    let mut total = 0u64;
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir() {
        return Err(error(format!(
            "package usr 不是 directory: {}",
            root.display()
        )));
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            total = total.saturating_add(installed_bytes(&path)?);
        } else {
            ensure_single_regular(&metadata, &path, "package payload")?;
            total = total.saturating_add(metadata.len());
        }
    }
    Ok(total)
}

fn copy_regular(source: &Path, destination: &Path, mode: u32) -> ToolResult<()> {
    let source_metadata = fs::symlink_metadata(source)?;
    ensure_single_regular(&source_metadata, source, "package source file")?;
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new();
    output.write(true).create_new(true);
    #[cfg(unix)]
    output.mode(0o600);
    let mut output = output.open(destination)?;
    io::copy(&mut input, &mut output)?;
    output.flush()?;
    set_mode(destination, mode)?;
    let current = fs::symlink_metadata(source)?;
    if identity(&source_metadata) != identity(&current) || source_metadata.len() != current.len() {
        return Err(error(format!(
            "package source 在复制期间发生变化: {}",
            source.display()
        )));
    }
    Ok(())
}

fn write_regular_file(path: &Path, bytes: &[u8], mode: u32) -> ToolResult<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    set_mode(path, mode)?;
    Ok(())
}

fn ensure_directory(path: &Path, mode: u32) -> ToolResult<()> {
    validate_absolute_no_parent(path, "directory")?;
    validate_existing_components(path, "directory")?;
    if path_exists(path)? {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_dir() {
            return Err(error(format!(
                "directory 不是 no-follow directory: {}",
                path.display()
            )));
        }
    } else {
        fs::create_dir_all(path)?;
    }
    validate_existing_components(path, "directory")?;
    set_mode(path, mode)?;
    Ok(())
}

fn validate_tree_if_present(path: &Path, label: &str) -> ToolResult<()> {
    if path_exists(path)? {
        validate_tree(path, label)?;
    }
    Ok(())
}

fn validate_tree(path: &Path, label: &str) -> ToolResult<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| error_with_path("无法检查 tree", path, error))?;
    if metadata.is_symlink() {
        return Err(error(format!("{label} 包含 symlink: {}", path.display())));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            validate_tree(&entry?.path(), label)?;
        }
        return Ok(());
    }
    ensure_single_regular(&metadata, path, label)
}

fn validate_absolute_directory(path: &Path, label: &str, require_exists: bool) -> ToolResult<()> {
    validate_absolute_no_parent(path, label)?;
    validate_existing_components(path, label)?;
    if require_exists {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| error_with_path("目录不存在", path, error))?;
        if !metadata.is_dir() {
            return Err(error(format!("{label} 不是 directory: {}", path.display())));
        }
    }
    Ok(())
}

fn validate_absolute_no_parent(path: &Path, label: &str) -> ToolResult<()> {
    if !path.is_absolute() {
        return Err(error(format!(
            "{label} 必须是 absolute path: {}",
            path.display()
        )));
    }
    let raw = path.as_os_str().to_string_lossy();
    if raw.starts_with("//") {
        return Err(error(format!("{label} 必须只包含一个前导 slash: {raw}")));
    }
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(error(format!(
            "{label} 不得包含 parent traversal: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_existing_components(path: &Path, label: &str) -> ToolResult<()> {
    let mut current = PathBuf::from("/");
    for component in path.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.is_symlink() {
                    return Err(error(format!(
                        "{label} 包含 symlink component: {}",
                        current.display()
                    )));
                }
                if !metadata.is_dir() {
                    return Err(error(format!(
                        "{label} 包含 non-directory component: {}",
                        current.display()
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(error_with_path(
                    "无法安全检查 path component",
                    &current,
                    error,
                ));
            }
        }
    }
    Ok(())
}

fn regular_directory_metadata(path: &Path, label: &str) -> ToolResult<Metadata> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| error_with_path("directory 无法检查", path, error))?;
    if metadata.is_symlink() || !metadata.is_dir() {
        return Err(error(format!(
            "{label} 不是 no-follow directory: {}",
            path.display()
        )));
    }
    Ok(metadata)
}

fn ensure_single_regular(metadata: &Metadata, path: &Path, label: &str) -> ToolResult<()> {
    if metadata.is_symlink() || !metadata.is_file() {
        return Err(error(format!(
            "{label} 不是 no-follow regular file: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(error(format!(
            "{label} 必须是 single-linked regular file: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_executable(path: &Path, label: &str) -> ToolResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure_single_regular(&metadata, path, label)?;
    #[cfg(unix)]
    if metadata.mode() & 0o111 == 0 {
        return Err(error(format!("{label} 不可执行: {}", path.display())));
    }
    Ok(())
}

fn path_exists(path: &Path) -> ToolResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error_with_path("无法检查 path", path, error)),
    }
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir => output.push("/"),
            Component::Normal(value) => output.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) => {}
        }
    }
    output
}

fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        let mut permissions = fs::symlink_metadata(path)?.permissions();
        permissions.set_mode(mode);
        fs::set_permissions(path, permissions)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

fn identity(metadata: &Metadata) -> Identity {
    #[cfg(unix)]
    {
        Identity {
            dev: metadata.dev(),
            ino: metadata.ino(),
        }
    }
    #[cfg(not(unix))]
    {
        Identity { dev: 0, ino: 0 }
    }
}

fn metadata_dev(path: &Path) -> ToolResult<u64> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        Ok(metadata.dev())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Ok(0)
    }
}

fn unique_nonce(attempt: u32) -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos() as u64);
    nanos.rotate_left(17) ^ (std::process::id() as u64).rotate_left(31) ^ attempt as u64
}

fn command_missing(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_or(true, |status| !status.success())
}

fn error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    io::Error::other(message.into()).into()
}

fn error_with_path(
    message: &str,
    path: &Path,
    error: io::Error,
) -> Box<dyn std::error::Error + Send + Sync> {
    io::Error::other(format!("{message}: {}: {error}", path.display())).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(name: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "xtask-package-{name}-{}-{}",
            std::process::id(),
            unique_nonce(0)
        ));
        fs::create_dir_all(&path).expect("fixture root should be creatable");
        path
    }

    #[test]
    fn dep_info_parser_handles_escapes_continuations_dollars_and_comments() {
        let input = br#"target: /tmp/source\ with\ space/crates/a/src/lib.rs \
 /tmp/source/crates/b/src/$$generated.rs # ignored\n"#;
        let parsed = parse_dep_info_prerequisites(input).expect("dep-info should parse");
        assert_eq!(
            parsed,
            vec![
                "/tmp/source with space/crates/a/src/lib.rs",
                "/tmp/source/crates/b/src/$generated.rs"
            ]
        );
    }

    #[test]
    fn package_options_forward_feature_values_without_skipping_following_flags() {
        let options = parse_options(&[
            "--features".to_owned(),
            "schema,sqlite".to_owned(),
            "--all-features".to_owned(),
            "--no-default-features".to_owned(),
        ])
        .expect("package options should parse");
        assert_eq!(
            options.build_args,
            [
                "--features",
                "schema,sqlite",
                "--all-features",
                "--no-default-features"
            ]
        );
    }

    #[test]
    fn dep_info_provenance_accepts_other_current_root_prerequisites_when_crates_is_present() {
        let root = fixture("dep-info-root");
        fs::create_dir_all(root.join("crates/example/src")).expect("crates fixture");
        fs::write(root.join("crates/example/src/lib.rs"), b"fn marker() {}")
            .expect("crate source fixture");
        fs::write(root.join("README.md"), b"readme").expect("root prerequisite fixture");
        let dep_info = root.join("kanban.d");
        fs::write(
            &dep_info,
            format!(
                "{}: {} {}\n",
                root.join("target/kanban").display(),
                root.join("README.md").display(),
                root.join("crates/example/src/lib.rs").display()
            ),
        )
        .expect("dep-info fixture");
        verify_dep_info(&root, &dep_info).expect("current crates prerequisite should suffice");
        fs::remove_dir_all(root).expect("fixture should be cleaned");
    }

    #[test]
    fn target_root_rejects_parent_traversal_and_source_tree() {
        let root = fixture("target-root");
        assert!(validate_target_path(&PathBuf::from("relative"), &root).is_err());
        assert!(validate_target_path(&root.join("nested/../outside"), &root).is_err());
        assert!(validate_target_path(&root.join("target"), &root).is_err());
        fs::remove_dir_all(root).expect("fixture should be cleaned");
    }

    #[cfg(unix)]
    #[test]
    fn existing_tree_rejects_symlink_hardlink_and_nonregular_entries() {
        let root = fixture("tree-safety");
        let outside = fixture("tree-safety-outside");
        fs::write(root.join("regular"), b"x").expect("regular fixture");
        fs::hard_link(root.join("regular"), root.join("hardlink")).expect("hardlink fixture");
        assert!(validate_tree(&root, "tree").is_err());
        fs::remove_file(root.join("hardlink")).expect("remove hardlink");
        let directory = root.join("directory-entry");
        fs::create_dir(&directory).expect("directory fixture");
        let directory_metadata = fs::symlink_metadata(&directory).expect("directory metadata");
        assert!(ensure_single_regular(&directory_metadata, &directory, "file").is_err());
        std::os::unix::fs::symlink(&outside, root.join("linked")).expect("symlink fixture");
        assert!(validate_tree(&root, "tree").is_err());
        fs::remove_dir_all(root).expect("fixture should be cleaned");
        fs::remove_dir_all(outside).expect("fixture should be cleaned");
    }

    #[test]
    fn owned_temp_cleanup_requires_prefix_and_parent_identity() {
        let parent = fixture("temp-cleanup");
        let owned = OwnedDirectory::create(&parent, TEMP_PREFIX).expect("owned directory");
        let path = owned.path.clone();
        owned.cleanup().expect("owned directory should clean");
        assert!(!path.exists());
        fs::remove_dir_all(parent).expect("fixture should be cleaned");
    }

    #[cfg(unix)]
    #[test]
    fn publish_replaces_regular_destination_but_rejects_unsafe_entries() {
        let root = fixture("publish");
        let stage = OwnedDirectory::create(&root, STAGE_PREFIX).expect("stage");
        let source = stage.path.join("out.deb");
        fs::write(&source, b"new").expect("source");
        let destination = root.join("out.deb");
        fs::write(&destination, b"old").expect("destination");
        publish_staged(&stage, &source, &root, &destination)
            .expect("single-linked regular destination should be replaced");
        assert_eq!(
            fs::read(&destination).expect("destination should be replaced"),
            b"new"
        );

        let symlink_target = root.join("symlink-target");
        fs::write(&symlink_target, b"untouched").expect("symlink target");
        let symlink = root.join("symlink.deb");
        std::os::unix::fs::symlink(&symlink_target, &symlink).expect("symlink destination");
        let symlink_stage = OwnedDirectory::create(&root, STAGE_PREFIX).expect("symlink stage");
        let symlink_source = symlink_stage.path.join("symlink.deb");
        fs::write(&symlink_source, b"new").expect("symlink source");
        assert!(publish_staged(&symlink_stage, &symlink_source, &root, &symlink).is_err());
        assert_eq!(
            fs::read(&symlink_target).expect("symlink target should remain"),
            b"untouched"
        );

        let hardlink_source = root.join("hardlink-source");
        fs::write(&hardlink_source, b"hardlink").expect("hardlink source");
        let hardlink = root.join("hardlink.deb");
        fs::hard_link(&hardlink_source, &hardlink).expect("hardlink destination");
        let hardlink_stage = OwnedDirectory::create(&root, STAGE_PREFIX).expect("hardlink stage");
        let hardlink_staged = hardlink_stage.path.join("hardlink.deb");
        fs::write(&hardlink_staged, b"new").expect("hardlink staged");
        assert!(publish_staged(&hardlink_stage, &hardlink_staged, &root, &hardlink).is_err());
        assert_eq!(
            fs::read(&hardlink).expect("hardlink should remain"),
            b"hardlink"
        );

        drop(stage);
        drop(symlink_stage);
        drop(hardlink_stage);
        fs::remove_dir_all(root).expect("fixture should be cleaned");
    }

    #[test]
    fn dependency_invalidation_matches_only_current_package_name_and_hash() {
        assert!(dependency_artifact_matches(
            "libworkspace_crate-aaa.rlib",
            "workspace_crate",
            "aaa"
        ));
        assert!(dependency_artifact_matches(
            "workspace_crate-aaa.d",
            "workspace_crate",
            "aaa"
        ));
        assert!(!dependency_artifact_matches(
            "libregistry_crate-aaa.rlib",
            "workspace_crate",
            "aaa"
        ));
        assert!(!dependency_artifact_matches(
            "libworkspace_crate-bbb.rlib",
            "workspace_crate",
            "aaa"
        ));
    }
}
