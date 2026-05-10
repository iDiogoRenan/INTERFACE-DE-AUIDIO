import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyJobEvent, useWorkspaceStore } from "./workspaceStore";
import {
  defaultOptions,
  type AppConfig,
  type AudioFileEntry,
  type DubbingJobEvent,
  type DubbingRequest,
  type ProjectMetadata,
  type SynthesisLinePreviewRequest
} from "../shared/tauri/types";
import { defaultSpeechModelPresets } from "../shared/speechModels";

const clientMocks = vi.hoisted(() => ({
  loadProjectMetadata: vi.fn<() => Promise<ProjectMetadata>>(() =>
    Promise.resolve({ version: 1, files: {} })
  ),
  saveProjectMetadata: vi.fn<
    (outputDir: string, metadata: ProjectMetadata) => Promise<ProjectMetadata>
  >(() => Promise.resolve({ version: 1, files: {} })),
  previewSynthesisLine: vi.fn<(request: SynthesisLinePreviewRequest) => Promise<string>>(() =>
    Promise.resolve("E:\\audio\\preview.wav")
  ),
  scanAudioFolder: vi.fn<(inputDir: string, outputDir: string) => Promise<AudioFileEntry[]>>(),
  saveConfig: vi.fn<(config: AppConfig) => Promise<AppConfig>>((nextConfig) =>
    Promise.resolve(nextConfig)
  ),
  startDubbingJob: vi.fn<(request: DubbingRequest) => Promise<string>>(() =>
    Promise.resolve("job-1")
  )
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => undefined))
}));

vi.mock("../shared/tauri/client", () => ({
  emptyProjectMetadata: () => ({ version: 1, files: {} }),
  isTauriRuntime: () => false,
  tauriClient: {
    loadConfig: vi.fn(),
    saveConfig: clientMocks.saveConfig,
    loadProjectMetadata: clientMocks.loadProjectMetadata,
    saveProjectMetadata: clientMocks.saveProjectMetadata,
    scanAudioFolder: clientMocks.scanAudioFolder,
    startDubbingJob: clientMocks.startDubbingJob,
    previewSynthesisLine: clientMocks.previewSynthesisLine,
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
  activeSpeechModel: "omnivoice",
  speechModelPresets: defaultSpeechModelPresets,
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
    clientMocks.loadProjectMetadata.mockClear();
    clientMocks.saveProjectMetadata.mockClear();
    clientMocks.previewSynthesisLine.mockClear();
    clientMocks.scanAudioFolder.mockReset();
    clientMocks.saveConfig.mockClear();
    clientMocks.startDubbingJob.mockClear();
    useWorkspaceStore.setState({
      config,
      files: [fileA, fileB],
      projectMetadata: { version: 1, files: {} },
      selectedPath: fileA.path,
      selectedLineIndex: 0,
      sourceText: "",
      targetText: "",
      transcriptionDrafts: {},
      transcriptionBaselines: {},
      submittedDubbingDrafts: {},
      activeJobId: null,
      currentStage: null,
      currentStatus: "Aguardando processamento.",
      currentFileName: null,
      currentFileIndex: null,
      totalFiles: null,
      isCancelling: false,
      lastOutputPath: null,
      lastOutputRevision: 0,
      linePreviewPath: null,
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

  it("keeps original sidecar baselines when scan cache contains a redubbed draft", async () => {
    const redubbedFile = audioFile(cachedDubbedFile.path, cachedDubbedFile.name, "dubbed", {
      sourceText: "Hello from cache.",
      targetText: "Texto revisado."
    });
    clientMocks.scanAudioFolder.mockResolvedValue([redubbedFile]);
    clientMocks.loadProjectMetadata.mockResolvedValue({
      version: 1,
      files: {
        [redubbedFile.name]: {
          sourceText: "Hello from cache.",
          targetText: "Texto revisado.",
          baselineSourceText: "Hello from cache.",
          baselineTargetText: "Ola do cache.",
          lines: {}
        }
      }
    });
    useWorkspaceStore.setState({ files: [], selectedPath: null });

    await useWorkspaceStore.getState().scan();

    expect(useWorkspaceStore.getState().targetText).toBe("Texto revisado.");
    expect(useWorkspaceStore.getState().transcriptionBaselines[redubbedFile.path]?.targetText).toBe(
      "Ola do cache."
    );
  });

  it("sends the edited cached transcription when redubbing a processed file", async () => {
    useWorkspaceStore.setState({ files: [cachedDubbedFile], selectedPath: null });
    useWorkspaceStore.getState().selectFile(cachedDubbedFile.path);
    useWorkspaceStore.getState().setTargetText("Texto revisado para nova sintese.");

    await useWorkspaceStore.getState().startDubbing();

    expect(clientMocks.startDubbingJob).toHaveBeenCalledWith(
      expect.objectContaining({
        inputPaths: [cachedDubbedFile.path],
        customSourceText: "Hello from cache.",
        customTargetText: "Texto revisado para nova sintese."
      })
    );
  });

  it("keeps submitted manual text when redubbing events replay cached transcription", async () => {
    useWorkspaceStore.setState({ files: [cachedDubbedFile], selectedPath: null });
    useWorkspaceStore.getState().selectFile(cachedDubbedFile.path);
    useWorkspaceStore.getState().setTargetText("Texto revisado para nova sintese.");

    await useWorkspaceStore.getState().startDubbing();
    applyJobEvent(
      jobEvent({
        filePath: cachedDubbedFile.path,
        sourceText: "Hello from cache.",
        targetText: "Ola do cache."
      })
    );

    expect(useWorkspaceStore.getState().targetText).toBe("Texto revisado para nova sintese.");
    expect(
      useWorkspaceStore.getState().projectMetadata.files[cachedDubbedFile.name]?.targetText
    ).toBe("Texto revisado para nova sintese.");

    applyJobEvent(
      jobEvent({
        kind: "file_complete",
        stage: "file_complete",
        filePath: cachedDubbedFile.path,
        outputPath: "E:\\audio\\saida\\line_d.wav",
        sourceText: "Hello from cache.",
        targetText: "Ola do cache."
      })
    );

    expect(useWorkspaceStore.getState().targetText).toBe("Texto revisado para nova sintese.");
  });

  it("sends line synthesis overrides when selected lines have native metadata", async () => {
    useWorkspaceStore.getState().setTargetText("Ola linha um.\nOla linha dois.");
    useWorkspaceStore.getState().setSelectedLineIndex(0);
    useWorkspaceStore.getState().insertNativeTag("[sigh]");
    useWorkspaceStore.getState().updateSelectedLineSettings({ speed: 1.2 });

    await useWorkspaceStore.getState().startDubbing();

    const [[request]] = clientMocks.startDubbingJob.mock.calls;
    expect(request.lineOverrides).toHaveLength(2);
    expect(request.lineOverrides[0]).toMatchObject({
      lineIndex: 0,
      targetText: "Ola linha um.",
      tags: ["[sigh]"]
    });
    expect(request.lineOverrides[0].settings.speed).toBe(1.2);
    expect(request.lineOverrides[1]).toMatchObject({
      lineIndex: 1,
      targetText: "Ola linha dois.",
      tags: []
    });
  });

  it("keeps native tags as removable line metadata instead of spoken text", () => {
    useWorkspaceStore.getState().setTargetText("[sigh] Ola linha um.\nOla linha dois.");
    useWorkspaceStore.getState().setSelectedLineIndex(0);

    expect(useWorkspaceStore.getState().targetText).toBe("Ola linha um.\nOla linha dois.");
    expect(
      useWorkspaceStore.getState().projectMetadata.files[fileA.name]?.lines["0"]?.tags
    ).toEqual(["[sigh]"]);

    useWorkspaceStore.getState().removeNativeTag("[sigh]");

    expect(
      useWorkspaceStore.getState().projectMetadata.files[fileA.name]?.lines["0"]?.tags
    ).toEqual([]);
  });

  it("generates a preview for the selected line using its native settings", async () => {
    useWorkspaceStore.getState().setTargetText("Linha para previa.");
    useWorkspaceStore.getState().insertNativeTag("[sigh]");
    useWorkspaceStore.getState().updateSelectedLineSettings({ voiceMode: "auto" });

    await useWorkspaceStore.getState().previewSelectedLine();

    const [[request]] = clientMocks.previewSynthesisLine.mock.calls;
    expect(request.sourceAudio).toBe(fileA.path);
    expect(request.text).toBe("Linha para previa.");
    expect(request.tags).toEqual(["[sigh]"]);
    expect(request.settings.voiceMode).toBe("auto");
    expect(useWorkspaceStore.getState().linePreviewPath).toBe("E:\\audio\\preview.wav");
    expect(useWorkspaceStore.getState().lastOutputPath).toBe("E:\\audio\\preview.wav");
    expect(useWorkspaceStore.getState().lastOutputRevision).toBe(1);
  });

  it("bumps the result player revision when a preview returns the same path", async () => {
    useWorkspaceStore.getState().setTargetText("Linha para previa.");

    await useWorkspaceStore.getState().previewSelectedLine();
    await useWorkspaceStore.getState().previewSelectedLine();

    expect(useWorkspaceStore.getState().lastOutputPath).toBe("E:\\audio\\preview.wav");
    expect(useWorkspaceStore.getState().lastOutputRevision).toBe(2);
  });

  it("normalizes line synthesis controls before sending them to the backend", async () => {
    useWorkspaceStore.getState().setTargetText("Linha com controles aceitos.");
    useWorkspaceStore.getState().updateSelectedLineSettings({
      voiceMode: "design",
      instruct: "   ",
      speed: Number.NaN,
      durationSeconds: 99,
      numStep: 2,
      guidanceScale: 99,
      positionTemperature: -5,
      classTemperature: Number.NaN,
      outputGainDb: -8,
      sibilanceReduction: 0.8,
      artifactReduction: 0.65
    });

    await useWorkspaceStore.getState().startDubbing();

    const [[request]] = clientMocks.startDubbingJob.mock.calls;
    expect(request.lineOverrides[0].settings).toMatchObject({
      voiceMode: "design",
      instruct: "female, young adult, moderate pitch",
      speed: null,
      durationSeconds: 60,
      numStep: 8,
      guidanceScale: 10,
      positionTemperature: 0,
      classTemperature: 0,
      outputGainDb: -8,
      sibilanceReduction: 0.8,
      artifactReduction: 0.65
    });
  });

  it("does not split whole-file synthesis for notes-only line metadata", async () => {
    useWorkspaceStore.getState().setTargetText("Primeira linha.\nSegunda linha.");
    useWorkspaceStore.getState().updateSelectedLineMetadata({ notes: "observacao interna" });

    await useWorkspaceStore.getState().startDubbing();

    const [[request]] = clientMocks.startDubbingJob.mock.calls;
    expect(request.lineOverrides).toEqual([]);
  });

  it("persists selected synthesis controls as the next global default", async () => {
    useWorkspaceStore.getState().setTargetText("Linha com padrao novo.");
    useWorkspaceStore.getState().updateSelectedLineSettings({
      outputGainDb: -6,
      sibilanceReduction: 0.7,
      artifactReduction: 0.4
    });

    await useWorkspaceStore.getState().saveSelectedLineSettingsAsDefault();

    const [[savedConfig]] = clientMocks.saveConfig.mock.calls;
    expect(savedConfig.options.nativeSynthesis).toMatchObject({
      voiceMode: "clone",
      instruct: null,
      outputGainDb: -6,
      sibilanceReduction: 0.7,
      artifactReduction: 0.4
    });
    expect(useWorkspaceStore.getState().config.options.nativeSynthesis.outputGainDb).toBe(-6);
  });

  it("restores factory synthesis defaults for the selected line and global config", async () => {
    useWorkspaceStore.getState().setTargetText("Linha com ajuste para reset.");
    useWorkspaceStore.getState().updateSelectedLineSettings({
      outputGainDb: -6,
      sibilanceReduction: 0.7
    });

    await useWorkspaceStore.getState().resetSelectedLineSettingsToDefault();

    const [[savedConfig]] = clientMocks.saveConfig.mock.calls;
    expect(savedConfig.options.nativeSynthesis).toMatchObject({
      outputGainDb: 0,
      sibilanceReduction: 0,
      artifactReduction: 0
    });
    expect(
      useWorkspaceStore.getState().projectMetadata.files[fileA.name]?.lines["0"]?.settings
    ).toMatchObject({
      outputGainDb: 0,
      sibilanceReduction: 0,
      artifactReduction: 0
    });
  });

  it("releases central controls when a terminal job event arrives", () => {
    useWorkspaceStore.setState({ activeJobId: "job-1", isBusy: true, isCancelling: true });

    applyJobEvent(
      jobEvent({
        kind: "failed",
        stage: "failed"
      })
    );

    expect(useWorkspaceStore.getState().activeJobId).toBeNull();
    expect(useWorkspaceStore.getState().isBusy).toBe(false);
    expect(useWorkspaceStore.getState().isCancelling).toBe(false);
  });

  it("keeps execution logs timestamped and sorted newest first", () => {
    const store = useWorkspaceStore.getState();

    store.appendLog("Evento antigo.", "info", "2026-05-10T10:00:00.000Z");
    store.appendLog("Evento novo.", "success", "2026-05-10T10:01:00.000Z");
    store.appendLog("Evento intermediario.", "warning", "2026-05-10T10:00:30.000Z");

    expect(useWorkspaceStore.getState().logs).toMatchObject([
      { message: "Evento novo.", timestamp: "2026-05-10T10:01:00.000Z" },
      { message: "Evento intermediario.", timestamp: "2026-05-10T10:00:30.000Z" },
      { message: "Evento antigo.", timestamp: "2026-05-10T10:00:00.000Z" }
    ]);
  });

  it("reverts edited transcription fields to the selected file baseline", () => {
    useWorkspaceStore.setState({ files: [cachedDubbedFile], selectedPath: null });
    useWorkspaceStore.getState().selectFile(cachedDubbedFile.path);
    useWorkspaceStore.getState().setSourceText("Edited source text.");
    useWorkspaceStore.getState().setTargetText("Texto editado.");

    useWorkspaceStore.getState().revertTranscription();

    expect(useWorkspaceStore.getState().sourceText).toBe("Hello from cache.");
    expect(useWorkspaceStore.getState().targetText).toBe("Ola do cache.");
  });

  it("can revert to the original baseline after redubbing edited transcription", () => {
    useWorkspaceStore.setState({ files: [cachedDubbedFile], selectedPath: null });
    useWorkspaceStore.getState().selectFile(cachedDubbedFile.path);
    useWorkspaceStore.getState().setTargetText("Texto editado para redublagem.");

    applyJobEvent(
      jobEvent({
        kind: "transcription",
        filePath: cachedDubbedFile.path,
        sourceText: "Hello from cache.",
        targetText: "Texto editado para redublagem."
      })
    );
    applyJobEvent(
      jobEvent({
        kind: "file_complete",
        stage: "file_complete",
        filePath: cachedDubbedFile.path,
        outputPath: "E:\\audio\\saida\\line_d.wav",
        sourceText: "Hello from cache.",
        targetText: "Texto editado para redublagem."
      })
    );

    expect(
      useWorkspaceStore.getState().transcriptionBaselines[cachedDubbedFile.path]?.targetText
    ).toBe("Ola do cache.");

    useWorkspaceStore.getState().revertTranscription();

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

function jobEvent(patch: Partial<DubbingJobEvent>): DubbingJobEvent {
  return {
    jobId: "00000000-0000-0000-0000-000000000001",
    kind: "transcription",
    stage: null,
    timestamp: "2026-05-10T10:00:00.000Z",
    message: "Evento de teste.",
    progress: null,
    fileName: null,
    filePath: null,
    fileIndex: null,
    totalFiles: null,
    sourceText: null,
    targetText: null,
    outputPath: null,
    ...patch
  };
}
