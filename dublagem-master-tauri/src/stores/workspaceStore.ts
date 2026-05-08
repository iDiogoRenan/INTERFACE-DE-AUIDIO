import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { isTauriRuntime, tauriClient } from "../shared/tauri/client";
import {
  defaultOptions,
  type AppConfig,
  type AudioFileEntry,
  type DubbingJobEvent
} from "../shared/tauri/types";

export interface LogEntry {
  id: string;
  level: "info" | "warning" | "error" | "success";
  message: string;
}

interface WorkspaceState {
  config: AppConfig;
  files: AudioFileEntry[];
  selectedPath: string | null;
  sourceText: string;
  targetText: string;
  logs: LogEntry[];
  activeJobId: string | null;
  progress: number;
  isBusy: boolean;
  load: () => Promise<void>;
  saveConfig: (config: AppConfig) => Promise<void>;
  scan: () => Promise<void>;
  selectFile: (path: string) => void;
  setSourceText: (value: string) => void;
  setTargetText: (value: string) => void;
  startDubbing: () => Promise<void>;
  cancelJob: () => Promise<void>;
  appendLog: (message: string, level?: LogEntry["level"]) => void;
}

const initialConfig: AppConfig = {
  inputDir: null,
  outputDir: null,
  guideAudio: null,
  approvedDir: null,
  modelDir: null,
  voicePoolDir: "voice_pool_ptbr",
  options: defaultOptions
};

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  config: initialConfig,
  files: [],
  selectedPath: null,
  sourceText: "",
  targetText: "",
  logs: [],
  activeJobId: null,
  progress: 0,
  isBusy: false,
  load: async () => {
    const config = await tauriClient.loadConfig();
    set({ config });
    if (isTauriRuntime()) {
      await registerJobListeners();
    }
  },
  saveConfig: async (config) => {
    const saved = await tauriClient.saveConfig(config);
    set({ config: saved });
  },
  scan: async () => {
    const { config } = get();
    if (!config.inputDir) {
      get().appendLog("Selecione a pasta de origem antes de escanear.", "warning");
      return;
    }
    const files = await tauriClient.scanAudioFolder(config.inputDir, config.outputDir);
    set({ files, selectedPath: files[0]?.path ?? null });
  },
  selectFile: (path) => {
    set({ selectedPath: path });
  },
  setSourceText: (sourceText) => {
    set({ sourceText });
  },
  setTargetText: (targetText) => {
    set({ targetText });
  },
  startDubbing: async () => {
    const { config, selectedPath, sourceText, targetText } = get();
    if (!selectedPath || !config.outputDir) {
      get().appendLog("Selecione um arquivo e uma pasta de destino.", "warning");
      return;
    }

    set({ isBusy: true, progress: 0 });
    const jobId = await tauriClient.startDubbingJob({
      inputPaths: [selectedPath],
      outputDir: config.outputDir,
      guideAudio: config.guideAudio,
      options: config.options,
      customSourceText: sourceText.trim().length > 0 ? sourceText : null,
      customTargetText: targetText.trim().length > 0 ? targetText : null
    });
    set({ activeJobId: jobId });
  },
  cancelJob: async () => {
    const { activeJobId } = get();
    if (!activeJobId) {
      return;
    }
    await tauriClient.cancelJob(activeJobId);
    set({ activeJobId: null, isBusy: false });
  },
  appendLog: (message, level = "info") => {
    set((state) => ({
      logs: [{ id: crypto.randomUUID(), level, message }, ...state.logs].slice(0, 200)
    }));
  }
}));

let listenersRegistered = false;

async function registerJobListeners(): Promise<void> {
  if (listenersRegistered) {
    return;
  }
  listenersRegistered = true;
  const store = useWorkspaceStore;

  await listen<DubbingJobEvent>("job:log", (event) => {
    store.getState().appendLog(event.payload.message, "info");
  });
  await listen<DubbingJobEvent>("job:progress", (event) => {
    store.setState({ progress: event.payload.progress ?? 0 });
  });
  await listen<DubbingJobEvent>("job:file-complete", (event) => {
    store.getState().appendLog(event.payload.message, "success");
  });
  await listen<DubbingJobEvent>("job:finished", (event) => {
    store.getState().appendLog(event.payload.message, "success");
    store.setState({ activeJobId: null, isBusy: false, progress: 100 });
  });
  await listen<DubbingJobEvent>("job:failed", (event) => {
    store.getState().appendLog(event.payload.message, "error");
    store.setState({ activeJobId: null, isBusy: false });
  });
}
