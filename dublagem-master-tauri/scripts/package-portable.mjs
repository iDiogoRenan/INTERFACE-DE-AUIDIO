import { createWriteStream } from "node:fs";
import { cp, mkdir, readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import yazl from "yazl";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, "..");
const releaseDir = path.join(projectRoot, "src-tauri", "target", "release");
const portableRoot = path.join(projectRoot, "dist-portable");
const portableAppDir = path.join(portableRoot, "NSG Gaming Dub 1.0");
const portableArchive = path.join(portableRoot, "NSG-Gaming-Dub-1.0-portable.zip");
const executableName = "dublagem-master-tauri.exe";
const executableSource = path.join(releaseDir, executableName);
const executableTarget = path.join(portableAppDir, executableName);
const modelSourceCandidates = [path.join(releaseDir, "models"), path.join(projectRoot, "models")];
const modelTarget = path.join(portableAppDir, "models");
const cargoConfigPath = path.join(projectRoot, ".cargo", "config.toml");
const reproducibleZipDate = new Date("1980-01-01T00:00:00.000Z");
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
await writeDistributionInstructions(portableAppDir);
await createPortableArchive(portableAppDir, portableArchive);

const modelBytes = await directorySize(modelTarget);
const cudaRuntimeBytes = await filesSize(cudaRuntimeFiles);
const archiveBytes = (await stat(portableArchive)).size;
process.stdout.write(`Portable distribution ready: ${portableAppDir}\n`);
process.stdout.write(`Portable archive ready: ${portableArchive}\n`);
process.stdout.write(`Bundled model payload: ${formatGiB(modelBytes)} GiB\n`);
process.stdout.write(
  `Bundled CUDA runtime: ${cudaRuntimeFiles.length} DLLs, ${formatGiB(cudaRuntimeBytes)} GiB\n`
);
process.stdout.write(`Portable archive size: ${formatGiB(archiveBytes)} GiB\n`);

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

async function writeDistributionInstructions(targetDirectory) {
  const instructions = [
    "NSG Gaming Dub 1.0 - distribuicao portatil",
    "",
    "Extraia esta pasta inteira antes de abrir dublagem-master-tauri.exe.",
    "Nao execute o .exe diretamente de dentro do .zip.",
    "Primeira instalacao: envie a pasta inteira, porque os DLLs CUDA e a pasta models devem ficar ao lado do .exe.",
    "Atualizacao/hotfix: se a pasta completa ja existe no computador, pode substituir apenas dublagem-master-tauri.exe.",
    "Requer GPU NVIDIA Turing/RTX 20 ou mais nova e driver NVIDIA R580 ou superior.",
    ""
  ].join("\n");

  await writeFile(path.join(targetDirectory, "LEIA-ME-DISTRIBUICAO.txt"), instructions, "utf8");
}

async function createPortableArchive(sourceDirectory, targetArchive) {
  await rm(targetArchive, { force: true });
  await mkdir(path.dirname(targetArchive), { recursive: true });

  const zipFile = new yazl.ZipFile();
  const output = createWriteStream(targetArchive);
  const completion = new Promise((resolve, reject) => {
    output.on("close", resolve);
    output.on("error", reject);
    zipFile.outputStream.on("error", reject);
  });

  zipFile.outputStream.pipe(output);
  await addDirectoryToArchive(zipFile, sourceDirectory, path.basename(sourceDirectory));
  zipFile.end();
  await completion;
}

async function addDirectoryToArchive(zipFile, sourceDirectory, archiveDirectory) {
  const entries = (await readdir(sourceDirectory, { withFileTypes: true })).sort((left, right) =>
    left.name.localeCompare(right.name)
  );

  for (const entry of entries) {
    const sourcePath = path.join(sourceDirectory, entry.name);
    const archivePath = toZipPath(path.join(archiveDirectory, entry.name));

    if (entry.isDirectory()) {
      await addDirectoryToArchive(zipFile, sourcePath, archivePath);
      continue;
    }

    if (entry.isFile()) {
      zipFile.addFile(sourcePath, archivePath, {
        compress: false,
        forceZip64Format: true,
        mtime: reproducibleZipDate
      });
    }
  }
}

function toZipPath(filePath) {
  return filePath.split(path.sep).join("/");
}

async function isFile(filePath) {
  const metadata = await stat(filePath).catch(() => null);
  return metadata?.isFile() ?? false;
}

function formatGiB(bytes) {
  return (bytes / 1024 ** 3).toFixed(2);
}
