use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use syn::{
    Attribute, Expr, ExprPath, Item, ItemExternCrate, ItemType, ItemUse, Macro, Meta,
    Path as SynPath, Stmt, Token, Type, UseTree,
    punctuated::Punctuated,
    visit::{self, Visit},
};

#[test]
fn production_file_backed_sqlite_openers_are_centrally_audited() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut observed = BTreeMap::<(PathBuf, String), usize>::new();
    collect_raw_file_openers_from_roots(
        &workspace_root,
        &production_source_roots(&workspace_root),
        &mut observed,
    );

    let expected = BTreeMap::from([
        // Derived helpers open only through this shared-lifecycle constructor.
        // Its wrapper retains both the SQLite connection and lifecycle guard
        // until SQLite closes, so it is a central guarded opener rather than
        // an exemption for helper-local raw opens.
        (
            (
                PathBuf::from("crates/kanban-local/src/sqlite_connection.rs"),
                "open".to_owned(),
            ),
            1,
        ),
        // The five central kanban-sqlite constructors acquire and retain the
        // matching lifecycle authority around these exact raw open calls:
        // shared read/write, shared read-only, the two exclusive immutable
        // read-only paths, and an exclusive-authority-owned replacement
        // inspection opener.
        (
            (
                PathBuf::from("crates/kanban-sqlite/src/db.rs"),
                "open".to_owned(),
            ),
            2,
        ),
        (
            (
                PathBuf::from("crates/kanban-sqlite/src/db.rs"),
                "open_with_flags".to_owned(),
            ),
            3,
        ),
    ]);
    assert_eq!(
        observed, expected,
        "file-backed SQLite openers must stay in guarded constructors or the exact phase-two replacement exemption"
    );
}

fn production_source_roots(workspace_root: &Path) -> Vec<PathBuf> {
    vec![
        workspace_root.join("crates"),
        workspace_root.join("apps/desktop/src-tauri"),
    ]
}

fn collect_raw_file_openers(
    workspace_root: &Path,
    directory: &Path,
    observed: &mut BTreeMap<(PathBuf, String), usize>,
) {
    let directory = directory.to_path_buf();
    collect_raw_file_openers_from_roots(workspace_root, std::slice::from_ref(&directory), observed);
}

fn collect_raw_file_openers_from_roots(
    workspace_root: &Path,
    directories: &[PathBuf],
    observed: &mut BTreeMap<(PathBuf, String), usize>,
) {
    let mut source_files = Vec::new();
    for directory in directories {
        collect_production_source_files(directory, &mut source_files);
    }
    let parsed = source_files
        .into_iter()
        .map(|path| {
            let source = fs::read_to_string(&path).unwrap();
            let syntax = syn::parse_file(&source).unwrap_or_else(|error| {
                panic!(
                    "raw SQLite opener audit could not parse {}: {error}",
                    path.display()
                )
            });
            (path, syntax)
        })
        .collect::<Vec<_>>();
    let shared_aliases = shared_connection_aliases(&parsed);
    for (path, syntax) in parsed {
        for (method, count) in raw_sqlite_openers_in_file(&syntax, &shared_aliases) {
            observed.insert(
                (
                    path.strip_prefix(workspace_root).unwrap().to_path_buf(),
                    method,
                ),
                count,
            );
        }
    }
}

fn collect_production_source_files(directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if matches!(
                path.file_name().and_then(|value| value.to_str()),
                Some("tests" | "benches" | "examples")
            ) {
                continue;
            }
            collect_production_source_files(&path, output);
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        output.push(path);
    }
}

fn raw_sqlite_openers(source: &str) -> BTreeMap<String, usize> {
    let syntax = syn::parse_file(source)
        .unwrap_or_else(|error| panic!("raw SQLite opener audit could not parse Rust: {error}"));
    raw_sqlite_openers_in_file(&syntax, &BTreeSet::new())
}

fn raw_sqlite_openers_in_file(
    syntax: &syn::File,
    shared_aliases: &BTreeSet<String>,
) -> BTreeMap<String, usize> {
    let aliases = AliasResolution::from_file_with_seed(syntax, shared_aliases);
    let mut visitor = RawOpenerVisitor {
        aliases: &aliases,
        observed: BTreeMap::new(),
    };
    visitor.visit_file(syntax);
    visitor.observed
}

fn shared_connection_aliases(files: &[(PathBuf, syn::File)]) -> BTreeSet<String> {
    let mut shared = BTreeSet::new();
    loop {
        let mut changed = false;
        for (_path, file) in files {
            let aliases = AliasResolution::from_file_with_seed(file, &shared);
            for alias in aliases.connection_aliases {
                changed |= shared.insert(alias);
            }
        }
        if !changed {
            return shared;
        }
    }
}

#[derive(Debug)]
enum UseBinding {
    Named { path: Vec<String>, alias: String },
    Glob { path: Vec<String> },
}

#[derive(Debug, Default)]
struct AliasCollector {
    crate_aliases: BTreeSet<String>,
    use_bindings: Vec<UseBinding>,
    type_aliases: Vec<(String, Vec<String>)>,
}

impl AliasCollector {
    fn collect_extern_crate(&mut self, item: &ItemExternCrate) {
        if item.ident == "rusqlite" {
            self.crate_aliases.insert(item.rename.as_ref().map_or_else(
                || item.ident.to_string(),
                |(_as_token, alias)| alias.to_string(),
            ));
        }
    }

    fn collect_use(&mut self, item: &ItemUse) {
        flatten_use_tree(&item.tree, &mut Vec::new(), &mut self.use_bindings);
    }

    fn collect_type(&mut self, item: &ItemType) {
        let Type::Path(path) = item.ty.as_ref() else {
            return;
        };
        if path.qself.is_none() {
            self.type_aliases
                .push((item.ident.to_string(), path_segments(&path.path)));
        }
    }
}

impl<'ast> Visit<'ast> for AliasCollector {
    fn visit_item(&mut self, item: &'ast Item) {
        if item_is_test_only(item) {
            return;
        }
        match item {
            Item::ExternCrate(item) => self.collect_extern_crate(item),
            Item::Use(item) => self.collect_use(item),
            Item::Type(item) => self.collect_type(item),
            _ => {}
        }
        visit::visit_item(self, item);
    }
}

fn flatten_use_tree(tree: &UseTree, prefix: &mut Vec<String>, output: &mut Vec<UseBinding>) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            flatten_use_tree(&path.tree, prefix, output);
            prefix.pop();
        }
        UseTree::Name(name) => {
            if name.ident != "self" {
                let mut path = prefix.clone();
                path.push(name.ident.to_string());
                output.push(UseBinding::Named {
                    alias: name.ident.to_string(),
                    path,
                });
            }
        }
        UseTree::Rename(rename) => {
            let mut path = prefix.clone();
            if rename.ident != "self" {
                path.push(rename.ident.to_string());
            }
            output.push(UseBinding::Named {
                path,
                alias: rename.rename.to_string(),
            });
        }
        UseTree::Glob(_) => output.push(UseBinding::Glob {
            path: prefix.clone(),
        }),
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use_tree(item, prefix, output);
            }
        }
    }
}

#[derive(Debug)]
struct AliasResolution {
    crate_aliases: BTreeSet<String>,
    connection_aliases: BTreeSet<String>,
}

impl AliasResolution {
    fn from_file_with_seed(file: &syn::File, seed: &BTreeSet<String>) -> Self {
        let mut collector = AliasCollector::default();
        collector.visit_file(file);
        let mut aliases = Self {
            crate_aliases: BTreeSet::from(["rusqlite".to_owned()])
                .into_iter()
                .chain(collector.crate_aliases)
                .collect(),
            connection_aliases: seed.clone(),
        };
        loop {
            let mut changed = false;
            for binding in &collector.use_bindings {
                match binding {
                    UseBinding::Named { path, alias } => {
                        if aliases.is_crate_path(path) {
                            changed |= aliases.crate_aliases.insert(alias.clone());
                        }
                        if aliases.is_connection_path(path) {
                            changed |= aliases.connection_aliases.insert(alias.clone());
                        }
                    }
                    UseBinding::Glob { path } if aliases.is_crate_path(path) => {
                        changed |= aliases.connection_aliases.insert("Connection".to_owned());
                    }
                    UseBinding::Glob { .. } => {}
                }
            }
            for (alias, path) in &collector.type_aliases {
                if aliases.is_connection_path(path) {
                    changed |= aliases.connection_aliases.insert(alias.clone());
                }
            }
            if !changed {
                break;
            }
        }
        aliases
    }

    fn is_crate_path(&self, path: &[String]) -> bool {
        path.len() == 1 && self.crate_aliases.contains(&path[0])
    }

    fn is_connection_path(&self, path: &[String]) -> bool {
        match path {
            [single] => self.connection_aliases.contains(single),
            [crate_name, connection] => {
                self.crate_aliases.contains(crate_name) && connection == "Connection"
            }
            _ => path
                .last()
                .is_some_and(|last| self.connection_aliases.contains(last)),
        }
    }

    fn is_connection_type(&self, ty: &Type) -> bool {
        let Type::Path(path) = ty else {
            return false;
        };
        path.qself.is_none() && self.is_connection_path(&path_segments(&path.path))
    }
}

struct RawOpenerVisitor<'a> {
    aliases: &'a AliasResolution,
    observed: BTreeMap<String, usize>,
}

impl<'ast> Visit<'ast> for RawOpenerVisitor<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        if item_is_test_only(item) {
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_stmt(&mut self, statement: &'ast Stmt) {
        if statement_attributes(statement)
            .iter()
            .any(attribute_requires_test)
        {
            return;
        }
        visit::visit_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'ast Expr) {
        if expression_attributes(expression)
            .iter()
            .any(attribute_requires_test)
        {
            return;
        }
        visit::visit_expr(self, expression);
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        if arm.attrs.iter().any(attribute_requires_test) {
            return;
        }
        visit::visit_arm(self, arm);
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        for method in macro_raw_sqlite_openers(mac, self.aliases) {
            *self.observed.entry(method).or_default() += 1;
        }
        visit::visit_macro(self, mac);
    }

    fn visit_expr_path(&mut self, expression: &'ast ExprPath) {
        let Some(method) = expression.path.segments.last() else {
            return;
        };
        let method = method.ident.to_string();
        if matches!(
            method.as_str(),
            "open" | "open_with_flags" | "open_with_flags_and_vfs"
        ) {
            let connection = expression.qself.as_ref().map_or_else(
                || {
                    let mut qualifier = path_segments(&expression.path);
                    qualifier.pop();
                    self.aliases.is_connection_path(&qualifier)
                },
                |qself| self.aliases.is_connection_type(&qself.ty),
            );
            if connection {
                *self.observed.entry(method).or_default() += 1;
            }
        }
        visit::visit_expr_path(self, expression);
    }
}

fn macro_raw_sqlite_openers(mac: &Macro, aliases: &AliasResolution) -> Vec<String> {
    let tokens = tokenize_macro_body(&mac.tokens.to_string());
    let mut observed = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if !matches!(
            token.as_str(),
            "open" | "open_with_flags" | "open_with_flags_and_vfs"
        ) {
            continue;
        }
        let direct_connection = index >= 2
            && tokens[index - 1] == "::"
            && aliases.connection_aliases.contains(&tokens[index - 2]);
        let crate_connection = index >= 4
            && tokens[index - 1] == "::"
            && tokens[index - 2] == "Connection"
            && tokens[index - 3] == "::"
            && aliases.crate_aliases.contains(&tokens[index - 4]);
        let qualified_connection = index >= 4
            && tokens[index - 1] == "::"
            && tokens[index - 2] == ">"
            && aliases.connection_aliases.contains(&tokens[index - 3])
            && tokens[index - 4] == "<";
        if direct_connection || crate_connection || qualified_connection {
            observed.push(token.clone());
            continue;
        }
        if macro_opener_qualifier_contains_metavariable(&tokens, index) {
            panic!("raw SQLite opener audit rejected unresolved macro qualifier for `{token}`");
        }
    }
    observed
}

fn macro_opener_qualifier_contains_metavariable(tokens: &[String], opener: usize) -> bool {
    if opener < 2 || tokens[opener - 1] != "::" {
        return false;
    }
    let qualifier = &tokens[..opener - 1];
    let start = qualifier
        .iter()
        .rposition(|token| {
            matches!(
                token.as_str(),
                "{" | "}" | "(" | ")" | "[" | "]" | ";" | "," | "=" | "=>" | ":" | "."
            )
        })
        .map_or(0, |index| index + 1);
    qualifier[start..].iter().any(|token| token == "$")
}

fn tokenize_macro_body(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if let Some(end) = rust_literal_end(bytes, index) {
            index = end;
            continue;
        }
        let byte = bytes[index];
        if byte == b'_' || byte.is_ascii_alphabetic() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
            {
                index += 1;
            }
            tokens.push(source[start..index].to_owned());
            continue;
        }
        if byte == b':' && bytes.get(index + 1) == Some(&b':') {
            tokens.push("::".to_owned());
            index += 2;
            continue;
        }
        if byte == b'=' && bytes.get(index + 1) == Some(&b'>') {
            tokens.push("=>".to_owned());
            index += 2;
            continue;
        }
        if matches!(
            byte,
            b'<' | b'>'
                | b'$'
                | b'{'
                | b'}'
                | b'('
                | b')'
                | b'['
                | b']'
                | b';'
                | b','
                | b'='
                | b':'
                | b'.'
        ) {
            tokens.push(char::from(byte).to_string());
        }
        index += 1;
    }
    tokens
}

fn rust_literal_end(bytes: &[u8], start: usize) -> Option<usize> {
    let (prefix_end, quote) = match bytes.get(start..)? {
        [b'"', ..] => (start, b'"'),
        [b'b' | b'c', b'"', ..] => (start + 1, b'"'),
        [b'\'', ..] => {
            let mut index = start + 1;
            let mut escaped = false;
            while let Some(byte) = bytes.get(index).copied() {
                if byte == b'\'' && !escaped {
                    return Some(index + 1);
                }
                if byte == b'\n' {
                    return None;
                }
                escaped = byte == b'\\' && !escaped;
                if byte != b'\\' {
                    escaped = false;
                }
                index += 1;
            }
            return None;
        }
        [b'b', b'\'', ..] => {
            let mut index = start + 2;
            let mut escaped = false;
            while let Some(byte) = bytes.get(index).copied() {
                if byte == b'\'' && !escaped {
                    return Some(index + 1);
                }
                escaped = byte == b'\\' && !escaped;
                if byte != b'\\' {
                    escaped = false;
                }
                index += 1;
            }
            return None;
        }
        _ => {
            let mut prefix_end = start;
            if matches!(bytes.get(prefix_end), Some(b'b' | b'c')) {
                prefix_end += 1;
            }
            if bytes.get(prefix_end) != Some(&b'r') {
                return None;
            }
            prefix_end += 1;
            while bytes.get(prefix_end) == Some(&b'#') {
                prefix_end += 1;
            }
            if bytes.get(prefix_end) != Some(&b'"') {
                return None;
            }
            (prefix_end, b'"')
        }
    };

    let hashes = bytes[start..prefix_end]
        .iter()
        .filter(|byte| **byte == b'#')
        .count();
    let raw = hashes > 0 || bytes[start..prefix_end].contains(&b'r');
    let mut index = prefix_end + 1;
    let mut escaped = false;
    while let Some(byte) = bytes.get(index).copied() {
        if byte == quote
            && (!escaped || raw)
            && (!raw
                || bytes
                    .get(index + 1..index + 1 + hashes)
                    .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#')))
        {
            return Some(index + 1 + hashes);
        }
        escaped = !raw && byte == b'\\' && !escaped;
        if byte != b'\\' {
            escaped = false;
        }
        index += 1;
    }
    Some(bytes.len())
}

fn path_segments(path: &SynPath) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect()
}

fn item_is_test_only(item: &Item) -> bool {
    item_attributes(item).iter().any(attribute_requires_test)
}

fn statement_attributes(statement: &Stmt) -> &[Attribute] {
    match statement {
        Stmt::Local(local) => &local.attrs,
        Stmt::Item(item) => item_attributes(item),
        Stmt::Expr(expression, _) => expression_attributes(expression),
        Stmt::Macro(mac) => &mac.attrs,
    }
}

fn expression_attributes(expression: &Expr) -> &[Attribute] {
    match expression {
        Expr::Array(expression) => &expression.attrs,
        Expr::Assign(expression) => &expression.attrs,
        Expr::Async(expression) => &expression.attrs,
        Expr::Await(expression) => &expression.attrs,
        Expr::Binary(expression) => &expression.attrs,
        Expr::Block(expression) => &expression.attrs,
        Expr::Break(expression) => &expression.attrs,
        Expr::Call(expression) => &expression.attrs,
        Expr::Cast(expression) => &expression.attrs,
        Expr::Closure(expression) => &expression.attrs,
        Expr::Const(expression) => &expression.attrs,
        Expr::Continue(expression) => &expression.attrs,
        Expr::Field(expression) => &expression.attrs,
        Expr::ForLoop(expression) => &expression.attrs,
        Expr::Group(expression) => &expression.attrs,
        Expr::If(expression) => &expression.attrs,
        Expr::Index(expression) => &expression.attrs,
        Expr::Infer(expression) => &expression.attrs,
        Expr::Let(expression) => &expression.attrs,
        Expr::Lit(expression) => &expression.attrs,
        Expr::Loop(expression) => &expression.attrs,
        Expr::Macro(expression) => &expression.attrs,
        Expr::Match(expression) => &expression.attrs,
        Expr::MethodCall(expression) => &expression.attrs,
        Expr::Paren(expression) => &expression.attrs,
        Expr::Path(expression) => &expression.attrs,
        Expr::Range(expression) => &expression.attrs,
        Expr::RawAddr(expression) => &expression.attrs,
        Expr::Reference(expression) => &expression.attrs,
        Expr::Repeat(expression) => &expression.attrs,
        Expr::Return(expression) => &expression.attrs,
        Expr::Struct(expression) => &expression.attrs,
        Expr::Try(expression) => &expression.attrs,
        Expr::TryBlock(expression) => &expression.attrs,
        Expr::Tuple(expression) => &expression.attrs,
        Expr::Unary(expression) => &expression.attrs,
        Expr::Unsafe(expression) => &expression.attrs,
        Expr::While(expression) => &expression.attrs,
        Expr::Yield(expression) => &expression.attrs,
        Expr::Verbatim(_) => &[],
        _ => &[],
    }
}

fn item_attributes(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        Item::Verbatim(_) => &[],
        _ => &[],
    }
}

fn attribute_requires_test(attribute: &Attribute) -> bool {
    if !attribute.path().is_ident("cfg") {
        return false;
    }
    attribute
        .parse_args::<Meta>()
        .is_ok_and(|meta| cfg_meta_requires_test(&meta))
}

fn cfg_meta_requires_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::List(list) if list.path.is_ident("all") => list
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .is_ok_and(|nested| nested.iter().any(cfg_meta_requires_test)),
        Meta::List(list) if list.path.is_ident("any") => list
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .is_ok_and(|nested| !nested.is_empty() && nested.iter().all(cfg_meta_requires_test)),
        Meta::List(_) | Meta::NameValue(_) => false,
    }
}

#[test]
fn opener_scanner_detects_connection_aliases_and_vfs_openers() {
    let source = r#"
use rusqlite::Connection as Db;
use rusqlite as sql;
type ReadConnection = Db;
type VfsConnection = ReadConnection;

fn open(path: &std::path::Path, flags: rusqlite::OpenFlags, vfs: &str) {
    let _ = Db::open(path);
    let _ = sql::Connection::open(path);
    let _ = rusqlite::Connection::open(path);
    let _ = ReadConnection::open_with_flags(path, flags);
    let _ = VfsConnection::open_with_flags_and_vfs(path, flags, vfs);
}
"#;

    assert_eq!(
        raw_sqlite_openers(source),
        BTreeMap::from([
            ("open".to_owned(), 3),
            ("open_with_flags".to_owned(), 1),
            ("open_with_flags_and_vfs".to_owned(), 1),
        ])
    );
}

#[test]
fn opener_scanner_detects_wildcards_qself_and_function_pointers() {
    let source = r#"
use rusqlite::*;
type ReadConnection = Connection;

fn open(path: &std::path::Path, flags: rusqlite::OpenFlags) {
    let direct = <Connection>::open;
    let flags_opener = ReadConnection::open_with_flags;
    let _ = direct(path);
    let _ = flags_opener(path, flags);
}
"#;

    assert_eq!(
        raw_sqlite_openers(source),
        BTreeMap::from([("open".to_owned(), 1), ("open_with_flags".to_owned(), 1),])
    );
}

#[test]
fn opener_scanner_detects_extern_crate_aliases_and_macro_tokens() {
    let source = r#"
extern crate rusqlite as sql;
use sql::Connection;

macro_rules! open_database {
    ($path:expr) => {
        Connection::open($path)
    };
}

fn open_direct(path: &std::path::Path) {
    let _ = sql::Connection::open_with_flags(path, sql::OpenFlags::SQLITE_OPEN_READ_ONLY);
}
"#;

    assert_eq!(
        raw_sqlite_openers(source),
        BTreeMap::from([("open".to_owned(), 1), ("open_with_flags".to_owned(), 1),])
    );
}

#[test]
fn opener_scanner_rejects_unresolved_macro_type_openers() {
    let source = r#"
macro_rules! invoke_open {
    ($ty:ty, $path:expr) => {
        <$ty>::open($path)
    };
}

fn open(path: &std::path::Path) {
    let _ = invoke_open!(rusqlite::Connection, path);
}
"#;

    let result = std::panic::catch_unwind(|| raw_sqlite_openers(source));
    assert!(
        result.is_err(),
        "a macro type parameter can hide a raw SQLite opener and must fail closed"
    );
}

#[test]
fn opener_scanner_allows_unresolved_macros_without_static_openers() {
    let source = r#"
macro_rules! invoke_connect {
    ($ty:ty, $path:expr) => {
        <$ty>::connect($path)
    };
}

macro_rules! invoke_instance_open {
    ($value:expr) => {
        $value.open()
    };
}
"#;

    assert!(raw_sqlite_openers(source).is_empty());
}

#[test]
fn opener_scanner_ignores_comments_and_strings() {
    let source = r#"
use rusqlite::Connection;

const DESCRIPTION: &str = "Connection::open(path)";
// Connection::open_with_flags(path, flags);
/* rusqlite::Connection::open_with_flags_and_vfs(path, flags, vfs); */

macro_rules! documentation {
    () => {
        "Connection::open(path)"
    };
}
"#;

    assert!(raw_sqlite_openers(source).is_empty());
}

#[test]
fn opener_scanner_skips_cfg_test_statements_and_expressions() {
    let source = r#"
use rusqlite::Connection;

fn open(path: &std::path::Path) {
    #[cfg(test)]
    let _ = Connection::open(path);

    let _ = {
        #[cfg(test)]
        Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
    };

    #[cfg(test)]
    raw_test_only!(Connection::open(path));

    let _ = Connection::open(path);
}
"#;

    assert_eq!(
        raw_sqlite_openers(source),
        BTreeMap::from([("open".to_owned(), 1)])
    );
}

#[test]
fn opener_scanner_skips_cfg_test_match_arms() {
    let source = r#"
use rusqlite::Connection;

fn open(path: &std::path::Path, test_only: bool) {
    match test_only {
        #[cfg(test)]
        true => {
            let _ = Connection::open_with_flags(
                path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            );
        }
        false => {
            let _ = Connection::open(path);
        }
    }
}
"#;

    assert_eq!(
        raw_sqlite_openers(source),
        BTreeMap::from([("open".to_owned(), 1)])
    );
}

#[test]
fn opener_scanner_skips_cfg_test_items_without_truncating_later_production_items() {
    let source = r#"
use rusqlite::Connection;

#[cfg(test)]
mod tests {
    use super::Connection;

    fn ignored(path: &std::path::Path) {
        let _ = Connection::open(path);
    }
}

fn production_after_test_module(path: &std::path::Path) {
    let _ = Connection::open(path);
}
"#;

    assert_eq!(
        raw_sqlite_openers(source),
        BTreeMap::from([("open".to_owned(), 1)])
    );
}

#[test]
fn production_scan_resolves_cross_module_reexported_connection_aliases() {
    let workspace = tempfile::tempdir().unwrap();
    let source_root = workspace.path().join("crates/example/src");
    fs::create_dir_all(&source_root).unwrap();
    fs::write(
        source_root.join("aliases.rs"),
        "pub type Database = rusqlite::Connection;\n",
    )
    .unwrap();
    fs::write(
        source_root.join("consumer.rs"),
        r#"
use crate::aliases::Database as ReadDatabase;

fn open(path: &std::path::Path) {
    let _ = ReadDatabase::open(path);
}
"#,
    )
    .unwrap();

    let mut observed = BTreeMap::new();
    collect_raw_file_openers(
        workspace.path(),
        &workspace.path().join("crates"),
        &mut observed,
    );

    assert_eq!(
        observed,
        BTreeMap::from([(
            (
                PathBuf::from("crates/example/src/consumer.rs"),
                "open".to_owned(),
            ),
            1,
        )])
    );
}

#[test]
fn production_scan_roots_include_desktop_tauri_rust() {
    let workspace_root = Path::new("/workspace");
    assert_eq!(
        production_source_roots(workspace_root),
        vec![
            workspace_root.join("crates"),
            workspace_root.join("apps/desktop/src-tauri"),
        ]
    );
}
