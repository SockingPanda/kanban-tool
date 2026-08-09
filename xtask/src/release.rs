//! Stage09 09A real-host proof 的仓库语义校验与 deterministic receipt。
//!
//! 该模块只验证机器可读 evidence；host/browser 进程生命周期仍由 release launcher 编排。
//! 未接入的 axe、visual、performance、stress、package gate 必须保持 `pending`，receipt
//! 不会把 pending evidence 标记为总 release ready。

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Write,
    path::{Component, Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use xtask::{ToolResult, check_contract};

const LEDGER_PATH: &str = "apps/web/docs/capability-ledger.release.json";
const MARKDOWN_LEDGER_PATH: &str = "apps/web/docs/capability-ledger.md";
const RELEASE_SPEC_PATH: &str = "apps/web/tests/release-proof.spec.ts";
const PLAYWRIGHT_CONFIG_PATH: &str = "apps/web/playwright.config.ts";
const PACKAGE_LOCK_PATH: &str = "pnpm-lock.yaml";
const DEFAULT_ARTIFACT_PATH: &str = "apps/web/dist";
const DEFAULT_EVIDENCE_PATH: &str = "output/release";
const DEFAULT_RECEIPT_PATH: &str = "output/release/release-proof-receipt.json";
const BASE_REVISION: &str = "311ef2fdbf238bee8b66e715cab0001df7cd186d";

#[derive(Debug, Deserialize)]
struct ReleaseLedger {
    schema_version: u64,
    stage: String,
    base_revision: String,
    rows: Vec<ReleaseLedgerRow>,
    gates: ReleaseGates,
}

#[derive(Debug, Deserialize)]
struct ReleaseLedgerRow {
    id: String,
    status: String,
    flow: Option<String>,
    chromium: String,
    firefox: String,
    requires_real_host: bool,
}

#[derive(Debug, Deserialize)]
struct ReleaseGates {
    real_host: String,
    axe: String,
    visual: String,
    performance: String,
    functional_2k: String,
    stress_5k: String,
    package: String,
}

#[derive(Debug, Serialize)]
struct ReleaseReceipt {
    schema_version: u64,
    stage: &'static str,
    run_id: String,
    start_sha: String,
    git_revision: String,
    base_revision: &'static str,
    worktree_clean: bool,
    evidence: EvidenceReceipt,
    build: BuildReceipt,
    artifact: ArtifactReceipt,
    host: HostReceipt,
    database: DatabaseReceipt,
    seed_task: TaskReceipt,
    created_tasks: Vec<TaskReceipt>,
    gates: GateReceipt,
}

#[derive(Debug, Serialize)]
struct EvidenceReceipt {
    directory: String,
    host: String,
    browser: String,
    sha256: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct BuildReceipt {
    xtask_source_sha256: String,
    target_scope: &'static str,
    kanban_binary_sha256: String,
    cargo_lock_sha256: String,
    package_lock_sha256: String,
    release_spec_sha256: String,
    playwright_config_sha256: String,
    protocol_source_sha256: String,
}

#[derive(Debug, Serialize)]
struct ArtifactReceipt {
    path: &'static str,
    manifest_sha256: String,
    build_id: String,
    server_version: String,
    protocol_version: String,
    file_count: usize,
    payload_bytes: u64,
    verified_snapshot: bool,
}

#[derive(Debug, Serialize)]
struct HostReceipt {
    run_id: String,
    start_sha: String,
    start_clean: bool,
    base_url_scope: &'static str,
    pid: u32,
    pid_identity: String,
    command_line: String,
    health_ok: bool,
    health_db: String,
    health_version: String,
    runtime_build_id: String,
    manifest_build_id: String,
    binary_path: String,
    binary_sha256: String,
    live_binary_sha256: String,
    db_path_before_restart: String,
    db_path_after_restart: String,
    db_path_after_browser: String,
    db_identity_before_restart: String,
    db_identity_after_restart: String,
    db_identity_after_browser: String,
    host_negative_checked: bool,
    origin_null_rejected: bool,
    origin_negative_checked: bool,
    csp_checked: bool,
    nosniff_checked: bool,
    referrer_policy_checked: bool,
    frame_deny_checked: bool,
}

#[derive(Debug, Serialize)]
struct DatabaseReceipt {
    path_scope: &'static str,
    path: String,
    path_sha256: String,
    identity_before_restart: String,
    identity_after_restart: String,
    identity_after_browser: String,
    fingerprint_before_restart: String,
    fingerprint_after_restart: String,
    fingerprint_after_browser: String,
    migration_before: i64,
    migration_after: i64,
    migration_unchanged: bool,
    canonical_host: bool,
}

#[derive(Debug, Serialize)]
struct TaskReceipt {
    browser: String,
    id: String,
    title: String,
    project: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskEvidence {
    browser: String,
    id: String,
    title: String,
    project: String,
    board_slug: String,
}

#[derive(Debug, Serialize)]
struct GateReceipt {
    real_host: &'static str,
    axe: &'static str,
    visual: &'static str,
    performance: &'static str,
    functional_2k: &'static str,
    stress_5k: &'static str,
    package: &'static str,
}

#[derive(Debug, Default)]
struct Options {
    root: PathBuf,
    evidence: Option<PathBuf>,
    artifact: Option<PathBuf>,
    out: Option<PathBuf>,
}

/// 分发 `xtask release check|receipt`。
pub fn run(command: Option<&str>, arguments: &[String]) -> ToolResult<()> {
    let options = parse_options(arguments)?;
    let root = lexical_root(&options.root)?;
    match command {
        Some("check") => check(&root),
        Some("receipt") => receipt(&root, &options),
        Some(other) => Err(error(format!("release 不支持子命令: {other}"))),
        None => Err(error("release 缺少子命令")),
    }
}

fn parse_options(arguments: &[String]) -> ToolResult<Options> {
    let mut options = Options {
        root: PathBuf::from("."),
        ..Options::default()
    };
    let mut index = 0;
    while index < arguments.len() {
        let flag = arguments[index].as_str();
        let target = match flag {
            "--root" => Some(&mut options.root),
            "--evidence" => Some(options.evidence.get_or_insert_with(PathBuf::new)),
            "--artifact" => Some(options.artifact.get_or_insert_with(PathBuf::new)),
            "--out" => Some(options.out.get_or_insert_with(PathBuf::new)),
            _ => None,
        };
        if let Some(target) = target {
            index += 1;
            let value = arguments
                .get(index)
                .ok_or_else(|| error(format!("{flag} 缺少路径")))?;
            *target = PathBuf::from(value);
        } else {
            return Err(error(format!("release 参数无效: {flag}")));
        }
        index += 1;
    }
    Ok(options)
}

fn check(root: &Path) -> ToolResult<()> {
    let ledger = load_ledger(root)?;
    validate_ledger(root, &ledger)?;
    validate_base_and_provenance(root, &ledger)?;
    println!(
        "release-proof 09A inventory 已通过: ready_rows={} pending_rows={} real_host_gate={}",
        ledger
            .rows
            .iter()
            .filter(|row| row.status == "ready")
            .count(),
        ledger
            .rows
            .iter()
            .filter(|row| row.status == "pending")
            .count(),
        ledger.gates.real_host,
    );
    Ok(())
}

fn receipt(root: &Path, options: &Options) -> ToolResult<()> {
    let ledger = load_ledger(root)?;
    validate_ledger(root, &ledger)?;
    validate_base_and_provenance(root, &ledger)?;
    let evidence_root = options
        .evidence
        .clone()
        .unwrap_or_else(|| root.join(DEFAULT_EVIDENCE_PATH));
    let evidence_root = resolve_root_relative(root, &evidence_root)?;
    validate_approved_evidence_root(root, &evidence_root)?;
    reject_symlink_chain(&evidence_root)?;
    let host_evidence = read_regular_json(&evidence_root.join("host.json"))?;
    let browser_evidence = read_regular_json(&evidence_root.join("browser.json"))?;
    let chromium_evidence = read_regular_json(&evidence_root.join("browser-chromium.json"))?;
    let firefox_evidence = read_regular_json(&evidence_root.join("browser-firefox.json"))?;
    validate_host_evidence(&host_evidence)?;
    let host_url = string_field(&host_evidence, "base_url")?.to_owned();
    validate_loopback_url(&host_url)?;
    let run_id = string_field(&host_evidence, "run_id")?.to_owned();
    let created_tasks = validate_browser_evidence(
        &browser_evidence,
        &ledger,
        &host_url,
        &run_id,
        &chromium_evidence,
        &firefox_evidence,
    )?;

    let artifact_path = options
        .artifact
        .clone()
        .unwrap_or_else(|| root.join(DEFAULT_ARTIFACT_PATH));
    let artifact_path = resolve_root_relative(root, &artifact_path)?;
    let artifact = kanban_web_artifact::verify_directory(&artifact_path, env!("CARGO_PKG_VERSION"))
        .map_err(|source| error(format!("Web artifact snapshot 校验失败: {source}")))?;
    let payload_bytes = artifact
        .payloads()
        .try_fold(0_u64, |total, payload| {
            total.checked_add(payload.descriptor().bytes)
        })
        .ok_or_else(|| error("Web artifact payload bytes overflow"))?;
    let host_data = object_field(&host_evidence, "health_after_browser")?;
    let runtime_data = object_field(&host_evidence, "runtime")?;
    let manifest_data = object_field(&host_evidence, "manifest")?;
    let expected_build_id = artifact.manifest().build_id.clone();
    if string_field(manifest_data, "buildId")? != expected_build_id
        || string_field(runtime_data, "webBuildId")? != expected_build_id
    {
        return Err(error(
            "host/runtime/manifest buildId 未与 verified Web artifact 对齐",
        ));
    }
    if string_field(host_data, "db")? != "turso"
        || !bool_field(host_data, "ok")?
        || string_field(host_data, "version")? != env!("CARGO_PKG_VERSION")
    {
        return Err(error("real kanban serve health 未通过"));
    }
    let live_health = curl_json(&format!("{host_url}/health"))?;
    let live_health_data = live_health
        .get("data")
        .ok_or_else(|| error("health 缺少 data"))?;
    let live_health_ok = bool_field(live_health_data, "ok")?;
    if !live_health_ok {
        return Err(error("live host health.ok=false"));
    }
    let live_health_db = string_field(live_health_data, "db")?;
    let live_health_version = string_field(live_health_data, "version")?;
    if live_health_db != "turso" || live_health_version != env!("CARGO_PKG_VERSION") {
        return Err(error("live host health identity 未通过"));
    }
    let live_runtime = curl_json(&format!("{host_url}/app/runtime.json"))?;
    let live_manifest = curl_json(&format!("{host_url}/app/manifest.json"))?;
    let live_runtime_build_id = string_field(&live_runtime, "webBuildId")?;
    let live_manifest_build_id = string_field(&live_manifest, "buildId")?;
    if live_runtime_build_id != expected_build_id || live_manifest_build_id != expected_build_id {
        return Err(error(
            "live runtime/manifest buildId 未与 verified Web artifact 对齐",
        ));
    }
    if string_field(host_data, "db")? != live_health_db
        || string_field(host_data, "version")? != live_health_version
        || string_field(runtime_data, "webBuildId")? != live_runtime_build_id
        || string_field(manifest_data, "buildId")? != live_manifest_build_id
    {
        return Err(error("host evidence 与 live host identity 不一致"));
    }
    let security = probe_live_security(&host_url)?;
    if !security.host_negative_checked
        || !security.origin_negative_checked
        || !security.origin_null_rejected
        || !security.csp_checked
        || !security.nosniff_checked
        || !security.referrer_policy_checked
        || !security.frame_deny_checked
    {
        return Err(error("live host security probes 未通过"));
    }
    let live_doctor = curl_json(&format!("{host_url}/api/v1/maintenance/doctor"))?;
    let doctor_data = live_doctor
        .get("data")
        .ok_or_else(|| error("doctor 缺少 data"))?;
    let migration_after = i64_field(doctor_data, "migration_version")?;
    let migration_before = i64_field(&host_evidence, "migration_before")?;
    let migration_unchanged = migration_before == migration_after;
    if !migration_unchanged {
        return Err(error(
            "canonical DB migration version changed during release proof",
        ));
    }
    let db_path_before_restart = string_field(&host_evidence, "db_path_before_restart")?.to_owned();
    let db_path_after_restart = string_field(&host_evidence, "db_path_after_restart")?.to_owned();
    let db_path_after_browser = string_field(&host_evidence, "db_path_after_browser")?.to_owned();
    if db_path_before_restart != db_path_after_restart
        || db_path_after_restart != db_path_after_browser
    {
        return Err(error("canonical DB path changed across release proof"));
    }
    let db_path = lexical_absolute(Path::new(string_field(live_health_data, "db_path")?))?;
    if db_path.display().to_string() != db_path_after_browser {
        return Err(error("live health db_path 与 host evidence 不一致"));
    }
    validate_private_db_path(&db_path)?;
    reject_symlink_chain(&db_path)?;
    let db_bytes_hash = sha256_file(&db_path)?;
    let identity_before_restart =
        string_field(&host_evidence, "db_identity_before_restart")?.to_owned();
    let identity_after_restart =
        string_field(&host_evidence, "db_identity_after_restart")?.to_owned();
    let identity_after_browser =
        string_field(&host_evidence, "db_identity_after_browser")?.to_owned();
    let live_identity = db_identity(&db_path)?;
    validate_db_identity_triplet(
        &identity_before_restart,
        &identity_after_restart,
        &identity_after_browser,
        &live_identity,
    )?;
    let fingerprint_before_restart =
        string_field(&host_evidence, "db_fingerprint_before_restart")?.to_owned();
    let fingerprint_after_restart =
        string_field(&host_evidence, "db_fingerprint_after_restart")?.to_owned();
    let fingerprint_after_browser =
        string_field(&host_evidence, "db_fingerprint_after_browser")?.to_owned();
    let pid = u32_field(&host_evidence, "pid")?;
    let argv = process_argv(pid)?;
    let command_line = argv.join(" ");
    let evidence_identity = format!("{pid}:{}", string_field(&host_evidence, "start_after")?);
    let current_identity = process_identity(pid)?;
    if current_identity != evidence_identity {
        return Err(error("host PID start_after identity 与 live /proc 不一致"));
    }
    if command_line != string_field(&host_evidence, "command_line")? {
        return Err(error("host evidence command_line 与 live /proc 不一致"));
    }
    if argv != string_array_field(&host_evidence, "argv")? {
        return Err(error("host evidence argv 与 live /proc 不一致"));
    }
    let host_port = loopback_port(&host_url)?;
    let binary_path = lexical_absolute(Path::new(string_field(&host_evidence, "binary_path")?))?;
    let canonical_host =
        validate_host_argv(&argv, host_port, &db_path, &artifact_path, &binary_path)?;
    let binary_sha256 = sha256_file(&binary_path)?;
    let live_binary_sha256 = sha256_process_exe(pid)?;
    if binary_sha256 != string_field(&host_evidence, "binary_sha256")?
        || live_binary_sha256 != string_field(&host_evidence, "live_binary_sha256")?
        || binary_sha256 != live_binary_sha256
    {
        return Err(error("live host executable 未与 launcher binary SHA 绑定"));
    }
    let task_receipts = created_tasks
        .iter()
        .map(|task| live_task_receipt(&host_url, task))
        .collect::<ToolResult<Vec<_>>>()?;
    let seed_task = live_seed_task(&host_url)?;
    let package_sha = collect_evidence_hashes(&evidence_root)?;
    let start_sha = string_field(&host_evidence, "start_sha")?.to_owned();
    let start_clean = bool_field(&host_evidence, "start_clean")?;
    let current_revision = git_revision(root)?;
    validate_start_binding(&start_sha, &current_revision, start_clean)?;
    let worktree_clean = git_status_clean(root)?;
    if !worktree_clean {
        return Err(error(
            "release receipt requires a clean worktree; output/release must be ignored",
        ));
    }
    let cargo_lock_sha = sha256_file(&root.join("Cargo.lock"))?;
    let package_lock_sha = sha256_file(&root.join(PACKAGE_LOCK_PATH))?;
    let release_spec_sha = sha256_file(&root.join(RELEASE_SPEC_PATH))?;
    let playwright_config_sha = sha256_file(&root.join(PLAYWRIGHT_CONFIG_PATH))?;
    let protocol_sha = sha256_file(&root.join("crates/kanban-protocol/src/lib.rs"))?;
    let receipt = ReleaseReceipt {
        schema_version: 1,
        stage: "stage09-release-proof-09A",
        run_id: run_id.clone(),
        start_sha,
        git_revision: current_revision,
        base_revision: BASE_REVISION,
        worktree_clean,
        evidence: EvidenceReceipt {
            directory: evidence_root.display().to_string(),
            host: "host.json".to_owned(),
            browser: "browser.json".to_owned(),
            sha256: package_sha,
        },
        build: BuildReceipt {
            xtask_source_sha256: sha256_file(&root.join("xtask/src/release.rs"))?,
            target_scope: "shared-cargo-target",
            kanban_binary_sha256: binary_sha256.clone(),
            cargo_lock_sha256: cargo_lock_sha,
            package_lock_sha256: package_lock_sha,
            release_spec_sha256: release_spec_sha,
            playwright_config_sha256: playwright_config_sha,
            protocol_source_sha256: protocol_sha,
        },
        artifact: ArtifactReceipt {
            path: DEFAULT_ARTIFACT_PATH,
            manifest_sha256: artifact.manifest_sha256().to_owned(),
            build_id: artifact.manifest().build_id.clone(),
            server_version: artifact.manifest().server_version.clone(),
            protocol_version: artifact.manifest().protocol_version.clone(),
            file_count: artifact.payloads().len(),
            payload_bytes,
            verified_snapshot: true,
        },
        host: HostReceipt {
            run_id,
            start_sha: string_field(&host_evidence, "start_sha")?.to_owned(),
            start_clean,
            base_url_scope: "loopback-explicit-port",
            pid,
            pid_identity: current_identity,
            command_line,
            health_ok: live_health_ok,
            health_db: live_health_db.to_owned(),
            health_version: live_health_version.to_owned(),
            runtime_build_id: live_runtime_build_id.to_owned(),
            manifest_build_id: live_manifest_build_id.to_owned(),
            binary_path: binary_path.display().to_string(),
            binary_sha256,
            live_binary_sha256,
            db_path_before_restart,
            db_path_after_restart,
            db_path_after_browser,
            db_identity_before_restart: identity_before_restart.clone(),
            db_identity_after_restart: identity_after_restart.clone(),
            db_identity_after_browser: identity_after_browser.clone(),
            host_negative_checked: security.host_negative_checked,
            origin_null_rejected: security.origin_null_rejected,
            origin_negative_checked: security.origin_negative_checked,
            csp_checked: security.csp_checked,
            nosniff_checked: security.nosniff_checked,
            referrer_policy_checked: security.referrer_policy_checked,
            frame_deny_checked: security.frame_deny_checked,
        },
        database: DatabaseReceipt {
            path_scope: "temporary-canonical-db",
            path: db_path.display().to_string(),
            path_sha256: db_bytes_hash,
            identity_before_restart,
            identity_after_restart,
            identity_after_browser,
            fingerprint_before_restart,
            fingerprint_after_restart,
            fingerprint_after_browser,
            migration_before,
            migration_after,
            migration_unchanged,
            canonical_host,
        },
        seed_task,
        created_tasks: task_receipts,
        gates: GateReceipt {
            real_host: "passed",
            axe: "pending",
            visual: "pending",
            performance: "pending",
            functional_2k: "pending",
            stress_5k: "pending",
            package: "pending",
        },
    };
    let out = options
        .out
        .clone()
        .unwrap_or_else(|| root.join(DEFAULT_RECEIPT_PATH));
    let out = resolve_root_relative(root, &out)?;
    if out.parent() != Some(evidence_root.as_path()) {
        return Err(error("receipt 输出必须位于 approved evidence root"));
    }
    atomic_write_json(&out, &receipt)?;
    println!("release-proof 09A receipt 已原子写入: {}", out.display());
    Ok(())
}

fn load_ledger(root: &Path) -> ToolResult<ReleaseLedger> {
    let path = root.join(LEDGER_PATH);
    let ledger: ReleaseLedger = serde_json::from_slice(&read_regular_bytes(&path)?)?;
    if ledger.schema_version != 1 || ledger.stage != "stage09-release-proof-09A" {
        return Err(error("release ledger schema/stage 不匹配"));
    }
    Ok(ledger)
}

fn validate_ledger(root: &Path, ledger: &ReleaseLedger) -> ToolResult<()> {
    if ledger.base_revision != BASE_REVISION {
        return Err(error(
            "release ledger base_revision 必须固定到 Stage08 merge base",
        ));
    }
    let markdown = String::from_utf8(read_regular_bytes(&root.join(MARKDOWN_LEDGER_PATH))?)?;
    let expected = markdown_capability_ids(&markdown);
    let release_spec = String::from_utf8(read_regular_bytes(&root.join(RELEASE_SPEC_PATH))?)?;
    if release_spec.contains("route.fulfill")
        || release_spec.contains("addInitScript")
        || release_spec.contains("networkidle")
        || release_spec.contains("waitForTimeout")
        || release_spec.contains("test.skip")
        || release_spec.contains("test.fixme")
    {
        return Err(error(
            "real-host release spec 含 preview/mock/伪证 API 或等待",
        ));
    }
    let mut actual = BTreeSet::new();
    let mut ready = 0;
    for row in &ledger.rows {
        if !actual.insert(row.id.clone()) {
            return Err(error(format!("release ledger row 重复: {}", row.id)));
        }
        if row.requires_real_host && row.status == "ready" {
            let flow = row
                .flow
                .as_deref()
                .ok_or_else(|| error(format!("ready row 缺少 flow: {}", row.id)))?;
            let (path, anchor) = flow
                .split_once('#')
                .ok_or_else(|| error("flow 必须为 path#anchor"))?;
            let source = String::from_utf8(read_regular_bytes(&root.join(path))?)?;
            if source.contains("route.fulfill")
                || source.contains("addInitScript")
                || source.contains("networkidle")
                || source.contains("waitForTimeout")
                || source.contains("test.skip")
                || source.contains("test.fixme")
            {
                return Err(error("real-host flow 含 preview/mock/伪证 API 或等待"));
            }
            if !source.contains(anchor) {
                return Err(error(format!("flow anchor 不存在: {flow}")));
            }
            if row.chromium != "full" || row.firefox != "key" {
                return Err(error(format!(
                    "ready row 必须声明 Chromium full/Firefox key: {}",
                    row.id
                )));
            }
            ready += 1;
        } else if row.status != "pending" {
            return Err(error(format!(
                "row status 必须为 ready/pending: {}",
                row.id
            )));
        }
    }
    if expected != actual {
        return Err(error("release ledger 与 capability-ledger.md 行集合漂移"));
    }
    if ready == 0 {
        return Err(error("release ledger 至少需要一条 ready real-host flow"));
    }
    if ledger.gates.real_host != "ready"
        || ledger.gates.axe != "pending"
        || ledger.gates.visual != "pending"
        || ledger.gates.performance != "pending"
        || ledger.gates.functional_2k != "pending"
        || ledger.gates.stress_5k != "pending"
        || ledger.gates.package != "pending"
    {
        return Err(error("09A gate 状态必须仅 real_host=ready，其余 pending"));
    }
    Ok(())
}

fn validate_base_and_provenance(root: &Path, ledger: &ReleaseLedger) -> ToolResult<()> {
    let head = git_revision(root)?;
    let status = Command::new("git")
        .args([
            "-C",
            &root.display().to_string(),
            "status",
            "--porcelain=v1",
        ])
        .output()?;
    if !status.status.success() {
        return Err(error("git status 失败"));
    }
    if !Command::new("git")
        .args([
            "-C",
            &root.display().to_string(),
            "merge-base",
            "--is-ancestor",
            ledger.base_revision.as_str(),
            head.as_str(),
        ])
        .status()?
        .success()
    {
        return Err(error("HEAD 不是 Stage08 base 的 descendant"));
    }
    let metadata = Command::new("cargo")
        .args([
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
            &root.join("Cargo.toml").display().to_string(),
        ])
        .output()?;
    if !metadata.status.success()
        || !String::from_utf8_lossy(&metadata.stdout).contains("\"name\":\"xtask\"")
    {
        return Err(error("cargo metadata 未证明当前 worktree xtask provenance"));
    }
    check_contract(root)?;
    Ok(())
}

fn validate_host_evidence(evidence: &Value) -> ToolResult<()> {
    if u64_field(evidence, "schema_version")? != 2 {
        return Err(error("host evidence schema_version 必须为 2"));
    }
    validate_loopback_url(string_field(evidence, "base_url")?)?;
    validate_run_id(string_field(evidence, "run_id")?)?;
    validate_git_sha(string_field(evidence, "start_sha")?)?;
    if !bool_field(evidence, "real_host")?
        || u32_field(evidence, "pid")? == 0
        || !bool_field(evidence, "host_negative_checked")?
        || !bool_field(evidence, "origin_negative_checked")?
        || !bool_field(evidence, "origin_null_rejected")?
        || !bool_field(evidence, "csp_checked")?
        || !bool_field(evidence, "nosniff_checked")?
        || !bool_field(evidence, "referrer_policy_checked")?
        || !bool_field(evidence, "frame_deny_checked")?
        || !bool_field(evidence, "seed_recovered")?
        || string_field(evidence, "seed_title")? != "Seed release task"
    {
        return Err(error(
            "host evidence 必须是 live kanban serve 且覆盖 Host/Origin/CSP 负测",
        ));
    }
    for key in [
        "runtime",
        "manifest",
        "health_before_restart",
        "health_after_restart",
        "health_after_browser",
        "db_path_before_restart",
        "db_path_after_restart",
        "db_path_after_browser",
        "db_identity_before_restart",
        "db_identity_after_restart",
        "db_identity_after_browser",
        "db_fingerprint_before_restart",
        "db_fingerprint_after_restart",
        "db_fingerprint_after_browser",
        "migration_before",
        "start_after",
        "command_line",
        "argv",
        "binary_path",
        "binary_sha256",
        "live_binary_sha256",
    ] {
        if evidence.get(key).is_none() {
            return Err(error(format!("host evidence 缺少 {key}")));
        }
    }
    for (health_key, path_key, fingerprint_key) in [
        (
            "health_before_restart",
            "db_path_before_restart",
            "db_fingerprint_before_restart",
        ),
        (
            "health_after_restart",
            "db_path_after_restart",
            "db_fingerprint_after_restart",
        ),
        (
            "health_after_browser",
            "db_path_after_browser",
            "db_fingerprint_after_browser",
        ),
    ] {
        let health = object_field(evidence, health_key)?;
        if string_field(health, "db")? != "turso"
            || string_field(health, "db_path")? != string_field(evidence, path_key)?
            || string_field(health, "db_fingerprint")? != string_field(evidence, fingerprint_key)?
        {
            return Err(error(format!(
                "{health_key} 未与 top-level DB snapshot 对齐"
            )));
        }
    }
    validate_sha(string_field(evidence, "binary_sha256")?)?;
    validate_sha(string_field(evidence, "live_binary_sha256")?)?;
    Ok(())
}

fn validate_browser_evidence(
    evidence: &Value,
    ledger: &ReleaseLedger,
    host_base_url: &str,
    run_id: &str,
    chromium: &Value,
    firefox: &Value,
) -> ToolResult<Vec<TaskEvidence>> {
    if u64_field(evidence, "schema_version")? != 2 {
        return Err(error("browser evidence schema_version 必须为 2"));
    }
    validate_loopback_url(string_field(evidence, "base_url")?)?;
    if string_field(evidence, "base_url")? != host_base_url {
        return Err(error(
            "browser evidence base_url 未与 host evidence 完全一致",
        ));
    }
    let expected_flow_count = (ledger
        .rows
        .iter()
        .filter(|row| row.status == "ready")
        .count() as u64)
        .saturating_mul(2);
    let expected_project_flow_count = expected_flow_count / 2;
    if string_field(evidence, "run_id")? != run_id
        || !bool_field(evidence, "real_host")?
        || u64_field(evidence, "ready_flow_count")? != expected_flow_count
    {
        return Err(error(
            "browser evidence 未证明 ready ledger flows 使用 real host",
        ));
    }
    if string_field(evidence, "chromium")? != "passed"
        || string_field(evidence, "firefox")? != "passed"
    {
        return Err(error(
            "09A Chromium full/Firefox key browser evidence 未通过",
        ));
    }
    let ready_ids = ledger
        .rows
        .iter()
        .filter(|row| row.status == "ready")
        .map(|row| row.id.clone())
        .collect::<BTreeSet<_>>();
    for key in ["chromium_flow_ids", "firefox_flow_ids", "flow_ids"] {
        let values = string_array_field(evidence, key)?;
        let actual = values.iter().cloned().collect::<BTreeSet<_>>();
        if values.len() != actual.len() || actual != ready_ids {
            return Err(error(format!(
                "browser evidence {key} 必须精确覆盖 ready capability IDs",
            )));
        }
    }
    let ready_ids = ledger
        .rows
        .iter()
        .filter(|row| row.status == "ready")
        .map(|row| row.id.clone())
        .collect::<BTreeSet<_>>();
    let chromium_task = validate_project_browser_evidence(
        chromium,
        "chromium",
        run_id,
        host_base_url,
        expected_project_flow_count,
        &ready_ids,
    )?;
    let firefox_task = validate_project_browser_evidence(
        firefox,
        "firefox",
        run_id,
        host_base_url,
        expected_project_flow_count,
        &ready_ids,
    )?;
    if chromium_task.id == firefox_task.id {
        return Err(error("Chromium/Firefox created task IDs 必须 distinct"));
    }
    let expected_tasks = vec![chromium_task.clone(), firefox_task.clone()];
    let aggregate_tasks = task_arrays(evidence)?;
    if aggregate_tasks != expected_tasks {
        return Err(error(
            "aggregate browser task readback 与 per-browser evidence 不一致",
        ));
    }
    Ok(expected_tasks)
}

fn validate_project_browser_evidence(
    evidence: &Value,
    project: &str,
    run_id: &str,
    host_base_url: &str,
    expected_flow_count: u64,
    ready_ids: &BTreeSet<String>,
) -> ToolResult<TaskEvidence> {
    let status_key = if project == "chromium" {
        "chromium"
    } else {
        "firefox"
    };
    if u64_field(evidence, "schema_version")? != 2
        || string_field(evidence, "run_id")? != run_id
        || string_field(evidence, "browser")? != project
        || string_field(evidence, "base_url")? != host_base_url
        || !bool_field(evidence, "real_host")?
        || string_field(evidence, status_key)? != "passed"
        || u64_field(evidence, "ready_flow_count")? != expected_flow_count
        || evidence.get("failure") != Some(&Value::Null)
    {
        return Err(error(format!(
            "{project} browser evidence identity/status 不一致"
        )));
    }
    let flow_ids = string_array_field(evidence, "flow_ids")?;
    let actual_flow_ids = flow_ids.iter().cloned().collect::<BTreeSet<_>>();
    if flow_ids.len() != actual_flow_ids.len() || &actual_flow_ids != ready_ids {
        return Err(error(format!("{project} flow_ids 未精确覆盖 ready rows")));
    }
    let ids = string_array_field(evidence, "created_task_ids")?;
    let titles = string_array_field(evidence, "created_task_titles")?;
    let projects = string_array_field(evidence, "created_task_projects")?;
    let board_slugs = string_array_field(evidence, "created_task_board_slugs")?;
    if ids.len() != 1 || titles.len() != 1 || projects.len() != 1 || board_slugs.len() != 1 {
        return Err(error(format!(
            "{project} 必须恰好有一条 canonical task readback"
        )));
    }
    if projects[0] != project
        || board_slugs[0] != "default"
        || titles[0] != format!("Stage09 real task {project}")
        || validate_task_id(&ids[0]).is_err()
    {
        return Err(error(format!("{project} task readback metadata 不一致")));
    }
    Ok(TaskEvidence {
        browser: project.to_owned(),
        id: ids[0].clone(),
        title: titles[0].clone(),
        project: projects[0].clone(),
        board_slug: board_slugs[0].clone(),
    })
}

fn validate_task_id(value: &str) -> ToolResult<()> {
    if !value.starts_with("t_")
        || value.len() <= 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(error("task id 必须是 canonical t_ path segment"));
    }
    Ok(())
}
fn live_task_receipt(base_url: &str, task: &TaskEvidence) -> ToolResult<TaskReceipt> {
    let live_task = curl_json(&format!("{base_url}/api/v1/tasks/{}", task.id))?;
    let live_data = object_field(&live_task, "data")?;
    if string_field(live_data, "id")? != task.id
        || string_field(live_data, "title")? != task.title
        || string_field(live_data, "board_slug")? != task.board_slug
    {
        return Err(error(format!(
            "live task readback 与 {} evidence 不一致",
            task.id
        )));
    }
    Ok(TaskReceipt {
        browser: task.browser.clone(),
        id: task.id.clone(),
        title: task.title.clone(),
        project: task.project.clone(),
    })
}
fn live_seed_task(base_url: &str) -> ToolResult<TaskReceipt> {
    let seed = TaskEvidence {
        browser: "launcher".to_owned(),
        id: "t_release_seed".to_owned(),
        title: "Seed release task".to_owned(),
        project: "launcher".to_owned(),
        board_slug: "default".to_owned(),
    };
    live_task_receipt(base_url, &seed)
}

fn task_arrays(evidence: &Value) -> ToolResult<Vec<TaskEvidence>> {
    let ids = string_array_field(evidence, "created_task_ids")?;
    let titles = string_array_field(evidence, "created_task_titles")?;
    let projects = string_array_field(evidence, "created_task_projects")?;
    let board_slugs = string_array_field(evidence, "created_task_board_slugs")?;
    if ids.len() != titles.len() || ids.len() != projects.len() || ids.len() != board_slugs.len() {
        return Err(error("aggregate task readback arrays 长度不一致"));
    }
    ids.into_iter()
        .zip(titles)
        .zip(projects)
        .zip(board_slugs)
        .map(|(((id, title), project), board_slug)| {
            Ok(TaskEvidence {
                browser: project.clone(),
                id,
                title,
                project,
                board_slug,
            })
        })
        .collect()
}

fn markdown_capability_ids(markdown: &str) -> BTreeSet<String> {
    markdown
        .lines()
        .filter_map(|line| {
            let cells = line.split('|').collect::<Vec<_>>();
            let value = cells.get(1)?.trim();
            let value = value.strip_prefix('`')?.strip_suffix('`')?;
            (!value.is_empty() && value != "id").then(|| value.to_owned())
        })
        .collect()
}

fn read_regular_json(path: &Path) -> ToolResult<Value> {
    Ok(serde_json::from_slice(&read_regular_bytes(path)?)?)
}

fn read_regular_bytes(path: &Path) -> ToolResult<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(error(format!(
            "evidence 必须是 regular file: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    if std::os::unix::fs::MetadataExt::nlink(&metadata) != 1 {
        return Err(error("evidence 禁止 hardlink"));
    }
    Ok(fs::read(path)?)
}

fn atomic_write_json(path: &Path, value: &impl Serialize) -> ToolResult<()> {
    let parent = path.parent().ok_or_else(|| error("receipt 缺少 parent"))?;
    reject_symlink_chain(parent)?;
    fs::create_dir_all(parent)?;
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(error("receipt destination 必须是 regular file"));
        }
        #[cfg(unix)]
        if std::os::unix::fs::MetadataExt::nlink(&metadata) != 1 {
            return Err(error("receipt destination 禁止 hardlink"));
        }
    }
    let temporary = parent.join(format!(".release-proof.{}.tmp", std::process::id()));
    if temporary.exists() || temporary.is_symlink() {
        return Err(error("receipt temporary path 已存在"));
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let bytes = serde_json::to_vec_pretty(value)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    Ok(())
}

fn collect_evidence_hashes(root: &Path) -> ToolResult<BTreeMap<String, String>> {
    [
        "host.json",
        "browser.json",
        "browser-chromium.json",
        "browser-firefox.json",
    ]
    .into_iter()
    .map(|name| Ok((name.to_owned(), sha256_file(&root.join(name))?)))
    .collect()
}

fn curl_json(url: &str) -> ToolResult<Value> {
    let output = Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--max-time", "5", url])
        .output()?;
    if !output.status.success() {
        return Err(error(format!(
            "curl live host 失败: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[derive(Debug, Default)]
struct LiveProbe {
    status: u16,
    headers: BTreeMap<String, String>,
}

fn curl_probe(url: &str, request_header: Option<&str>) -> ToolResult<LiveProbe> {
    let mut command = Command::new("curl");
    command.args([
        "--silent",
        "--show-error",
        "--max-time",
        "5",
        "--dump-header",
        "-",
        "--output",
        "/dev/null",
        "--write-out",
        "\n%{http_code}",
    ]);
    if let Some(request_header) = request_header {
        command.args(["-H", request_header]);
    }
    command.arg(url);
    let output = command.output()?;
    if !output.status.success() {
        return Err(error(format!(
            "curl live host probe 失败: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let stdout = String::from_utf8(output.stdout)?;
    let (headers, status) = stdout
        .rsplit_once('\n')
        .ok_or_else(|| error("curl live host probe 缺少 status"))?;
    let status = status
        .trim()
        .parse::<u16>()
        .map_err(|_| error("curl live host probe status 无效"))?;
    let parsed_headers = headers
        .lines()
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        })
        .collect();
    Ok(LiveProbe {
        status,
        headers: parsed_headers,
    })
}

#[derive(Debug, Default)]
struct LiveSecurityChecks {
    host_negative_checked: bool,
    origin_negative_checked: bool,
    origin_null_rejected: bool,
    csp_checked: bool,
    nosniff_checked: bool,
    referrer_policy_checked: bool,
    frame_deny_checked: bool,
}

fn probe_live_security(base_url: &str) -> ToolResult<LiveSecurityChecks> {
    let app_url = format!("{base_url}/app/");
    let hostile_host = curl_probe(&app_url, Some("Host: evil.invalid"))?;
    let hostile_origin = curl_probe(&app_url, Some("Origin: https://evil.invalid"))?;
    let null_origin = curl_probe(&app_url, Some("Origin: null"))?;
    let normal = curl_probe(&app_url, None)?;
    let no_acao = |probe: &LiveProbe| !probe.headers.contains_key("access-control-allow-origin");
    let csp = normal
        .headers
        .get("content-security-policy")
        .map(String::as_str)
        .unwrap_or_default();
    Ok(LiveSecurityChecks {
        host_negative_checked: matches!(hostile_host.status, 400 | 403),
        origin_negative_checked: matches!(hostile_origin.status, 400 | 403)
            && no_acao(&hostile_origin),
        origin_null_rejected: matches!(null_origin.status, 400 | 403) && no_acao(&null_origin),
        csp_checked: normal.status == 200
            && csp.contains("default-src 'self'")
            && csp.contains("script-src 'self'"),
        nosniff_checked: normal.status == 200
            && normal
                .headers
                .get("x-content-type-options")
                .map(|value| value.eq_ignore_ascii_case("nosniff"))
                .unwrap_or(false),
        referrer_policy_checked: normal.status == 200
            && normal
                .headers
                .get("referrer-policy")
                .map(|value| value.eq_ignore_ascii_case("no-referrer"))
                .unwrap_or(false),
        frame_deny_checked: normal.status == 200
            && normal
                .headers
                .get("x-frame-options")
                .map(|value| value.eq_ignore_ascii_case("DENY"))
                .unwrap_or(false),
    })
}

fn process_argv(pid: u32) -> ToolResult<Vec<String>> {
    let bytes = fs::read(format!("/proc/{pid}/cmdline"))?;
    let mut argv = Vec::new();
    for token in bytes
        .split(|byte| *byte == 0)
        .filter(|token| !token.is_empty())
    {
        argv.push(String::from_utf8(token.to_vec())?);
    }
    if argv.is_empty() {
        return Err(error("live host /proc argv 为空"));
    }
    Ok(argv)
}
fn process_identity(pid: u32) -> ToolResult<String> {
    Ok(format!(
        "{}:{}",
        pid,
        fs::read_to_string(format!("/proc/{pid}/stat"))?
            .split_whitespace()
            .nth(21)
            .unwrap_or("unknown")
    ))
}
fn sha256_process_exe(pid: u32) -> ToolResult<String> {
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(fs::read(format!("/proc/{pid}/exe"))?)
    ))
}
fn validate_host_argv(
    argv: &[String],
    expected_port: u16,
    expected_db: &Path,
    expected_web_dir: &Path,
    expected_binary: &Path,
) -> ToolResult<bool> {
    let expected = vec![
        expected_binary.to_string_lossy().into_owned(),
        "--db".to_owned(),
        expected_db.to_string_lossy().into_owned(),
        "serve".to_owned(),
        "--host".to_owned(),
        "127.0.0.1".to_owned(),
        "--port".to_owned(),
        expected_port.to_string(),
        "--web-dir".to_owned(),
        expected_web_dir.to_string_lossy().into_owned(),
    ];
    if argv != expected {
        return Err(error(format!(
            "host PID argv 未精确匹配 launcher argv: expected={expected:?} actual={argv:?}"
        )));
    }
    Ok(true)
}

fn sha256_file(path: &Path) -> ToolResult<String> {
    let mut hash = Sha256::new();
    hash.update(read_regular_bytes(path)?);
    Ok(format!("sha256:{:x}", hash.finalize()))
}
fn git_revision(root: &Path) -> ToolResult<String> {
    let output = Command::new("git")
        .args(["-C", &root.display().to_string(), "rev-parse", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err(error("git rev-parse 失败"));
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
fn git_status_clean(root: &Path) -> ToolResult<bool> {
    Ok(Command::new("git")
        .args([
            "-C",
            &root.display().to_string(),
            "status",
            "--porcelain=v1",
        ])
        .output()?
        .stdout
        .is_empty())
}

fn lexical_root(path: &Path) -> ToolResult<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        env::current_dir()?.join(path)
    };
    lexical_absolute(&absolute)
}
fn resolve_root_relative(root: &Path, path: &Path) -> ToolResult<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    };
    lexical_absolute(&absolute)
}
fn validate_approved_evidence_root(root: &Path, path: &Path) -> ToolResult<()> {
    let default = lexical_absolute(&root.join(DEFAULT_EVIDENCE_PATH))?;
    if path != default && (!path.starts_with("/tmp/") || path == Path::new("/tmp")) {
        return Err(error(
            "evidence root 必须是 workspace output/release 或 launcher-owned /tmp directory",
        ));
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(error("custom evidence root 必须是 directory"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != current_uid()? || metadata.mode() & 0o777 != 0o700 {
            return Err(error("custom evidence root 必须由当前 uid 拥有且 mode=700"));
        }
    }
    Ok(())
}
fn lexical_absolute(path: &Path) -> ToolResult<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(error(format!(
            "path 必须是 absolute no-traversal: {}",
            path.display()
        )));
    }
    Ok(path.to_owned())
}
fn reject_symlink_chain(path: &Path) -> ToolResult<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if let Ok(metadata) = fs::symlink_metadata(&current)
            && metadata.file_type().is_symlink()
        {
            return Err(error(format!(
                "path chain 不得含 symlink: {}",
                current.display()
            )));
        }
    }
    Ok(())
}
fn validate_loopback_url(url: &str) -> ToolResult<()> {
    loopback_port(url)?;
    Ok(())
}
fn validate_private_db_path(path: &Path) -> ToolResult<()> {
    if path.file_name().and_then(|value| value.to_str()) != Some("canonical.db")
        || !path.starts_with("/tmp/")
    {
        return Err(error("canonical DB path 必须是 /tmp/*/canonical.db"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| error("canonical DB path 缺少 parent"))?;
    let parent_name = parent
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !parent_name.starts_with("kanban-release-09a.") {
        return Err(error(
            "canonical DB parent 必须是 launcher-owned mktemp directory",
        ));
    }
    let parent_metadata = fs::symlink_metadata(parent)?;
    if !parent_metadata.is_dir() {
        return Err(error("canonical DB parent 不是 directory"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if parent_metadata.mode() & 0o777 != 0o700 || parent_metadata.uid() != current_uid()? {
            return Err(error("canonical DB parent 必须由当前 uid 拥有且 mode=700"));
        }
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() {
        return Err(error("canonical DB path 不是 regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != current_uid()? || metadata.nlink() != 1 {
            return Err(error("canonical DB file 必须由当前 uid 拥有且 nlink=1"));
        }
    }
    Ok(())
}
fn db_identity(path: &Path) -> ToolResult<String> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!("{}:{}", metadata.dev(), metadata.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Err(error("DB dev:inode identity 仅支持 Unix"))
    }
}
fn validate_run_id(value: &str) -> ToolResult<()> {
    let nonce = value.strip_prefix("09a-").unwrap_or_default();
    if nonce.len() != 32 || !nonce.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(error("run_id 必须是 09a- 加 16-byte hex nonce"));
    }
    Ok(())
}
fn current_uid() -> ToolResult<u32> {
    let status = fs::read_to_string("/proc/self/status")?;
    let uid = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:")?.split_whitespace().next())
        .ok_or_else(|| error("无法读取当前 uid"))?
        .parse::<u32>()?;
    Ok(uid)
}
fn validate_sha(value: &str) -> ToolResult<()> {
    let digest = value.strip_prefix("sha256:").unwrap_or_default();
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(error("SHA 字段必须是 sha256:<64 hex>"));
    }
    Ok(())
}
fn validate_git_sha(value: &str) -> ToolResult<()> {
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(error("git SHA 必须是 40-hex revision"));
    }
    Ok(())
}
fn validate_db_identity_triplet(
    before_restart: &str,
    after_restart: &str,
    after_browser: &str,
    live: &str,
) -> ToolResult<()> {
    if before_restart.is_empty()
        || before_restart != after_restart
        || after_restart != after_browser
        || after_browser != live
    {
        return Err(error(
            "canonical DB dev:inode identity changed across release proof",
        ));
    }
    Ok(())
}
fn validate_start_binding(start_sha: &str, current_sha: &str, start_clean: bool) -> ToolResult<()> {
    if !start_clean || start_sha != current_sha {
        return Err(error("release receipt requires clean unchanged START_SHA"));
    }
    Ok(())
}
fn loopback_port(url: &str) -> ToolResult<u16> {
    let authority = url
        .strip_prefix("http://")
        .ok_or_else(|| error("release host 必须 loopback HTTP"))?;
    if authority.contains('/') || authority.contains('?') || authority.contains('#') {
        return Err(error("release host URL 不得含 path/query/fragment"));
    }
    let socket = authority
        .parse::<std::net::SocketAddr>()
        .map_err(|_| error("release host URL 必须包含显式 loopback IPv4 port"))?;
    if !socket.ip().is_loopback() || !socket.ip().is_ipv4() || socket.port() == 0 {
        return Err(error("release host 必须是非零 IPv4 loopback port"));
    }
    Ok(socket.port())
}

fn object_field<'a>(value: &'a Value, key: &str) -> ToolResult<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| error(format!("缺少字段 {key}")))
}
fn string_field<'a>(value: &'a Value, key: &str) -> ToolResult<&'a str> {
    object_field(value, key)?
        .as_str()
        .ok_or_else(|| error(format!("字段 {key} 必须 string")))
}
fn string_array_field(value: &Value, key: &str) -> ToolResult<Vec<String>> {
    object_field(value, key)?
        .as_array()
        .ok_or_else(|| error(format!("字段 {key} 必须 string array")))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| error(format!("字段 {key} 必须 string array")))
        })
        .collect()
}
fn bool_field(value: &Value, key: &str) -> ToolResult<bool> {
    object_field(value, key)?
        .as_bool()
        .ok_or_else(|| error(format!("字段 {key} 必须 bool")))
}
fn i64_field(value: &Value, key: &str) -> ToolResult<i64> {
    object_field(value, key)?
        .as_i64()
        .ok_or_else(|| error(format!("字段 {key} 必须 integer")))
}
fn u32_field(value: &Value, key: &str) -> ToolResult<u32> {
    object_field(value, key)?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| error(format!("字段 {key} 必须 u32")))
}
fn u64_field(value: &Value, key: &str) -> ToolResult<u64> {
    object_field(value, key)?
        .as_u64()
        .ok_or_else(|| error(format!("字段 {key} 必须 u64")))
}
fn error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    std::io::Error::other(message.into()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger_with_ready_row() -> ReleaseLedger {
        ReleaseLedger {
            schema_version: 1,
            stage: "stage09-release-proof-09A".to_owned(),
            base_revision: BASE_REVISION.to_owned(),
            rows: vec![ReleaseLedgerRow {
                id: "shell.runtime".to_owned(),
                status: "ready".to_owned(),
                flow: Some("apps/web/tests/release-proof.spec.ts#shell.runtime".to_owned()),
                chromium: "full".to_owned(),
                firefox: "key".to_owned(),
                requires_real_host: true,
            }],
            gates: ReleaseGates {
                real_host: "ready".to_owned(),
                axe: "pending".to_owned(),
                visual: "pending".to_owned(),
                performance: "pending".to_owned(),
                functional_2k: "pending".to_owned(),
                stress_5k: "pending".to_owned(),
                package: "pending".to_owned(),
            },
        }
    }

    #[test]
    fn loopback_parser_rejects_non_loopback_and_path() {
        assert!(validate_loopback_url("http://127.0.0.1:18721").is_ok());
        assert!(validate_loopback_url("http://127.0.0.1:18721/app/").is_err());
        assert!(validate_loopback_url("https://127.0.0.1:18721").is_err());
        assert!(validate_loopback_url("http://192.0.2.1:18721").is_err());
    }

    #[test]
    fn browser_evidence_rejects_forged_flow_count() {
        let ledger = ledger_with_ready_row();
        let evidence = aggregate_evidence(0);
        assert!(
            validate_browser_evidence(
                &evidence,
                &ledger,
                "http://127.0.0.1:18721",
                "09a-00000000000000000000000000000000",
                &project_evidence("chromium"),
                &project_evidence("firefox"),
            )
            .is_err()
        );
    }

    #[test]
    fn browser_evidence_rejects_cross_browser_count() {
        let ledger = ledger_with_ready_row();
        let evidence = aggregate_evidence(4);
        assert!(
            validate_browser_evidence(
                &evidence,
                &ledger,
                "http://127.0.0.1:18721",
                "09a-00000000000000000000000000000000",
                &project_evidence("chromium"),
                &project_evidence("firefox"),
            )
            .is_err()
        );
    }

    #[test]
    fn browser_evidence_accepts_explicit_ready_flow() {
        let ledger = ledger_with_ready_row();
        let evidence = aggregate_evidence(2);
        assert!(
            validate_browser_evidence(
                &evidence,
                &ledger,
                "http://127.0.0.1:18721",
                "09a-00000000000000000000000000000000",
                &project_evidence("chromium"),
                &project_evidence("firefox"),
            )
            .is_ok()
        );
    }

    fn aggregate_evidence(ready_flow_count: u64) -> Value {
        serde_json::json!({
            "schema_version": 2,
            "run_id": "09a-00000000000000000000000000000000",
            "real_host": true,
            "base_url": "http://127.0.0.1:18721",
            "ready_flow_count": ready_flow_count,
            "chromium": "passed",
            "firefox": "passed",
            "chromium_flow_ids": ["shell.runtime"],
            "firefox_flow_ids": ["shell.runtime"],
            "flow_ids": ["shell.runtime"],
            "created_task_ids": ["t_chromium", "t_firefox"],
            "created_task_titles": ["Stage09 real task chromium", "Stage09 real task firefox"],
            "created_task_projects": ["chromium", "firefox"],
            "created_task_board_slugs": ["default", "default"]
        })
    }

    fn project_evidence(project: &str) -> Value {
        serde_json::json!({
            "schema_version": 2,
            "run_id": "09a-00000000000000000000000000000000",
            "real_host": true,
            "base_url": "http://127.0.0.1:18721",
            "browser": project,
            "chromium": if project == "chromium" { "passed" } else { "not-applicable" },
            "firefox": if project == "firefox" { "passed" } else { "not-applicable" },
            "ready_flow_count": 1,
            "flow_ids": ["shell.runtime"],
            "failure": null,
            "created_task_ids": [format!("t_{project}")],
            "created_task_titles": [format!("Stage09 real task {project}")],
            "created_task_projects": [project],
            "created_task_board_slugs": ["default"]
        })
    }

    #[test]
    fn browser_evidence_rejects_forged_run_id_and_task_metadata() {
        let ledger = ledger_with_ready_row();
        let mut forged_run = project_evidence("firefox");
        forged_run["run_id"] = Value::String("09a-ffffffffffffffffffffffffffffffff".to_owned());
        assert!(
            validate_browser_evidence(
                &aggregate_evidence(2),
                &ledger,
                "http://127.0.0.1:18721",
                "09a-00000000000000000000000000000000",
                &project_evidence("chromium"),
                &forged_run,
            )
            .is_err()
        );

        let mut forged_task = aggregate_evidence(2);
        forged_task["created_task_titles"][1] = Value::String("forged title".to_owned());
        assert!(
            validate_browser_evidence(
                &forged_task,
                &ledger,
                "http://127.0.0.1:18721",
                "09a-00000000000000000000000000000000",
                &project_evidence("chromium"),
                &project_evidence("firefox"),
            )
            .is_err()
        );
    }

    #[test]
    fn host_evidence_requires_measured_host_negative_check() {
        let mut evidence = serde_json::json!({
            "schema_version": 2,
            "run_id": "09a-00000000000000000000000000000000",
            "start_sha": "0000000000000000000000000000000000000000",
            "start_clean": true,
            "real_host": true,
            "base_url": "http://127.0.0.1:18721",
            "pid": 18721,
            "start_after": "1",
            "command_line": "kanban serve",
            "argv": ["kanban", "serve"],
            "host_negative_checked": false,
            "origin_negative_checked": true,
            "origin_null_rejected": true,
            "csp_checked": true,
            "nosniff_checked": true,
            "referrer_policy_checked": true,
            "frame_deny_checked": true,
            "seed_recovered": true,
            "seed_title": "Seed release task",
            "health_before_restart": {"db":"turso","db_path":"/tmp/kanban-release-09a.test/canonical.db","db_fingerprint":"turso:before"},
            "health_after_restart": {"db":"turso","db_path":"/tmp/kanban-release-09a.test/canonical.db","db_fingerprint":"turso:after"},
            "health_after_browser": {"db":"turso","db_path":"/tmp/kanban-release-09a.test/canonical.db","db_fingerprint":"turso:browser"},
            "runtime": {},
            "manifest": {},
            "db_path_before_restart": "/tmp/kanban-release-09a.test/canonical.db",
            "db_path_after_restart": "/tmp/kanban-release-09a.test/canonical.db",
            "db_path_after_browser": "/tmp/kanban-release-09a.test/canonical.db",
            "db_identity_before_restart": "1:1",
            "db_identity_after_restart": "1:1",
            "db_identity_after_browser": "1:1",
            "db_fingerprint_before_restart": "turso:before",
            "db_fingerprint_after_restart": "turso:after",
            "db_fingerprint_after_browser": "turso:browser",
            "migration_before": 1,
            "binary_path": "/tmp/kanban",
            "binary_sha256": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "live_binary_sha256": "sha256:0000000000000000000000000000000000000000000000000000000000000000"
        });
        assert!(validate_host_evidence(&evidence).is_err());
        evidence["host_negative_checked"] = Value::Bool(true);
        assert!(validate_host_evidence(&evidence).is_ok());
    }

    #[test]
    fn db_identity_mismatch_is_rejected() {
        assert!(validate_db_identity_triplet("1:1", "1:1", "1:2", "1:1").is_err());
        assert!(validate_db_identity_triplet("1:1", "1:1", "1:1", "1:1").is_ok());
    }

    #[test]
    fn start_sha_mismatch_is_rejected() {
        assert!(validate_start_binding("sha256:a", "sha256:b", true).is_err());
        assert!(validate_start_binding("sha256:a", "sha256:a", false).is_err());
        assert!(validate_start_binding("sha256:a", "sha256:a", true).is_ok());
    }

    #[test]
    fn host_argv_requires_exact_launcher_tokens() {
        let binary = Path::new("/tmp/kanban");
        let db = Path::new("/tmp/kanban-release-09a.test/canonical.db");
        let web_dir = Path::new("/workspace/apps/web/dist");
        let exact = vec![
            binary.display().to_string(),
            "--db".to_owned(),
            db.display().to_string(),
            "serve".to_owned(),
            "--host".to_owned(),
            "127.0.0.1".to_owned(),
            "--port".to_owned(),
            "18721".to_owned(),
            "--web-dir".to_owned(),
            web_dir.display().to_string(),
        ];
        assert!(validate_host_argv(&exact, 18721, db, web_dir, binary).is_ok());

        let mut extra = exact.clone();
        extra.push("--no-web".to_owned());
        assert!(validate_host_argv(&extra, 18721, db, web_dir, binary).is_err());

        let mut reordered = exact;
        reordered.swap(1, 3);
        assert!(validate_host_argv(&reordered, 18721, db, web_dir, binary).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn evidence_reader_rejects_symlink() {
        let root = env::temp_dir().join(format!("release-proof-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("test root");
        let target = root.join("target.json");
        fs::write(&target, b"{}").expect("target");
        std::os::unix::fs::symlink(&target, root.join("link.json")).expect("link");
        assert!(read_regular_bytes(&root.join("link.json")).is_err());
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn atomic_receipt_writes_newline_and_replaces_regular_file() {
        let root = env::temp_dir().join(format!("release-proof-atomic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("test root");
        let output = root.join("receipt.json");
        atomic_write_json(&output, &serde_json::json!({"status":"pending"})).expect("first write");
        atomic_write_json(&output, &serde_json::json!({"status":"ready"})).expect("replace");
        let content = fs::read_to_string(&output).expect("receipt");
        assert!(content.ends_with('\n'));
        assert!(content.contains("ready"));
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn atomic_receipt_rejects_symlink_destination() {
        let root =
            env::temp_dir().join(format!("release-proof-destination-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("test root");
        let target = root.join("target.json");
        let output = root.join("receipt.json");
        fs::write(&target, b"target").expect("target");
        std::os::unix::fs::symlink(&target, &output).expect("link");
        assert!(atomic_write_json(&output, &serde_json::json!({"status":"ready"})).is_err());
        fs::remove_dir_all(&root).expect("cleanup");
    }
}
