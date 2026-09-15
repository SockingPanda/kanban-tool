import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const packageRoot = path.dirname(fileURLToPath(import.meta.url));
const protoRoot = path.resolve(packageRoot, "../../../proto");
const generatedRoot = path.join(packageRoot, "src/gen");
const plugin = path.join(packageRoot, "node_modules/.bin/protoc-gen-es");

fs.mkdirSync(generatedRoot, { recursive: true });
// 与 Rust 生成链共用仓库根 proto；migration/proto 仅保留附件离线夹具。
const result = spawnSync(process.env.PROTOC ?? "protoc", [
  `--proto_path=${protoRoot}`,
  `--plugin=protoc-gen-es=${plugin}`,
  `--es_out=${generatedRoot}`,
  "--es_opt=target=ts",
  path.join(protoRoot, "kanban/framework/v1/board.proto"),
], { cwd: packageRoot, stdio: "inherit" });

if (result.error) console.error(result.error.message);
process.exitCode = result.status ?? 1;
