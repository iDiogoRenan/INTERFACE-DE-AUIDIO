export type LanguageCode = "auto" | "en" | "pt" | "fr" | "sv";
export type DubbingMode = "classico" | "antisotaque";
export type AudioFileStatus =
  | "pending"
  | "dubbed"
  | "approved"
  | "rejected"
  | "missing_source"
  | "failed";
export type JobEventKind =
  | "stage"
  | "transcription"
  | "progress"
  | "log"
  | "file_complete"
  | "cancelled"
  | "finished"
  | "failed";
export type JobStage =
  | "queued"
  | "loading_models"
  | "preparing_file"
  | "transcribing"
  | "transcribed"
  | "translating"
  | "translated"
  | "synthesizing"
  | "writing_output"
  | "file_complete"
  | "cancelling"
  | "cancelled"
  | "finished"
  | "failed";

export interface DubbingOptions {
  sourceLanguage: LanguageCode;
  targetLanguage: LanguageCode;
  mode: DubbingMode;
  palatalize: boolean;
  commaBeforeQuestion: boolean;
  trailingPeriod: boolean;
  padMs: number;
  omniTemperature: number;
}

export interface AppConfig {
  inputDir: string | null;
  outputDir: string | null;
  guideAudio: string | null;
  approvedDir: string | null;
  modelDir: string | null;
  voicePoolDir: string | null;
  options: DubbingOptions;
}

export interface AudioMetadata {
  durationSeconds: number | null;
  sampleRate: number | null;
  channels: number | null;
  format: string;
}

export interface AudioFileEntry {
  name: string;
  path: string;
  family: string;
  status: AudioFileStatus;
  metadata: AudioMetadata | null;
  transcription: CachedTranscription | null;
}

export interface CachedTranscription {
  sourceText: string;
  targetText: string;
}

export interface TranscriptionResult {
  sourceText: string;
  targetText: string;
  sourceLanguage: LanguageCode;
  targetLanguage: LanguageCode;
}

export interface TranslationRequest {
  text: string;
  sourceLanguage: LanguageCode;
  targetLanguage: LanguageCode;
}

export interface TranslationResult {
  translatedText: string;
  provider: string;
}

export interface DubbingRequest {
  inputPaths: string[];
  outputDir: string;
  guideAudio: string | null;
  modelDir: string | null;
  options: DubbingOptions;
  customSourceText: string | null;
  customTargetText: string | null;
}

export interface DubbingJobEvent {
  jobId: string;
  kind: JobEventKind;
  stage: JobStage | null;
  message: string;
  progress: number | null;
  fileName: string | null;
  filePath: string | null;
  fileIndex: number | null;
  totalFiles: number | null;
  sourceText: string | null;
  targetText: string | null;
  outputPath: string | null;
}

export const defaultOptions: DubbingOptions = {
  sourceLanguage: "auto",
  targetLanguage: "pt",
  mode: "classico",
  palatalize: false,
  commaBeforeQuestion: false,
  trailingPeriod: false,
  padMs: 200,
  omniTemperature: 0
};
