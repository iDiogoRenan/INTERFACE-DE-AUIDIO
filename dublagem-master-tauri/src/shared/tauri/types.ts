export type LanguageCode = "auto" | "en" | "pt" | "fr" | "sv";
export type DubbingMode = "classico" | "antisotaque";
export type VoiceMode = "clone" | "design" | "auto";
export type SpeechModelId = "omnivoice";
export type AudioFileStatus =
  | "pending"
  | "dubbed"
  | "approved"
  | "rejected"
  | "ignored"
  | "awaiting_confirmation"
  | "cancelled"
  | "chunk_limit_exceeded"
  | "batch_processed"
  | "missing_source"
  | "failed";
export type ChunkLimitPolicy =
  | "warn_and_continue"
  | "process_in_batches"
  | "require_confirmation"
  | "resegment_first"
  | "cancel_with_record";
export type TimingChunkStatus =
  | "ok"
  | "time_stretched"
  | "regenerated"
  | "text_adapted"
  | "out_of_limit"
  | "needs_manual_review"
  | "overlap_risk"
  | "abrupt_ending_detected"
  | "bad_reference"
  | "tts_failed"
  | "chunk_limit_exceeded"
  | "awaiting_confirmation"
  | "batch_processed";
export type TimingAdjustmentAction =
  | "accepted"
  | "time_stretched"
  | "text_adapted"
  | "regenerated"
  | "fade_applied"
  | "loudness_normalized"
  | "tail_preserved"
  | "batch_queued"
  | "manual_review_required";
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
  maxSynthesisChunks: number;
  preserveSentenceBoundaries: boolean;
  nativeSynthesis: NativeSynthesisSettings;
  timingAlignment: TimingAlignmentOptions;
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

export interface SpeechModelPreset {
  nativeSynthesis: NativeSynthesisSettings;
}

export interface TimingAlignmentOptions {
  acceptDurationDiffPercent: number;
  lightStretchDiffPercent: number;
  maxStretchDiffPercent: number;
  maxRegenerationAttempts: number;
  autoTextAdaptation: boolean;
  preserveOriginalPauses: boolean;
  preventOverlap: boolean;
  fadeOutMs: number;
  crossfadeMs: number;
  normalizeLoudness: boolean;
  blockExportOnCriticalChunks: boolean;
  minTailMs: number;
  chunkLimitPolicy: ChunkLimitPolicy;
}

export interface AppConfig {
  inputDir: string | null;
  outputDir: string | null;
  guideAudio: string | null;
  approvedDir: string | null;
  modelDir: string | null;
  voicePoolDir: string | null;
  activeSpeechModel: SpeechModelId;
  speechModelPresets: Record<SpeechModelId, SpeechModelPreset | undefined>;
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
  outputPath: string | null;
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
  saveOutputAs: string | null;
  guideAudio: string | null;
  modelDir: string | null;
  options: DubbingOptions;
  customSourceText: string | null;
  customTargetText: string | null;
  pinnedTags: string[];
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
  pinnedNativeTags: string[];
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
  timestamp: string;
  message: string;
  progress: number | null;
  fileName: string | null;
  filePath: string | null;
  fileIndex: number | null;
  totalFiles: number | null;
  sourceText: string | null;
  targetText: string | null;
  outputPath: string | null;
  outputStatus: AudioFileStatus | null;
  alignmentReport: TimingAlignmentReport | null;
}

export interface TimingAlignmentChunkReport {
  segmentId: string;
  audioId: string;
  chunkIndex: number;
  totalChunks: number;
  startOriginal: number;
  endOriginal: number;
  durationOriginal: number;
  textoOriginalEn: string;
  textoPtbr: string;
  originalSegmentPath: string | null;
  dubbedSegmentPath: string | null;
  durationGenerated: number | null;
  durationDifferencePercent: number | null;
  statuses: TimingChunkStatus[];
  actionsApplied: TimingAdjustmentAction[];
  modelUsed: SpeechModelId;
  attempts: number;
  failureReason: string | null;
  stretchRatio: number | null;
  overlapSeconds: number | null;
  abruptEndingDetected: boolean;
}

export interface TimingAlignmentReport {
  audioId: string;
  fileName: string;
  modelUsed: SpeechModelId;
  totalChunks: number;
  configuredChunkLimit: number;
  chunkLimitPolicy: ChunkLimitPolicy;
  chunkLimitExceeded: boolean;
  processedInBatches: boolean;
  hasCriticalChunks: boolean;
  warnings: string[];
  chunks: TimingAlignmentChunkReport[];
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
  maxSynthesisChunks: 1,
  preserveSentenceBoundaries: false,
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
  },
  timingAlignment: {
    acceptDurationDiffPercent: 5,
    lightStretchDiffPercent: 10,
    maxStretchDiffPercent: 20,
    maxRegenerationAttempts: 3,
    autoTextAdaptation: true,
    preserveOriginalPauses: true,
    preventOverlap: true,
    fadeOutMs: 50,
    crossfadeMs: 35,
    normalizeLoudness: true,
    blockExportOnCriticalChunks: true,
    minTailMs: 200,
    chunkLimitPolicy: "process_in_batches"
  }
};
