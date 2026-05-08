import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  AudioFileEntry,
  AudioMetadata,
  DubbingRequest,
  LanguageCode,
  TranslationRequest,
  TranslationResult,
  TranscriptionResult
} from "./types";
import { defaultOptions } from "./types";

type CommandArgs = Record<string, unknown>;

function command<TResponse>(name: string, args?: CommandArgs): Promise<TResponse> {
  if (!isTauriRuntime()) {
    return Promise.reject(new Error("Runtime Tauri indisponivel no preview web."));
  }
  return invoke<TResponse>(name, args);
}

async function runCommand(name: string, args?: CommandArgs): Promise<void> {
  await command<unknown>(name, args);
}

export const tauriClient = {
  loadConfig: () =>
    isTauriRuntime() ? command<AppConfig>("load_config") : Promise.resolve(defaultConfig),
  saveConfig: (config: AppConfig) => command<AppConfig>("save_config", { config }),
  scanAudioFolder: (inputDir: string, outputDir: string | null) =>
    command<AudioFileEntry[]>("scan_audio_folder", { inputDir, outputDir }),
  getAudioMetadata: (path: string) => command<AudioMetadata>("get_audio_metadata", { path }),
  prepareAudioPreview: (source: string) => command<string>("prepare_audio_preview", { source }),
  transcribeAudio: (path: string, sourceLanguage: LanguageCode, targetLanguage: LanguageCode) =>
    command<TranscriptionResult>("transcribe_audio", { path, sourceLanguage, targetLanguage }),
  translateText: (request: TranslationRequest) =>
    command<TranslationResult>("translate_text", { request }),
  startDubbingJob: (request: DubbingRequest) => command<string>("start_dubbing_job", { request }),
  cancelJob: (jobId: string) => runCommand("cancel_job", { jobId }),
  approveFile: (source: string, approvedDir: string) =>
    command<string>("approve_file", { source, approvedDir }),
  rejectFile: (source: string, rejectedDir: string) =>
    command<string>("reject_file", { source, rejectedDir }),
  generateVoicePool: (outputDir: string) => command<string[]>("generate_voice_pool", { outputDir })
};

const defaultConfig: AppConfig = {
  inputDir: null,
  outputDir: null,
  guideAudio: null,
  approvedDir: null,
  modelDir: null,
  voicePoolDir: "voice_pool_ptbr",
  options: defaultOptions
};

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined;
}
