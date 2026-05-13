import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { isTauriRuntime } from "./client";

export async function openDirectoryLocation(path: string): Promise<void> {
  assertTauriRuntime();
  await openPath(path);
}

export async function revealFileLocation(path: string): Promise<void> {
  assertTauriRuntime();
  await revealItemInDir(path);
}

function assertTauriRuntime(): void {
  if (!isTauriRuntime()) {
    throw new Error("Abertura de pasta disponível apenas no aplicativo desktop.");
  }
}
