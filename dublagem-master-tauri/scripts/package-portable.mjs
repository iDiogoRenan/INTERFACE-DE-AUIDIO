import { cp, mkdir, readdir, readFile, rm, stat } from "node:fs/promises";
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
const cargoConfigPath = path.join(projectRoot, ".cargo", "config.toml");
const cudaRuntimeDllPatterns = [
  /^cublas64_\d+\.dll$/i,
  /^cublasLt64_\d+\.dll$/i,
  /^cudart64_\d+\.dll$/i,
  /^curand64_\d+\.dll$/i,
  /^nvrtc64_\d+_\d+\.dll$/i,
  /^nvrtc-builtins64_\d+\.dll$/i,
  /^nvJitLink_\d+_\d+\.dll$/i,
  /^nvfatbin_\d+_\d+\.dll$/i
];

await assertFile(executableSource, "release executable");
const modelSource = await firstExistingDirectory(modelSourceCandidates);

await rm(portableAppDir, { force: true, recursive: true });
await mkdir(portableAppDir, { recursive: true });
await cp(executableSource, executableTarget);
await cp(modelSource, modelTarget, { recursive: true, force: true });
const cudaRuntimeFiles = await copyCudaRuntimeDependencies(portableAppDir);

const modelBytes = await directorySize(modelTarget);
const cudaRuntimeBytes = await filesSize(cudaRuntimeFiles);
process.stdout.write(`Portable distribution ready: ${portableAppDir}\n`);
process.stdout.write(`Bundled model payload: ${formatGiB(modelBytes)} GiB\n`);
process.stdout.write(
  `Bundled CUDA runtime: ${cudaRuntimeFiles.length} DLLs, ${formatGiB(cudaRuntimeBytes)} GiB\n`
);

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

async function filesSize(filePaths) {
  let totalBytes = 0;

  for (const filePath of filePaths) {
    totalBytes += (await stat(filePath)).size;
  }

  return totalBytes;
}

async function copyCudaRuntimeDependencies(targetDirectory) {
  const cudaRoot = await resolveCudaRoot();
  const cudaBinDirectories = [path.join(cudaRoot, "bin", "x64"), path.join(cudaRoot, "bin")];
  const sourceDirectory = await firstExistingDirectory(cudaBinDirectories);
  const sourceEntries = await readdir(sourceDirectory, { withFileTypes: true });
  const dlls = sourceEntries
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name)
    .filter((name) => cudaRuntimeDllPatterns.some((pattern) => pattern.test(name)))
    .sort((left, right) => left.localeCompare(right));

  assertCudaRuntimeCoverage(dlls);

  const copiedFiles = [];
  for (const dll of dlls) {
    const source = path.join(sourceDirectory, dll);
    const target = path.join(targetDirectory, dll);
    await rm(target, { force: true });
    await cp(source, target);
    copiedFiles.push(target);
  }

  await copyCudaLicense(cudaRoot, targetDirectory);

  return copiedFiles;
}

async function resolveCudaRoot() {
  if (process.env.CUDA_PATH && (await isDirectory(process.env.CUDA_PATH))) {
    return process.env.CUDA_PATH;
  }

  const cargoConfig = await readFile(cargoConfigPath, "utf8").catch(() => "");
  const configuredPath = /^\s*CUDA_PATH\s*=\s*"([^"]+)"/m.exec(cargoConfig)?.[1];
  if (configuredPath && (await isDirectory(configuredPath))) {
    return configuredPath;
  }

  throw new Error(`CUDA Toolkit not found. Set CUDA_PATH or configure it in ${cargoConfigPath}`);
}

function assertCudaRuntimeCoverage(dlls) {
  const requiredFamilies = ["cublas64_", "cublasLt64_", "cudart64_", "curand64_"];
  const missingFamilies = requiredFamilies.filter(
    (family) => !dlls.some((dll) => dll.toLowerCase().startsWith(family.toLowerCase()))
  );

  if (missingFamilies.length > 0) {
    throw new Error(`Missing CUDA runtime DLLs: ${missingFamilies.join(", ")}`);
  }
}

async function copyCudaLicense(cudaRoot, targetDirectory) {
  for (const licenseFile of ["EULA.txt", "LICENSE"]) {
    const source = path.join(cudaRoot, licenseFile);
    if (await isFile(source)) {
      await cp(source, path.join(targetDirectory, `CUDA_${licenseFile}`));
    }
  }
}

async function isFile(filePath) {
  const metadata = await stat(filePath).catch(() => null);
  return metadata?.isFile() ?? false;
}

function formatGiB(bytes) {
  return (bytes / 1024 ** 3).toFixed(2);
}
