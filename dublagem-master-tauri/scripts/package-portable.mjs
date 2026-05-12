import { cp, mkdir, readdir, rm, stat } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, "..");
const releaseDir = path.join(projectRoot, "src-tauri", "target", "release");
const portableRoot = path.join(projectRoot, "dist-portable");
const portableAppDir = path.join(portableRoot, "NSG Gaming Dub 1.0");
const executableName = "dublagem-master-tauri.exe";
const executableSource = path.join(releaseDir, executableName);
const executableTarget = path.join(portableAppDir, executableName);
const modelSourceCandidates = [path.join(releaseDir, "models"), path.join(projectRoot, "models")];
const modelTarget = path.join(portableAppDir, "models");

await assertFile(executableSource, "release executable");
const modelSource = await firstExistingDirectory(modelSourceCandidates);

await mkdir(portableAppDir, { recursive: true });
await rm(executableTarget, { force: true });
await cp(executableSource, executableTarget);
await cp(modelSource, modelTarget, { recursive: true, force: true });

const modelBytes = await directorySize(modelTarget);
process.stdout.write(`Portable distribution ready: ${portableAppDir}\n`);
process.stdout.write(`Bundled model payload: ${formatGiB(modelBytes)} GiB\n`);

async function firstExistingDirectory(candidates) {
  for (const candidate of candidates) {
    if (await isDirectory(candidate)) {
      return candidate;
    }
  }

  throw new Error(`No model directory found. Checked: ${candidates.join(", ")}`);
}

async function assertFile(filePath, label) {
  const metadata = await stat(filePath).catch(() => null);
  if (!metadata?.isFile()) {
    throw new Error(`Missing ${label}: ${filePath}`);
  }
}

async function isDirectory(directoryPath) {
  const metadata = await stat(directoryPath).catch(() => null);
  return metadata?.isDirectory() ?? false;
}

async function directorySize(directoryPath) {
  const entries = await readdir(directoryPath, { withFileTypes: true });
  let totalBytes = 0;

  for (const entry of entries) {
    const entryPath = path.join(directoryPath, entry.name);
    if (entry.isDirectory()) {
      totalBytes += await directorySize(entryPath);
      continue;
    }
    if (entry.isFile()) {
      totalBytes += (await stat(entryPath)).size;
    }
  }

  return totalBytes;
}

function formatGiB(bytes) {
  return (bytes / 1024 ** 3).toFixed(2);
}
