import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import {
  createLineMetadata,
  mergeNativeTags,
  nativeTagSet,
  removeNativeTagsFromText,
  splitLines,
  tagsByLine,
  type NativeTag
} from "../shared/omnivoice/nativeControls";
import { emptyProjectMetadata, isTauriRuntime, tauriClient } from "../shared/tauri/client";
import {
  defaultOptions,
  type AppConfig,
  type AudioFileEntry,
  type DubbingJobEvent,
  type JobStage,
  type LineSynthesisOverride,
  type NativeSynthesisSettings,
  type ProjectFileMetadata,
  type ProjectLineMetadata,
  type ProjectMetadata
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
  projectMetadata: ProjectMetadata;
  selectedPath: string | null;
  selectedLineIndex: number;
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
  linePreviewPath: string | null;
  progress: number;
  isBusy: boolean;
  load: () => Promise<void>;
  saveConfig: (config: AppConfig) => Promise<void>;
  scan: () => Promise<void>;
  selectFile: (path: string) => void;
  setSelectedLineIndex: (lineIndex: number) => void;
  setSourceText: (value: string) => void;
  setTargetText: (value: string) => void;
  insertNativeTag: (tag: NativeTag) => void;
  removeNativeTag: (tag: NativeTag) => void;
  updateSelectedLineMetadata: (patch: Partial<ProjectLineMetadata>) => void;
  updateSelectedLineSettings: (patch: Partial<NativeSynthesisSettings>) => void;
  previewSelectedLine: () => Promise<void>;
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
  projectMetadata: emptyProjectMetadata(),
  selectedPath: null,
  selectedLineIndex: 0,
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
  linePreviewPath: null,
  progress: 0,
  isBusy: false,
  load: async () => {
    const config = await tauriClient.loadConfig();
    const projectMetadata = await loadProjectMetadataForConfig(config);
    set({ config, projectMetadata });
    if (isTauriRuntime()) {
      await registerJobListeners();
    }
  },
  saveConfig: async (config) => {
    const saved = await tauriClient.saveConfig(config);
    const projectMetadata = await loadProjectMetadataForConfig(saved);
    set({ config: saved, projectMetadata });
  },
  scan: async () => {
    const { config, selectedPath } = get();
    if (!config.inputDir) {
      get().appendLog("Selecione a pasta de origem antes de escanear.", "warning");
      return;
    }
    const [files, loadedProjectMetadata] = await Promise.all([
      tauriClient.scanAudioFolder(config.inputDir, config.outputDir),
      loadProjectMetadataForConfig(config)
    ]);
    const nextSelectedPath = files.some((file) => file.path === selectedPath)
      ? selectedPath
      : (files[0]?.path ?? null);
    const fileTranscriptions = transcriptionsFromFiles(files);
    const projectMetadata = ensureProjectBaselineMetadata(
      loadedProjectMetadata,
      files,
      fileTranscriptions
    );
    const metadataTranscriptions = transcriptionsFromProjectMetadata(projectMetadata, files);
    const metadataBaselines = transcriptionBaselinesFromProjectMetadata(projectMetadata, files);
    const transcriptionDrafts = {
      ...fileTranscriptions,
      ...metadataTranscriptions,
      ...get().transcriptionDrafts
    };
    const transcriptionBaselines = {
      ...fileTranscriptions,
      ...metadataBaselines
    };
    if (projectMetadata !== loadedProjectMetadata) {
      queueProjectMetadataSave(config.outputDir, projectMetadata);
    }
    set({
      files,
      projectMetadata,
      selectedPath: nextSelectedPath,
      selectedLineIndex: 0,
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
        selectedLineIndex: 0,
        linePreviewPath: null,
        lastOutputPath: outputPathForSelection(path, state.files, state.config),
        transcriptionBaselines,
        ...draftStateForPath(path, state.transcriptionDrafts, state.files)
      };
    });
  },
  setSelectedLineIndex: (lineIndex) => {
    set({ selectedLineIndex: Math.max(0, lineIndex) });
  },
  setSourceText: (sourceText) => {
    set((state) => {
      const transcriptionDrafts = upsertDraft(state.transcriptionDrafts, state.selectedPath, {
        sourceText
      });
      const projectMetadata = upsertProjectFileDraft(state, { sourceText });
      queueProjectMetadataSave(state.config.outputDir, projectMetadata);
      return { sourceText, transcriptionDrafts, projectMetadata };
    });
  },
  setTargetText: (targetText) => {
    set((state) => {
      const inlineTagsByLine = tagsByLine(targetText);
      const sanitizedTargetText = removeNativeTagsFromText(targetText);
      const transcriptionDrafts = upsertDraft(state.transcriptionDrafts, state.selectedPath, {
        targetText: sanitizedTargetText
      });
      const projectMetadata = syncProjectTargetText(
        upsertProjectFileDraft(state, { targetText: sanitizedTargetText }),
        state,
        sanitizedTargetText,
        inlineTagsByLine
      );
      queueProjectMetadataSave(state.config.outputDir, projectMetadata);
      return { targetText: sanitizedTargetText, transcriptionDrafts, projectMetadata };
    });
  },
  insertNativeTag: (tag) => {
    if (!nativeTagSet.has(tag)) {
      get().appendLog(`Tag OmniVoice nao suportada: ${tag}`, "warning");
      return;
    }

    set((state) => {
      const current = selectedLineMetadata(state);
      const projectMetadata = upsertSelectedLineMetadata(state, {
        tags: mergeNativeTags(current.tags, [tag])
      });
      queueProjectMetadataSave(state.config.outputDir, projectMetadata);
      return { projectMetadata };
    });
  },
  removeNativeTag: (tag) => {
    set((state) => {
      const current = selectedLineMetadata(state);
      const projectMetadata = upsertSelectedLineMetadata(state, {
        tags: current.tags.filter((currentTag) => currentTag !== tag)
      });
      queueProjectMetadataSave(state.config.outputDir, projectMetadata);
      return { projectMetadata };
    });
  },
  updateSelectedLineMetadata: (patch) => {
    set((state) => {
      const projectMetadata = upsertSelectedLineMetadata(state, patch);
      queueProjectMetadataSave(state.config.outputDir, projectMetadata);
      return { projectMetadata };
    });
  },
  updateSelectedLineSettings: (patch) => {
    set((state) => {
      const current = selectedLineMetadata(state);
      const projectMetadata = upsertSelectedLineMetadata(state, {
        settings: { ...current.settings, ...patch }
      });
      queueProjectMetadataSave(state.config.outputDir, projectMetadata);
      return { projectMetadata };
    });
  },
  previewSelectedLine: async () => {
    const state = get();
    if (!state.selectedPath) {
      state.appendLog("Selecione uma linha antes da previa.", "warning");
      return;
    }
    const text = splitLines(state.targetText)[state.selectedLineIndex]?.trim() ?? "";
    if (text.length === 0) {
      state.appendLog("Linha selecionada sem texto para previa.", "warning");
      return;
    }
    const lineMetadata = selectedLineMetadata(state);
    try {
      const linePreviewPath = await tauriClient.previewSynthesisLine({
        sourceAudio: state.selectedPath,
        text,
        tags: lineMetadata.tags,
        settings: lineMetadata.settings
      });
      set({ linePreviewPath, lastOutputPath: linePreviewPath });
      state.appendLog("Previa da linha gerada.", "success");
    } catch (unknownError: unknown) {
      state.appendLog(errorMessage(unknownError), "error");
    }
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

    set((state) => {
      const projectMetadata = upsertProjectFileDraft(state, baseline);
      queueProjectMetadataSave(state.config.outputDir, projectMetadata);
      return {
        sourceText: baseline.sourceText,
        targetText: baseline.targetText,
        projectMetadata,
        transcriptionDrafts: {
          ...state.transcriptionDrafts,
          [selectedPath]: baseline
        }
      };
    });
  },
  startDubbing: async () => {
    const { config, selectedPath, sourceText, targetText, files, projectMetadata } = get();
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
        customTargetText: targetText.trim().length > 0 ? targetText : null,
        lineOverrides: buildLineOverrides({
          selectedPath,
          files,
          projectMetadata,
          targetText,
          baseSettings: config.options.nativeSynthesis
        })
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
let metadataSaveTimer: ReturnType<typeof setTimeout> | null = null;

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

export function applyJobEvent(payload: DubbingJobEvent): void {
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
    update.transcriptionBaselines = upsertMissingBaseline(
      state.transcriptionBaselines,
      eventPath,
      transcriptionPatch
    );
    const projectMetadata = ensureProjectFileBaseline(
      upsertProjectFileDraft(state, transcriptionPatch, eventPath),
      state.files,
      eventPath,
      transcriptionPatch
    );
    update.projectMetadata = projectMetadata;
    queueProjectMetadataSave(state.config.outputDir, projectMetadata);
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

async function loadProjectMetadataForConfig(config: AppConfig): Promise<ProjectMetadata> {
  if (!config.outputDir) {
    return emptyProjectMetadata();
  }

  try {
    return await tauriClient.loadProjectMetadata(config.outputDir);
  } catch {
    return emptyProjectMetadata();
  }
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

function transcriptionsFromProjectMetadata(
  metadata: ProjectMetadata,
  files: AudioFileEntry[]
): TranscriptionMap {
  return files.reduce<TranscriptionMap>((drafts, file) => {
    const fileMetadata = metadata.files[file.name];
    if (fileMetadata?.sourceText || fileMetadata?.targetText) {
      drafts[file.path] = {
        sourceText: fileMetadata.sourceText ?? "",
        targetText: fileMetadata.targetText ?? ""
      };
    }
    return drafts;
  }, {});
}

function transcriptionBaselinesFromProjectMetadata(
  metadata: ProjectMetadata,
  files: AudioFileEntry[]
): TranscriptionMap {
  return files.reduce<TranscriptionMap>((baselines, file) => {
    const fileMetadata = metadata.files[file.name];
    if (fileMetadata?.baselineSourceText || fileMetadata?.baselineTargetText) {
      baselines[file.path] = {
        sourceText: fileMetadata.baselineSourceText ?? "",
        targetText: fileMetadata.baselineTargetText ?? ""
      };
    }
    return baselines;
  }, {});
}

function baselineStateForPath(
  path: string,
  baselines: TranscriptionMap,
  files: AudioFileEntry[]
): TranscriptionMap {
  const cached = files.find((file) => file.path === path)?.transcription;
  if (!cached || baselines[path]) {
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

function upsertMissingBaseline(
  baselines: TranscriptionMap,
  path: string | null,
  patch: Partial<TranscriptionDraft>
): TranscriptionMap {
  if (!path) {
    return baselines;
  }

  const current = baselines[path] ?? { sourceText: "", targetText: "" };
  return {
    ...baselines,
    [path]: {
      sourceText: current.sourceText.length > 0 ? current.sourceText : (patch.sourceText ?? ""),
      targetText: current.targetText.length > 0 ? current.targetText : (patch.targetText ?? "")
    }
  };
}

function upsertProjectFileDraft(
  state: WorkspaceState,
  patch: Partial<TranscriptionDraft>,
  path = state.selectedPath
): ProjectMetadata {
  const fileKey = fileKeyForPath(path, state.files);
  if (!fileKey) {
    return state.projectMetadata;
  }

  const current = state.projectMetadata.files[fileKey] ?? emptyProjectFileMetadata();
  return {
    ...state.projectMetadata,
    version: 1,
    files: {
      ...state.projectMetadata.files,
      [fileKey]: {
        ...current,
        sourceText: patch.sourceText ?? current.sourceText,
        targetText: patch.targetText ?? current.targetText
      }
    }
  };
}

function ensureProjectBaselineMetadata(
  metadata: ProjectMetadata,
  files: AudioFileEntry[],
  fileTranscriptions: TranscriptionMap
): ProjectMetadata {
  let nextMetadata = metadata;
  for (const file of files) {
    const transcription = fileTranscriptions[file.path];
    if (!transcription) {
      continue;
    }
    nextMetadata = ensureProjectFileBaseline(nextMetadata, files, file.path, transcription);
  }
  return nextMetadata;
}

function ensureProjectFileBaseline(
  metadata: ProjectMetadata,
  files: AudioFileEntry[],
  path: string | null,
  patch: Partial<TranscriptionDraft>
): ProjectMetadata {
  const fileKey = fileKeyForPath(path, files);
  if (!fileKey) {
    return metadata;
  }

  const current = metadata.files[fileKey] ?? emptyProjectFileMetadata();
  const baselineSourceText = current.baselineSourceText ?? patch.sourceText ?? null;
  const baselineTargetText = current.baselineTargetText ?? patch.targetText ?? null;
  if (
    baselineSourceText === current.baselineSourceText &&
    baselineTargetText === current.baselineTargetText
  ) {
    return metadata;
  }

  return {
    ...metadata,
    version: 1,
    files: {
      ...metadata.files,
      [fileKey]: {
        ...current,
        baselineSourceText,
        baselineTargetText
      }
    }
  };
}

function syncProjectTargetText(
  metadata: ProjectMetadata,
  state: WorkspaceState,
  targetText: string,
  inlineTagsByLine: NativeTag[][] = []
): ProjectMetadata {
  const fileKey = fileKeyForPath(state.selectedPath, state.files);
  if (!fileKey) {
    return metadata;
  }

  const currentFile = metadata.files[fileKey] ?? emptyProjectFileMetadata();
  const lines = splitLines(targetText);
  const nextLines = { ...currentFile.lines };
  lines.forEach((line, index) => {
    const tags = inlineTagsByLine[index] ?? [];
    const key = String(index);
    if (nextLines[key] || tags.length > 0) {
      const currentLine =
        nextLines[key] ?? createLineMetadata(line, state.config.options.nativeSynthesis);
      nextLines[key] = {
        ...currentLine,
        tags: mergeNativeTags(currentLine.tags, tags)
      };
    }
  });

  return {
    ...metadata,
    files: {
      ...metadata.files,
      [fileKey]: {
        ...currentFile,
        targetText,
        lines: nextLines
      }
    }
  };
}

function upsertSelectedLineMetadata(
  state: WorkspaceState,
  patch: Partial<ProjectLineMetadata>
): ProjectMetadata {
  const fileKey = fileKeyForPath(state.selectedPath, state.files);
  if (!fileKey) {
    return state.projectMetadata;
  }

  const fileMetadata = state.projectMetadata.files[fileKey] ?? emptyProjectFileMetadata();
  const lineKey = String(state.selectedLineIndex);
  const currentLine = selectedLineMetadata(state);
  return {
    ...state.projectMetadata,
    version: 1,
    files: {
      ...state.projectMetadata.files,
      [fileKey]: {
        ...fileMetadata,
        lines: {
          ...fileMetadata.lines,
          [lineKey]: {
            ...currentLine,
            ...patch,
            settings: patch.settings ?? currentLine.settings
          }
        }
      }
    }
  };
}

export function selectedLineMetadata(state: WorkspaceState): ProjectLineMetadata {
  const fileKey = fileKeyForPath(state.selectedPath, state.files);
  const line = splitLines(state.targetText)[state.selectedLineIndex] ?? "";
  if (!fileKey) {
    return createLineMetadata(line, state.config.options.nativeSynthesis);
  }

  return (
    state.projectMetadata.files[fileKey]?.lines[String(state.selectedLineIndex)] ??
    createLineMetadata(line, state.config.options.nativeSynthesis)
  );
}

function buildLineOverrides(input: {
  selectedPath: string;
  files: AudioFileEntry[];
  projectMetadata: ProjectMetadata;
  targetText: string;
  baseSettings: NativeSynthesisSettings;
}): LineSynthesisOverride[] {
  const fileKey = fileKeyForPath(input.selectedPath, input.files);
  const fileMetadata = fileKey ? input.projectMetadata.files[fileKey] : undefined;
  if (!fileMetadata || !hasFileLineOverrides(fileMetadata, input.baseSettings)) {
    return [];
  }

  return splitLines(input.targetText)
    .map((line, lineIndex) => ({
      lineIndex,
      targetText: removeNativeTagsFromText(line).trim(),
      tags: fileMetadata.lines[String(lineIndex)]?.tags ?? [],
      settings: cloneSettings(fileMetadata.lines[String(lineIndex)]?.settings ?? input.baseSettings)
    }))
    .filter((line) => line.targetText.length > 0);
}

function hasFileLineOverrides(
  fileMetadata: ProjectFileMetadata,
  baseSettings: NativeSynthesisSettings
): boolean {
  return Object.values(fileMetadata.lines).some((line) => {
    if (!line) {
      return false;
    }
    return (
      line.tags.length > 0 ||
      line.characterId !== null ||
      line.notes !== null ||
      JSON.stringify(line.settings) !== JSON.stringify(baseSettings)
    );
  });
}

function emptyProjectFileMetadata(): ProjectFileMetadata {
  return {
    sourceText: null,
    targetText: null,
    baselineSourceText: null,
    baselineTargetText: null,
    lines: {}
  };
}

function fileKeyForPath(path: string | null, files: AudioFileEntry[]): string | null {
  if (!path) {
    return null;
  }

  return files.find((file) => file.path === path)?.name ?? path;
}

function cloneSettings(settings: NativeSynthesisSettings): NativeSynthesisSettings {
  return { ...settings };
}

function queueProjectMetadataSave(outputDir: string | null, metadata: ProjectMetadata): void {
  if (!outputDir || !isTauriRuntime()) {
    return;
  }

  if (metadataSaveTimer) {
    clearTimeout(metadataSaveTimer);
  }
  metadataSaveTimer = setTimeout(() => {
    void tauriClient.saveProjectMetadata(outputDir, metadata).catch((unknownError: unknown) => {
      useWorkspaceStore.getState().appendLog(errorMessage(unknownError), "error");
    });
  }, 450);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
