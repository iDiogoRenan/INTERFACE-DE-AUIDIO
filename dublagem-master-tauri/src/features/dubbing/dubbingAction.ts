import type { AudioFileStatus } from "../../shared/tauri/types";

export type DubbingActionIntent = "dub" | "redub";

export interface DubbingActionCopy {
  intent: DubbingActionIntent;
  idleLabel: string;
  busyLabel: string;
}

const redubbableStatuses = new Set<AudioFileStatus>(["dubbed", "rejected"]);

export function dubbingActionCopy(status: AudioFileStatus | null): DubbingActionCopy {
  if (status && redubbableStatuses.has(status)) {
    return {
      intent: "redub",
      idleLabel: "Redublar selecionado",
      busyLabel: "Redublando"
    };
  }

  return {
    intent: "dub",
    idleLabel: "Dublar selecionado",
    busyLabel: "Dublando"
  };
}
