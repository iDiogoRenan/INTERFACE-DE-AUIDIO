import { beforeEach, describe, expect, it, vi } from "vitest";
import { useWorkspaceStore } from "./workspaceStore";
import { defaultOptions, type AppConfig, type AudioFileEntry } from "../shared/tauri/types";

const clientMocks = vi.hoisted(() => ({
  scanAudioFolder: vi.fn(),
  startDubbingJob: vi.fn(() => Promise.resolve("job-1"))
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => undefined))
}));

vi.mock("../shared/tauri/client", () => ({
  isTauriRuntime: () => false,
  tauriClient: {
    loadConfig: vi.fn(),
    saveConfig: vi.fn(),
    scanAudioFolder: clientMocks.scanAudioFolder,
    startDubbingJob: clientMocks.startDubbingJob,
    cancelJob: vi.fn()
  }
}));

const config: AppConfig = {
  inputDir: "E:\\audio\\origem",
  outputDir: "E:\\audio\\saida",
  guideAudio: null,
  approvedDir: null,
  modelDir: "E:\\audio\\models",
  voicePoolDir: "voice_pool_ptbr",
  options: defaultOptions
};

const fileA = audioFile("E:\\audio\\origem\\line_a.wav", "line_a.wav");
const fileB = audioFile("E:\\audio\\origem\\line_b.wav", "line_b.wav");
const dubbedFile = audioFile("E:\\audio\\origem\\line_c.wav", "line_c.wav", "dubbed");
const cachedDubbedFile = audioFile("E:\\audio\\origem\\line_d.wav", "line_d.wav", "dubbed", {
  sourceText: "Hello from cache.",
  targetText: "Ola do cache."
});

describe("workspaceStore transcription hydration", () => {
  beforeEach(() => {
    clientMocks.scanAudioFolder.mockReset();
    clientMocks.startDubbingJob.mockClear();
    useWorkspaceStore.setState({
      config,
      files: [fileA, fileB],
      selectedPath: fileA.path,
      sourceText: "",
      targetText: "",
      transcriptionDrafts: {},
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
      logs: []
    });
  });

  it("keeps source and target text scoped to the selected file", () => {
    useWorkspaceStore.getState().setSourceText("hello from line a");
    useWorkspaceStore.getState().setTargetText("ola da linha a");

    useWorkspaceStore.getState().selectFile(fileB.path);

    expect(useWorkspaceStore.getState().sourceText).toBe("");
    expect(useWorkspaceStore.getState().targetText).toBe("");

    useWorkspaceStore.getState().setSourceText("hello from line b");
    useWorkspaceStore.getState().setTargetText("ola da linha b");
    useWorkspaceStore.getState().selectFile(fileA.path);

    expect(useWorkspaceStore.getState().sourceText).toBe("hello from line a");
    expect(useWorkspaceStore.getState().targetText).toBe("ola da linha a");
  });

  it("does not send stale custom text after switching to an unhydrated file", async () => {
    useWorkspaceStore.getState().setSourceText("old source");
    useWorkspaceStore.getState().setTargetText("old target");
    useWorkspaceStore.getState().selectFile(fileB.path);

    await useWorkspaceStore.getState().startDubbing();

    expect(clientMocks.startDubbingJob).toHaveBeenCalledWith(
      expect.objectContaining({
        inputPaths: [fileB.path],
        customSourceText: null,
        customTargetText: null
      })
    );
  });

  it("hydrates the result player path when a dubbed file is selected", () => {
    useWorkspaceStore.setState({ files: [fileA, fileB, dubbedFile] });

    useWorkspaceStore.getState().selectFile(dubbedFile.path);

    expect(useWorkspaceStore.getState().lastOutputPath).toBe("E:\\audio\\saida\\line_c.wav");

    useWorkspaceStore.getState().selectFile(fileB.path);

    expect(useWorkspaceStore.getState().lastOutputPath).toBeNull();
  });

  it("hydrates cached transcription text when a dubbed file is selected", () => {
    useWorkspaceStore.setState({ files: [fileA, cachedDubbedFile] });

    useWorkspaceStore.getState().selectFile(cachedDubbedFile.path);

    expect(useWorkspaceStore.getState().sourceText).toBe("Hello from cache.");
    expect(useWorkspaceStore.getState().targetText).toBe("Ola do cache.");
  });

  it("hydrates cached transcription text after scanning a processed folder", async () => {
    clientMocks.scanAudioFolder.mockResolvedValue([cachedDubbedFile]);
    useWorkspaceStore.setState({ files: [], selectedPath: null });

    await useWorkspaceStore.getState().scan();

    expect(useWorkspaceStore.getState().selectedPath).toBe(cachedDubbedFile.path);
    expect(useWorkspaceStore.getState().sourceText).toBe("Hello from cache.");
    expect(useWorkspaceStore.getState().targetText).toBe("Ola do cache.");
  });
});

function audioFile(
  path: string,
  name: string,
  status: AudioFileEntry["status"] = "pending",
  transcription: AudioFileEntry["transcription"] = null
): AudioFileEntry {
  return {
    name,
    path,
    family: "line",
    status,
    metadata: null,
    transcription
  };
}
