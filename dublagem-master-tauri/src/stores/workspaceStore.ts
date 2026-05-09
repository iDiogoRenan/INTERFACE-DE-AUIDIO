import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { isTauriRuntime, tauriClient } from "../shared/tauri/client";
import {
  defaultOptions,
  type AppConfig,
  type AudioFileEntry,
  type DubbingJobEvent,
  type JobStage
} from "../shared/tauri/types";

export interface LogEntry {
  id: string;
  level: "info" | "warning" | "error" | "success";
  message: string;
}

interface TranscriptionDraft {
  sourceText: string;
  targetText: string;
}

type TranscriptionMap = Record<string, TranscriptionDraft | undefined>;

interface WorkspaceState {
  config: AppConfig;
  files: AudioFileEntry[];
  selectedPath: string | null;
  sourceText: string;
  targetText: string;
  transcriptionDrafts: TranscriptionMap;
  transcriptionBaselines: TranscriptionMap;
  logs: LogEntry[];
  activeJobId: string | null;
  currentStage: JobStage | null;
  currentStatus: string;
  currentFileName: string | null;
  currentFileIndex: number | null;
  totalFiles: number | null;
  isCancelling: boolean;
  lastOutputPath: string | null;
  progress: number;
  isBusy: boolean;
  load: () => Promise<void>;
  saveConfig: (config: AppConfig) => Promise<void>;
  scan: () => Promise<void>;
  selectFile: (path: string) => void;
  setSourceText: (value: string) => void;
  setTargetText: (value: string) => void;
  revertTranscription: () => void;
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
  transcriptionDrafts: {},
  transcriptionBaselines: {},
  logs: [],
  activeJobId: null,
  currentStage: null,
  currentStatus: "Aguardando job.",
  currentFileName: null,
  currentFileIndex: null,
  totalFiles: null,
  isCancelling: false,
  lastOutputPath: null,
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
    const { config, selectedPath } = get();
    if (!config.inputDir) {
      get().appendLog("Selecione a pasta de origem antes de escanear.", "warning");
      return;
    }
    const files = await tauriClient.scanAudioFolder(config.inputDir, config.outputDir);
    const nextSelectedPath = files.some((file) => file.path === selectedPath)
      ? selectedPath
      : (files[0]?.path ?? null);
    const fileTranscriptions = transcriptionsFromFiles(files);
    const transcriptionDrafts = {
      ...fileTranscriptions,
      ...get().transcriptionDrafts
    };
    const transcriptionBaselines = {
      ...get().transcriptionBaselines,
      ...fileTranscriptions
    };
    set({
      files,
      selectedPath: nextSelectedPath,
      lastOutputPath: outputPathForSelection(nextSelectedPath, files, config),
      transcriptionDrafts,
      transcriptionBaselines,
      ...draftStateForPath(nextSelectedPath, transcriptionDrafts, files)
    });
  },
  selectFile: (path) => {
    set((state) => {
      const transcriptionBaselines = baselineStateForPath(
        path,
        state.transcriptionBaselines,
        state.files
      );
      return {
        selectedPath: path,
        lastOutputPath: outputPathForSelection(path, state.files, state.config),
        transcriptionBaselines,
        ...draftStateForPath(path, state.transcriptionDrafts, state.files)
      };
    });
  },
  setSourceText: (sourceText) => {
    set((state) => ({
      sourceText,
      transcriptionDrafts: upsertDraft(state.transcriptionDrafts, state.selectedPath, {
        sourceText
      })
    }));
  },
  setTargetText: (targetText) => {
    set((state) => ({
      targetText,
      transcriptionDrafts: upsertDraft(state.transcriptionDrafts, state.selectedPath, {
        targetText
      })
    }));
  },
  revertTranscription: () => {
    const { selectedPath, transcriptionBaselines } = get();
    if (!selectedPath) {
      return;
    }

    const baseline = transcriptionBaselines[selectedPath];
    if (!baseline) {
      return;
    }

    set((state) => ({
      sourceText: baseline.sourceText,
      targetText: baseline.targetText,
      transcriptionDrafts: {
        ...state.transcriptionDrafts,
        [selectedPath]: baseline
      }
    }));
  },
  startDubbing: async () => {
    const { config, selectedPath, sourceText, targetText } = get();
    if (!selectedPath || !config.outputDir) {
      get().appendLog("Selecione um arquivo e uma pasta de destino.", "warning");
      return;
    }

    set({
      isBusy: true,
      isCancelling: false,
      progress: 0,
      currentStage: "queued",
      currentStatus: "Enviando job para o backend.",
      currentFileName: null,
      currentFileIndex: null,
      totalFiles: null,
      lastOutputPath: null
    });
    try {
      const jobId = await tauriClient.startDubbingJob({
        inputPaths: [selectedPath],
        outputDir: config.outputDir,
        guideAudio: config.guideAudio,
        modelDir: config.modelDir,
        options: config.options,
        customSourceText: sourceText.trim().length > 0 ? sourceText : null,
        customTargetText: targetText.trim().length > 0 ? targetText : null
      });
      set({ activeJobId: jobId });
    } catch (unknownError: unknown) {
      get().appendLog(errorMessage(unknownError), "error");
      set({ isBusy: false, isCancelling: false, progress: 0, currentStage: "failed" });
    }
  },
  cancelJob: async () => {
    const { activeJobId } = get();
    if (!activeJobId) {
      return;
    }
    set({
      isCancelling: true,
      currentStage: "cancelling",
      currentStatus: "Cancelamento solicitado. Aguardando o backend encerrar a etapa atual."
    });
    try {
      await tauriClient.cancelJob(activeJobId);
    } catch (unknownError: unknown) {
      get().appendLog(errorMessage(unknownError), "error");
      set({ isCancelling: false });
    }
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
  await listen<DubbingJobEvent>("job:stage", (event) => {
    applyJobEvent(event.payload);
    store.getState().appendLog(event.payload.message, "info");
  });
  await listen<DubbingJobEvent>("job:transcription", (event) => {
    applyJobEvent(event.payload);
    store.getState().appendLog(event.payload.message, "success");
  });
  await listen<DubbingJobEvent>("job:progress", (event) => {
    applyJobEvent(event.payload);
  });
  await listen<DubbingJobEvent>("job:file-complete", (event) => {
    applyJobEvent(event.payload);
    store.getState().appendLog(event.payload.message, "success");
  });
  await listen<DubbingJobEvent>("job:cancelled", (event) => {
    applyJobEvent(event.payload);
    store.getState().appendLog(event.payload.message, "warning");
    store.setState({
      activeJobId: null,
      isBusy: false,
      isCancelling: false,
      currentStage: "cancelled"
    });
  });
  await listen<DubbingJobEvent>("job:finished", (event) => {
    applyJobEvent(event.payload);
    store.getState().appendLog(event.payload.message, "success");
    store.setState({
      activeJobId: null,
      isBusy: false,
      isCancelling: false,
      currentStage: "finished",
      progress: 100
    });
  });
  await listen<DubbingJobEvent>("job:failed", (event) => {
    applyJobEvent(event.payload);
    store.getState().appendLog(event.payload.message, "error");
    store.setState({
      activeJobId: null,
      isBusy: false,
      isCancelling: false,
      currentStage: "failed"
    });
  });
}

function applyJobEvent(payload: DubbingJobEvent): void {
  const state = useWorkspaceStore.getState();
  const eventPath = payload.filePath ?? state.selectedPath;
  const update: Partial<WorkspaceState> = {
    currentStatus: payload.message
  };

  if (payload.stage) {
    update.currentStage = payload.stage;
  }
  if (payload.progress !== null) {
    update.progress = payload.progress;
  }
  if (payload.fileName !== null) {
    update.currentFileName = payload.fileName;
  }
  if (payload.fileIndex !== null) {
    update.currentFileIndex = payload.fileIndex;
  }
  if (payload.totalFiles !== null) {
    update.totalFiles = payload.totalFiles;
  }

  const transcriptionPatch: Partial<TranscriptionDraft> = {};
  if (payload.sourceText !== null) {
    transcriptionPatch.sourceText = payload.sourceText;
  }
  if (payload.targetText !== null) {
    transcriptionPatch.targetText = payload.targetText;
  }
  if (Object.keys(transcriptionPatch).length > 0) {
    update.transcriptionDrafts = upsertDraft(
      state.transcriptionDrafts,
      eventPath,
      transcriptionPatch
    );
    update.transcriptionBaselines = upsertDraft(
      state.transcriptionBaselines,
      eventPath,
      transcriptionPatch
    );
  }

  const shouldHydrateSelectedEditor = eventPath === state.selectedPath;
  if (shouldHydrateSelectedEditor && payload.sourceText !== null) {
    update.sourceText = payload.sourceText;
  }
  if (shouldHydrateSelectedEditor && payload.targetText !== null) {
    update.targetText = payload.targetText;
  }
  if (shouldHydrateSelectedEditor && payload.outputPath !== null) {
    update.lastOutputPath = payload.outputPath;
  }
  if (eventPath && payload.outputPath !== null) {
    const transcriptionBaselines = update.transcriptionBaselines ?? state.transcriptionBaselines;
    const completedTranscription = transcriptionBaselines[eventPath] ?? null;
    update.files = state.files.map((file) =>
      file.path === eventPath
        ? { ...file, status: "dubbed", transcription: completedTranscription ?? file.transcription }
        : file
    );
  }

  useWorkspaceStore.setState(update);
}

function outputPathForSelection(
  selectedPath: string | null,
  files: AudioFileEntry[],
  config: AppConfig
): string | null {
  if (!selectedPath || !config.outputDir) {
    return null;
  }

  const selectedFile = files.find((file) => file.path === selectedPath);
  if (selectedFile?.status !== "dubbed") {
    return null;
  }

  return joinNativePath(config.outputDir, selectedFile.name);
}

function joinNativePath(directory: string, fileName: string): string {
  const separator = directory.includes("\\") ? "\\" : "/";
  return `${directory.replace(/[\\/]+$/u, "")}${separator}${fileName}`;
}

function transcriptionsFromFiles(files: AudioFileEntry[]): TranscriptionMap {
  return files.reduce<TranscriptionMap>((drafts, file) => {
    if (file.transcription) {
      drafts[file.path] = {
        sourceText: file.transcription.sourceText,
        targetText: file.transcription.targetText
      };
    }
    return drafts;
  }, {});
}

function baselineStateForPath(
  path: string,
  baselines: TranscriptionMap,
  files: AudioFileEntry[]
): TranscriptionMap {
  const cached = files.find((file) => file.path === path)?.transcription;
  if (!cached) {
    return baselines;
  }

  return {
    ...baselines,
    [path]: {
      sourceText: cached.sourceText,
      targetText: cached.targetText
    }
  };
}

function draftStateForPath(
  path: string | null,
  drafts: TranscriptionMap,
  files: AudioFileEntry[] = []
): Pick<WorkspaceState, "sourceText" | "targetText"> {
  if (!path) {
    return { sourceText: "", targetText: "" };
  }

  const cached = files.find((file) => file.path === path)?.transcription;
  return (
    drafts[path] ??
    (cached
      ? { sourceText: cached.sourceText, targetText: cached.targetText }
      : { sourceText: "", targetText: "" })
  );
}

function upsertDraft(
  drafts: TranscriptionMap,
  path: string | null,
  patch: Partial<TranscriptionDraft>
): TranscriptionMap {
  if (!path) {
    return drafts;
  }

  const current = drafts[path] ?? { sourceText: "", targetText: "" };
  return {
    ...drafts,
    [path]: {
      ...current,
      ...patch
    }
  };
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
