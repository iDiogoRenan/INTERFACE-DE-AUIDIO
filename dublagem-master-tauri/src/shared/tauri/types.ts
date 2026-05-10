export type LanguageCode = "auto" | "en" | "pt" | "fr" | "sv";
export type DubbingMode = "classico" | "antisotaque";
export type VoiceMode = "clone" | "design" | "auto";
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
  nativeSynthesis: NativeSynthesisSettings;
}

export interface NativeSynthesisSettings {
  voiceMode: VoiceMode;
  instruct: string | null;
  speed: number | null;
  durationSeconds: number | null;
  numStep: number;
  guidanceScale: number;
  positionTemperature: number;
  classTemperature: number;
  denoise: boolean;
  preprocessPrompt: boolean;
  postprocessOutput: boolean;
  matchSourceLoudness: boolean;
  loudnessMatchStrength: number;
  outputGainDb: number;
  sibilanceReduction: number;
  artifactReduction: number;
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
  lineOverrides: LineSynthesisOverride[];
}

export interface LineSynthesisOverride {
  lineIndex: number;
  targetText: string;
  tags: string[];
  settings: NativeSynthesisSettings;
}

export interface SynthesisLinePreviewRequest {
  sourceAudio: string;
  text: string;
  tags: string[];
  settings: NativeSynthesisSettings;
}

export interface ProjectMetadata {
  version: number;
  files: Record<string, ProjectFileMetadata | undefined>;
}

export interface ProjectFileMetadata {
  sourceText: string | null;
  targetText: string | null;
  baselineSourceText: string | null;
  baselineTargetText: string | null;
  lines: Record<string, ProjectLineMetadata | undefined>;
}

export interface ProjectLineMetadata {
  tags: string[];
  characterId: string | null;
  notes: string | null;
  settings: NativeSynthesisSettings;
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
  sourceLanguage: "en",
  targetLanguage: "pt",
  mode: "classico",
  palatalize: false,
  commaBeforeQuestion: false,
  trailingPeriod: false,
  padMs: 200,
  omniTemperature: 0,
  nativeSynthesis: {
    voiceMode: "clone",
    instruct: null,
    speed: null,
    durationSeconds: null,
    numStep: 48,
    guidanceScale: 2,
    positionTemperature: 1,
    classTemperature: 0,
    denoise: true,
    preprocessPrompt: true,
    postprocessOutput: true,
    matchSourceLoudness: false,
    loudnessMatchStrength: 0.85,
    outputGainDb: 0,
    sibilanceReduction: 0,
    artifactReduction: 0
  }
};
