import { open } from "@tauri-apps/plugin-dialog";

export type PathPickerMode = "directory" | "file";

const audioFileExtensions = ["wav", "mp3", "wem", "ogg", "flac"] as const;

export async function pickPath(mode: PathPickerMode): Promise<string | null> {
  const selectedPath = await open({
    directory: mode === "directory",
    multiple: false,
    filters:
      mode === "file" ? [{ name: "Audio", extensions: Array.from(audioFileExtensions) }] : undefined
  });

  return typeof selectedPath === "string" ? selectedPath : null;
}

export function pickDirectory(): Promise<string | null> {
  return pickPath("directory");
}
