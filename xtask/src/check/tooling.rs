use std::{ffi::OsStr, fs, path::Path};

use xtask::ToolResult;

/// 这些目录不是源代码树：它们来自版本控制、构建、前端依赖或生成步骤。
const SKIPPED_DIRECTORY_NAMES: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".pnpm",
    "__pycache__",
    "build",
    "coverage",
    "dist",
    "generated",
    "out",
];

/// 仓库中不应递归检查的、但名称不足以表达用途的生成目录。
const SKIPPED_DIRECTORY_PATHS: &[&str] = &[
    "schemas/json-schema",
    "apps/desktop/src-tauri/gen",
    "apps/desktop/src-tauri/binaries",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DiagnosticKind {
    Source,
    Shebang,
    Command,
    Path,
}

impl DiagnosticKind {
    fn name(self) -> &'static str {
        match self {
            Self::Source => "PythonSource",
            Self::Shebang => "PythonShebang",
            Self::Command => "PythonCommand",
            Self::Path => "PythonPath",
        }
    }
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Diagnostic {
    path: String,
    line: Option<usize>,
    kind: DiagnosticKind,
}

impl Diagnostic {
    fn source(root: &Path, path: &Path) -> Self {
        Self {
            path: relative_path(root, path),
            line: None,
            kind: DiagnosticKind::Source,
        }
    }

    fn active(root: &Path, path: &Path, line: usize, kind: DiagnosticKind) -> Self {
        Self {
            path: relative_path(root, path),
            line: Some(line),
            kind,
        }
    }

    fn render(&self) -> String {
        match self.line {
            Some(line) => format!(
                "{}: {}:{} 检测到 Python 入口",
                self.kind.name(),
                self.path,
                line
            ),
            None => format!("{}: {} 检测到 Python 源文件", self.kind.name(), self.path),
        }
    }
}

pub(crate) fn run(root: &Path) -> ToolResult<()> {
    let mut diagnostics = Vec::new();
    scan_path(root, root, &mut diagnostics)?;
    diagnostics.sort_unstable();

    if diagnostics.is_empty() {
        println!("ok: tooling check 已通过");
        return Ok(());
    }

    let details = diagnostics
        .iter()
        .map(Diagnostic::render)
        .collect::<Vec<_>>()
        .join("\n");
    Err(std::io::Error::other(format!("tooling check 失败：\n{details}")).into())
}

fn scan_path(root: &Path, path: &Path, diagnostics: &mut Vec<Diagnostic>) -> ToolResult<()> {
    let file_type = fs::symlink_metadata(path)?.file_type();
    let is_symlink = file_type.is_symlink();
    let is_python_name = has_python_extension(path);

    // symlink_metadata 不会跟随链接；只对链接做一次 metadata 判断类型，绝不递归进入
    // 链接目录。链接文件仍按它在仓库中的名字参与源文件和 active command 检查。
    if is_symlink {
        if is_python_name {
            diagnostics.push(Diagnostic::source(root, path));
        }
        let target = fs::metadata(path).ok();
        if target.as_ref().is_none_or(|metadata| metadata.is_dir()) {
            return Ok(());
        }
        if target.is_some_and(|metadata| metadata.is_file()) {
            scan_active_file(root, path, diagnostics)?;
        }
        return Ok(());
    }

    if file_type.is_file() {
        if is_python_name {
            diagnostics.push(Diagnostic::source(root, path));
        }
        scan_active_file(root, path, diagnostics)?;
        return Ok(());
    }

    if !file_type.is_dir() {
        return Ok(());
    }
    if path != root && should_skip_directory(root, path) {
        return Ok(());
    }

    let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        scan_path(root, &entry.path(), diagnostics)?;
    }
    Ok(())
}

fn scan_active_file(root: &Path, path: &Path, diagnostics: &mut Vec<Diagnostic>) -> ToolResult<()> {
    if !is_active_file(root, path) {
        return Ok(());
    }
    let bytes = fs::read(path)?;
    // 二进制或非 UTF-8 文件不是可执行文本；即使其中偶然出现 python 字节，也不应误报。
    if bytes.contains(&0) {
        return Ok(());
    }
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Ok(());
    };

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        if line.trim_start().starts_with("#![") {
            // Rust inner attribute 不是 shebang；active fixture 也不应把它当作 Python。
            continue;
        }
        if is_python_shebang(line) {
            diagnostics.push(Diagnostic::active(
                root,
                path,
                line_number,
                DiagnosticKind::Shebang,
            ));
            continue;
        }

        let command_line = strip_comment(line);
        if contains_python_command(command_line) {
            diagnostics.push(Diagnostic::active(
                root,
                path,
                line_number,
                DiagnosticKind::Command,
            ));
        }
        if contains_python_path(command_line) {
            diagnostics.push(Diagnostic::active(
                root,
                path,
                line_number,
                DiagnosticKind::Path,
            ));
        }
    }
    Ok(())
}

fn is_active_file(root: &Path, path: &Path) -> bool {
    let Some(relative) = path.strip_prefix(root).ok() else {
        return false;
    };
    if relative == Path::new("justfile") {
        return true;
    }

    let components = relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>();
    if components.len() >= 3
        && components[0] == ".github"
        && components[1] == "workflows"
        && is_workflow_extension(path)
    {
        return true;
    }
    if components.len() >= 2 && components[0] == "scripts" && has_extension(path, "sh", true) {
        return true;
    }
    false
}

fn is_workflow_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
        })
}

fn has_extension(path: &Path, expected: &str, case_insensitive: bool) -> bool {
    let Some(extension) = path.extension().and_then(OsStr::to_str) else {
        return false;
    };
    if case_insensitive {
        extension.eq_ignore_ascii_case(expected)
    } else {
        extension == expected
    }
}

fn should_skip_directory(root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    if relative.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| SKIPPED_DIRECTORY_NAMES.contains(&name))
    }) {
        return true;
    }
    SKIPPED_DIRECTORY_PATHS
        .iter()
        .any(|skipped| relative == Path::new(skipped) || relative.starts_with(Path::new(skipped)))
}

fn has_python_extension(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".py"))
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_python_shebang(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(body) = trimmed.strip_prefix("#!") else {
        return false;
    };
    let body = body.trim_start();
    if body.starts_with('[') {
        return false;
    }
    let mut tokens = body.split_whitespace();
    let Some(interpreter) = tokens.next() else {
        return false;
    };
    if command_basename(interpreter) != Some("env") {
        return is_python_interpreter(interpreter);
    }

    for token in tokens {
        if token == "--" || token == "-S" || token.starts_with('-') || token.contains('=') {
            continue;
        }
        return is_python_interpreter(token);
    }
    false
}

fn command_basename(token: &str) -> Option<&str> {
    let token = token.trim_matches(['\'', '"']);
    token.rsplit('/').next()
}

fn is_python_interpreter(token: &str) -> bool {
    let Some(name) = command_basename(token) else {
        return false;
    };
    let name = name.trim_matches(['\'', '"']);
    name == "python"
        || name
            .strip_prefix("python3")
            .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with('.'))
}

fn contains_python_command(line: &str) -> bool {
    contains_boundary_word(line, "python") || contains_boundary_word(line, "python3")
}

fn contains_boundary_word(line: &str, needle: &str) -> bool {
    let bytes = line.as_bytes();
    let needle = needle.as_bytes();
    bytes
        .windows(needle.len())
        .enumerate()
        .any(|(index, window)| {
            window == needle
                && (index == 0 || !is_ascii_word(bytes[index - 1]))
                && (index + needle.len() == bytes.len()
                    || !is_ascii_word(bytes[index + needle.len()]))
        })
}

fn contains_python_path(line: &str) -> bool {
    let line = line.to_ascii_lowercase();
    let bytes = line.as_bytes();
    bytes.windows(3).enumerate().any(|(index, window)| {
        window == b".py"
            && (index + 3 == bytes.len()
                || (!is_ascii_word(bytes[index + 3]) && bytes[index + 3] != b'.'))
    })
}

fn is_ascii_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn strip_comment(line: &str) -> &str {
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    for (index, byte) in line.bytes().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        match byte {
            b'\\' if quote != Some(b'\'') => escaped = true,
            b'\'' | b'"' => {
                if quote == Some(byte) {
                    quote = None;
                } else if quote.is_none() {
                    quote = Some(byte);
                }
            }
            b'#' if quote.is_none() => return &line[..index],
            _ => {}
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "xtask-tooling-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temporary root should be creatable");
        path
    }

    fn write(root: &Path, relative: &str, content: impl AsRef<[u8]>) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("fixture must have a parent"))
            .expect("fixture parent should be creatable");
        fs::write(path, content).expect("fixture should be writable");
    }

    #[test]
    fn clean_fixture_passes_and_does_not_scan_docs_or_rust() {
        let root = temp_root("clean");
        write(
            &root,
            "justfile",
            "check:\n    cargo check\n# python3 and scripts/example.py are prose\n",
        );
        write(
            &root,
            ".github/workflows/clean.yaml",
            "jobs:\n  check:\n    runs-on: ubuntu\n    steps:\n      - run: cargo check\n",
        );
        write(
            &root,
            "scripts/clean.sh",
            "#!/usr/bin/env bash\necho clean\n",
        );
        write(&root, "docs/guide.md", "python3 scripts/example.py\n");
        write(
            &root,
            "src/lib.rs",
            "#![allow(dead_code)]\nconst PY: &str = \"python3\";\n",
        );

        assert!(run(&root).is_ok());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn source_fixture_reports_regular_and_symlink_python_files() {
        let root = temp_root("source");
        write(&root, "scripts/tool.PY", "print('source')\n");

        #[cfg(unix)]
        {
            write(&root, "outside.txt", "not Python source\n");
            std::os::unix::fs::symlink(root.join("outside.txt"), root.join("linked.Py"))
                .expect("source symlink should be creatable");
        }

        let error = run(&root).expect_err("Python source should be rejected");
        let message = error.to_string();
        assert!(
            message.contains("PythonSource: scripts/tool.PY"),
            "{message}"
        );
        #[cfg(unix)]
        assert!(message.contains("PythonSource: linked.Py"), "{message}");
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn active_fixture_reports_shebang_command_and_path_with_lines() {
        let root = temp_root("active");
        write(
            &root,
            "justfile",
            "run:\n    /usr/bin/python3 -B scripts/check.py\n",
        );
        write(
            &root,
            ".github/workflows/python.yaml",
            "jobs:\n  check:\n    steps:\n      - run: env -S python3 scripts/work.py\n",
        );
        write(
            &root,
            "scripts/python.sh",
            "#!/usr/bin/env python3\npython scripts/run.PY\n",
        );

        let error = run(&root).expect_err("active Python references should be rejected");
        let message = error.to_string();
        assert!(
            message.contains("PythonShebang: scripts/python.sh:1"),
            "{message}"
        );
        assert!(message.contains("PythonCommand: justfile:2"), "{message}");
        assert!(
            message.contains("PythonCommand: .github/workflows/python.yaml:4"),
            "{message}"
        );
        assert!(message.contains("PythonPath: justfile:2"), "{message}");
        assert!(
            message.contains("PythonPath: scripts/python.sh:2"),
            "{message}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn skipped_directories_and_directory_symlink_are_not_followed() {
        let root = temp_root("skipped");
        for directory in [
            ".git",
            "target",
            "node_modules/pkg",
            ".pnpm/store",
            "generated",
            "schemas/json-schema",
        ] {
            write(
                &root,
                &format!("{directory}/hidden.py"),
                "print('ignored')\n",
            );
        }

        #[cfg(unix)]
        {
            let outside = temp_root("skipped-outside");
            write(&outside, "hidden.py", "print('outside')\n");
            std::os::unix::fs::symlink(&outside, root.join("linked-directory"))
                .expect("directory symlink should be creatable");
            assert!(run(&root).is_ok());
            fs::remove_dir_all(outside).expect("outside fixture should be removable");
        }
        #[cfg(not(unix))]
        assert!(run(&root).is_ok());

        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn binary_active_fixture_is_ignored() {
        let root = temp_root("binary");
        write(&root, "scripts/binary.sh", b"\0python3 scripts/binary.py\n");
        write(&root, "scripts/utf8.sh", b"\xffpython3\n");
        assert!(run(&root).is_ok());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}
