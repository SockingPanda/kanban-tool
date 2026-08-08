import { createHash } from "node:crypto"
import { access, mkdtemp, mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join, relative, sep } from "node:path"
import { fileURLToPath } from "node:url"

import { build, loadConfigFromFile } from "vite"
import { describe, expect, test } from "vitest"

import {
  WEB_ARTIFACT_BASE_PATH,
  WEB_ARTIFACT_ENTRYPOINT,
  WEB_ARTIFACT_FORMAT_VERSION,
  WEB_ARTIFACT_MANIFEST_PATH,
  WEB_PROTOCOL_VERSION,
  buildWebArtifactManifest,
  createWebArtifactManifestPlugin,
  validateWebArtifactManifest,
  webArtifactBuildIdFor,
  webArtifactBuildPreimage,
} from "./artifact-manifest"
import type { WebArtifactFile, WebArtifactManifest, WebArtifactOutput } from "./artifact-manifest"

const SHA_A = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
const SHA_B = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

function file(path: string, bytes: number, sha256: string): WebArtifactFile {
  return { path, bytes, sha256 }
}

function sampleFiles(): WebArtifactFile[] {
  return [
    file("assets/app.js", 3, SHA_A),
    file("index.html", 2, SHA_B),
  ]
}

describe("Web artifact manifest contract", () => {
  test("matches the Rust known vector byte-for-byte", () => {
    const files = sampleFiles()
    const preimage = webArtifactBuildPreimage({
      formatVersion: WEB_ARTIFACT_FORMAT_VERSION,
      basePath: WEB_ARTIFACT_BASE_PATH,
      entrypoint: WEB_ARTIFACT_ENTRYPOINT,
      serverVersion: "3.0.0",
      protocolVersion: WEB_PROTOCOL_VERSION,
      files,
    })

    expect(preimage.toString("hex")).toBe(
      "00000000000000246b616e62616e2d746f6f6c3a7765622d61727469666163742d6275696c642d69643a7631000000000000000100000000000000052f6170702f000000000000000a696e6465782e68746d6c0000000000000005332e302e30000000000000000276310000000000000002000000000000000d6173736574732f6170702e6a730000000000000003aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa000000000000000a696e6465782e68746d6c0000000000000002bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    )
    expect(
      webArtifactBuildIdFor({
        formatVersion: WEB_ARTIFACT_FORMAT_VERSION,
        basePath: WEB_ARTIFACT_BASE_PATH,
        entrypoint: WEB_ARTIFACT_ENTRYPOINT,
        serverVersion: "3.0.0",
        protocolVersion: WEB_PROTOCOL_VERSION,
        files,
      }),
    ).toBe("sha256:ce7b387aff6a614f4e376260a8edbd1341148d932df90db96dd00bce038f44a7")
  })

  test("sorts inventory by UTF-8 bytes and emits the fixed manifest shape", () => {
    const outputs: WebArtifactOutput[] = [
      { path: "index.html", source: Buffer.from("<html />") },
      { path: "assets/z.js", source: Buffer.from("z") },
      { path: "assets/a.js", source: Buffer.from("a") },
    ]

    const manifest = buildWebArtifactManifest({
      serverVersion: "3.0.0",
      outputs,
    })

    expect(manifest).toMatchObject({
      formatVersion: 1,
      basePath: "/app/",
      entrypoint: "index.html",
      serverVersion: "3.0.0",
      protocolVersion: "v1",
    })
    expect(manifest.files.map(({ path }) => path)).toEqual([
      "assets/a.js",
      "assets/z.js",
      "index.html",
    ])
    expect(manifest.files).toEqual([
      {
        path: "assets/a.js",
        bytes: 1,
        sha256: "sha256:ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb",
      },
      {
        path: "assets/z.js",
        bytes: 1,
        sha256: "sha256:594e519ae499312b29433b7dd8a97ff068defcba9755b6d5d00e84c524d67b06",
      },
      {
        path: "index.html",
        bytes: 8,
        sha256: "sha256:976c91556d9c07b0d6c8da7292df4661d9357b1ee5840acba40c31a89d7916eb",
      },
    ])
    expect(manifest.buildId).toMatch(/^sha256:[0-9a-f]{64}$/)
    expect(manifest).not.toHaveProperty("manifest")
  })

  test.each([
    "",
    "/absolute.js",
    "assets\\bundle.js",
    "assets/../secret.js",
    "assets/./bundle.js",
    "assets/%2e%2e/secret.js",
    "assets/app bundle.js",
    "assets/雪.js",
    WEB_ARTIFACT_MANIFEST_PATH,
  ])("rejects unsafe output path %s", (path) => {
    expect(() =>
      buildWebArtifactManifest({
        serverVersion: "3.0.0",
        outputs: [
          { path: "index.html", source: "index" },
          { path, source: "bad" },
        ],
      }),
    ).toThrow(/path|路径|entrypoint/i)
  })

  test("rejects duplicate paths and missing index entrypoint", () => {
    expect(() =>
      buildWebArtifactManifest({
        serverVersion: "3.0.0",
        outputs: [
          { path: "index.html", source: "index" },
          { path: "index.html", source: "duplicate" },
        ],
      }),
    ).toThrow(/duplicate|重复/i)

    expect(() =>
      buildWebArtifactManifest({
        serverVersion: "3.0.0",
        outputs: [{ path: "assets/app.js", source: "app" }],
      }),
    ).toThrow(/index|entrypoint/i)
  })

  test("Vite plugin prevalidates the final bundle without a stale intermediate manifest", () => {
    const emitted: Array<{ fileName: string; source: string }> = []
    const plugin = createWebArtifactManifestPlugin({ serverVersion: "3.0.0" })
    const generateBundle = plugin.generateBundle
    expect(generateBundle).toBeTypeOf("function")
    if (!generateBundle) throw new Error("manifest plugin has no generateBundle hook")

    generateBundle.call(
      {
        emitFile(file: { fileName: string; source: string }) {
          emitted.push(file)
        },
      } as never,
      {},
      {
        "index.html": {
          type: "asset",
          fileName: "index.html",
          source: "index",
        },
        "assets/app.js": {
          type: "asset",
          fileName: "assets/app.js",
          source: new Uint8Array([1, 2, 3]),
        },
      },
    )

    expect(emitted).toHaveLength(0)
  })

  test("writeBundle rewrites the manifest from final on-disk bytes", async () => {
    const outputDirectory = await mkdtemp(join(tmpdir(), "kanban-web-artifact-"))
    try {
      await mkdir(join(outputDirectory, "assets"))
      await writeFile(join(outputDirectory, "index.html"), "index", "utf8")
      await writeFile(join(outputDirectory, "assets", "app.js"), new Uint8Array([1, 2, 3]))
      await writeFile(join(outputDirectory, WEB_ARTIFACT_MANIFEST_PATH), '{"stale":true}', "utf8")

      const plugin = createWebArtifactManifestPlugin({ serverVersion: "3.0.0" })
      const writeBundle = plugin.writeBundle
      expect(writeBundle).toBeTypeOf("function")
      if (!writeBundle) throw new Error("manifest plugin has no writeBundle hook")
      await writeBundle.call({} as never, { dir: outputDirectory } as never, {} as never)

      const manifest = JSON.parse(
        await readFile(join(outputDirectory, WEB_ARTIFACT_MANIFEST_PATH), "utf8"),
      ) as WebArtifactManifest
      validateWebArtifactManifest(manifest)
      expect(manifest.files.map(({ path }) => path)).toEqual(["assets/app.js", "index.html"])
      expect(manifest.files).toEqual([
        {
          path: "assets/app.js",
          bytes: 3,
          sha256: "sha256:039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81",
        },
        {
          path: "index.html",
          bytes: 5,
          sha256: "sha256:1bc04b5291c26a46d918139138b992d2de976d6851d0893b0476b85bfbdfc6e6",
        },
      ])
      expect(manifest.buildId).toMatch(/^sha256:[0-9a-f]{64}$/)
    } finally {
      await rm(outputDirectory, { recursive: true, force: true })
    }
  })

  test("real Vite builds are reproducible and manifest inventory matches final disk bytes", async () => {
    const webRoot = fileURLToPath(new URL("..", import.meta.url))
    const configPath = fileURLToPath(new URL("../vite.config.ts", import.meta.url))
    const loaded = await loadConfigFromFile({ command: "build", mode: "production" }, configPath)
    expect(loaded?.config).toBeDefined()
    if (!loaded?.config) throw new Error("Vite config did not load")

    const outputDirectory = await mkdtemp(join(tmpdir(), "kanban-web-real-build-"))
    try {
      await writeFile(join(outputDirectory, "stale.txt"), "stale", "utf8")
      await writeFile(join(outputDirectory, WEB_ARTIFACT_MANIFEST_PATH), '{"stale":true}', "utf8")

      const runBuild = async () => {
        await build({
          ...loaded.config,
          root: webRoot,
          configFile: false,
          logLevel: "silent",
          build: {
            ...(loaded.config.build ?? {}),
            emptyOutDir: true,
            outDir: outputDirectory,
          },
        })
      }

      await runBuild()
      const firstManifestText = await readFile(join(outputDirectory, WEB_ARTIFACT_MANIFEST_PATH), "utf8")
      const firstManifest = JSON.parse(firstManifestText) as WebArtifactManifest
      validateWebArtifactManifest(firstManifest)
      const firstInventory = await inventoryFromDisk(outputDirectory)
      expect(firstManifest.files).toEqual(firstInventory)
      expect(firstManifest.files.map(({ path }) => path)).toEqual(firstInventory.map(({ path }) => path))
      expect(firstManifest.buildId).toMatch(/^sha256:[0-9a-f]{64}$/)
      await expectPathMissing(join(outputDirectory, "stale.txt"))

      await writeFile(join(outputDirectory, "stale.txt"), "stale again", "utf8")
      await runBuild()
      const secondManifestText = await readFile(join(outputDirectory, WEB_ARTIFACT_MANIFEST_PATH), "utf8")
      expect(secondManifestText).toBe(firstManifestText)
      const secondManifest = JSON.parse(secondManifestText) as WebArtifactManifest
      validateWebArtifactManifest(secondManifest)
      expect(await inventoryFromDisk(outputDirectory)).toEqual(firstInventory)
      await expectPathMissing(join(outputDirectory, "stale.txt"))
    } finally {
      await rm(outputDirectory, { recursive: true, force: true })
    }
  })
})

async function inventoryFromDisk(root: string): Promise<WebArtifactFile[]> {
  const files: WebArtifactFile[] = []
  async function visit(directory: string): Promise<void> {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const absolute = join(directory, entry.name)
      if (entry.name === WEB_ARTIFACT_MANIFEST_PATH && directory === root) continue
      if (entry.isDirectory()) {
        await visit(absolute)
      } else {
        expect(entry.isFile(), `unsupported output entry ${absolute}`).toBe(true)
        const bytes = await readFile(absolute)
        files.push({
          path: relative(root, absolute).split(sep).join("/"),
          bytes: bytes.byteLength,
          sha256: `sha256:${createHash("sha256").update(bytes).digest("hex")}`,
        })
      }
    }
  }
  await visit(root)
  files.sort((left, right) => Buffer.compare(Buffer.from(left.path), Buffer.from(right.path)))
  return files
}

async function expectPathMissing(path: string): Promise<void> {
  try {
    await access(path)
    throw new Error(`expected path to be absent: ${path}`)
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return
    throw error
  }
}
