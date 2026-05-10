import type { NativeSynthesisSettings, ProjectLineMetadata, VoiceMode } from "../tauri/types";
import { defaultOptions } from "../tauri/types";

export const nativeTagGroups = [
  {
    id: "expression",
    label: "Expressão",
    tags: ["[laughter]", "[sigh]"]
  },
  {
    id: "confirmation",
    label: "Confirmação",
    tags: ["[confirmation-en]"]
  },
  {
    id: "question",
    label: "Pergunta",
    tags: ["[question-en]", "[question-ah]", "[question-oh]", "[question-ei]", "[question-yi]"]
  },
  {
    id: "surprise",
    label: "Surpresa",
    tags: ["[surprise-ah]", "[surprise-oh]", "[surprise-wa]", "[surprise-yo]"]
  },
  {
    id: "dissatisfaction",
    label: "Insatisfação",
    tags: ["[dissatisfaction-hnn]"]
  }
] as const;

export const nativeTags = nativeTagGroups.flatMap((group) => group.tags);

export type NativeTag = (typeof nativeTags)[number];

export const nativeTagSet = new Set<string>(nativeTags);

export function isNativeTag(value: string): value is NativeTag {
  return nativeTagSet.has(value);
}

export const nativeTagDescriptions: Record<NativeTag, string> = {
  "[laughter]": "Risada natural ou deboche leve.",
  "[sigh]": "Suspiro de cansaço, alívio ou frustração.",
  "[confirmation-en]": "Confirmação curta, como um “hm-hm”.",
  "[question-en]": "Entonação interrogativa em inglês.",
  "[question-ah]": "Dúvida curta com som de “ah?”.",
  "[question-oh]": "Pergunta surpresa com som de “oh?”.",
  "[question-ei]": "Questionamento rápido, como “ei?”.",
  "[question-yi]": "Pergunta aguda ou hesitante.",
  "[surprise-ah]": "Surpresa aberta, como “ah!”.",
  "[surprise-oh]": "Surpresa ou espanto, como “oh!”.",
  "[surprise-wa]": "Reação de surpresa intensa.",
  "[surprise-yo]": "Surpresa com exclamação forte.",
  "[dissatisfaction-hnn]": "Insatisfação, reprovação ou resmungo."
};

export const defaultNativeSynthesisSettings: NativeSynthesisSettings = {
  ...defaultOptions.nativeSynthesis
};

export const omnivoiceMaxSynthesisSeconds = 30;

export const nativeSynthesisNumberControls = {
  speed: { min: 0.5, max: 2, step: 0.05 },
  durationSeconds: { min: 0.25, max: omnivoiceMaxSynthesisSeconds, step: 0.05 },
  numStep: { min: 8, max: 128, step: 1 },
  guidanceScale: { min: 0, max: 10, step: 0.1 },
  positionTemperature: { min: 0, max: 10, step: 0.1 },
  classTemperature: { min: 0, max: 10, step: 0.1 },
  loudnessMatchStrength: { min: 0, max: 1, step: 0.05 },
  outputGainDb: { min: -12, max: 12, step: 0.5 },
  sibilanceReduction: { min: 0, max: 1, step: 0.05 },
  artifactReduction: { min: 0, max: 1, step: 0.05 }
} as const;

type NativeSynthesisNumberControl = keyof typeof nativeSynthesisNumberControls;

export function normalizeNativeSynthesisSettings(
  settings: NativeSynthesisSettings
): NativeSynthesisSettings {
  const runtimeSettings: Partial<NativeSynthesisSettings> = settings;
  const voiceMode = normalizeVoiceMode(settings.voiceMode);
  const instruct = normalizeInstruct(voiceMode, settings.instruct);

  return {
    voiceMode,
    instruct,
    speed: normalizeOptionalNativeNumber(settings.speed, "speed"),
    durationSeconds: normalizeOptionalNativeNumber(settings.durationSeconds, "durationSeconds"),
    numStep: Math.round(normalizeNativeNumber(settings.numStep, "numStep")),
    guidanceScale: normalizeNativeNumber(settings.guidanceScale, "guidanceScale"),
    positionTemperature: normalizeNativeNumber(settings.positionTemperature, "positionTemperature"),
    classTemperature: normalizeNativeNumber(settings.classTemperature, "classTemperature"),
    denoise: runtimeSettings.denoise ?? defaultNativeSynthesisSettings.denoise,
    preprocessPrompt:
      runtimeSettings.preprocessPrompt ?? defaultNativeSynthesisSettings.preprocessPrompt,
    postprocessOutput:
      runtimeSettings.postprocessOutput ?? defaultNativeSynthesisSettings.postprocessOutput,
    matchSourceLoudness:
      runtimeSettings.matchSourceLoudness ?? defaultNativeSynthesisSettings.matchSourceLoudness,
    loudnessMatchStrength: normalizeNativeNumber(
      settings.loudnessMatchStrength,
      "loudnessMatchStrength"
    ),
    outputGainDb: normalizeNativeNumber(settings.outputGainDb, "outputGainDb"),
    sibilanceReduction: normalizeNativeNumber(settings.sibilanceReduction, "sibilanceReduction"),
    artifactReduction: normalizeNativeNumber(settings.artifactReduction, "artifactReduction")
  };
}

export function nativeSynthesisSettingsEqual(
  first: NativeSynthesisSettings,
  second: NativeSynthesisSettings
): boolean {
  return (
    JSON.stringify(normalizeNativeSynthesisSettings(first)) ===
    JSON.stringify(normalizeNativeSynthesisSettings(second))
  );
}

function normalizeOptionalNativeNumber(
  value: number | null,
  control: NativeSynthesisNumberControl
): number | null {
  return value === null || !Number.isFinite(value) ? null : normalizeNativeNumber(value, control);
}

function normalizeNativeNumber(value: number, control: NativeSynthesisNumberControl): number {
  const spec = nativeSynthesisNumberControls[control];
  if (!Number.isFinite(value)) {
    return nativeSynthesisDefaultNumber(control);
  }
  return Math.min(spec.max, Math.max(spec.min, value));
}

function nativeSynthesisDefaultNumber(control: NativeSynthesisNumberControl): number {
  const value = defaultNativeSynthesisSettings[control];
  return typeof value === "number" ? value : nativeSynthesisNumberControls[control].min;
}

function normalizeVoiceMode(value: unknown): VoiceMode {
  return value === "clone" || value === "design" || value === "auto"
    ? value
    : defaultNativeSynthesisSettings.voiceMode;
}

function normalizeInstruct(voiceMode: VoiceMode, value: string | null): string | null {
  if (voiceMode !== "design") {
    return null;
  }

  const trimmed = value?.trim() ?? "";
  return trimmed.length > 0 ? trimmed : "female, young adult, moderate pitch";
}

export interface TextSegment {
  value: string;
  kind: "text" | "tag";
}

export function splitLines(value: string): string[] {
  return value.length === 0 ? [""] : value.split(/\r?\n/u);
}

export function replaceLine(value: string, lineIndex: number, nextLine: string): string {
  const lines = splitLines(value);
  lines[lineIndex] = nextLine;
  return lines.join("\n");
}

export function insertTagIntoLine(value: string, lineIndex: number, tag: NativeTag): string {
  const lines = splitLines(value);
  const current = lines[lineIndex] ?? "";
  const separator = current.trim().length > 0 && !current.endsWith(" ") ? " " : "";
  lines[lineIndex] = `${current}${separator}${tag} `;
  return lines.join("\n");
}

export function removeNativeTagsFromLine(value: string): string {
  let sanitized = value;
  for (const tag of nativeTags) {
    sanitized = sanitized.split(tag).join(" ");
  }
  return sanitized.replace(/^[ \t]+/u, "").replace(/[ \t]+([,.;:!?])/gu, "$1");
}

export function removeNativeTagsFromText(value: string): string {
  return splitLines(value).map(removeNativeTagsFromLine).join("\n");
}

export function textSegments(value: string): TextSegment[] {
  const segments: TextSegment[] = [];
  const pattern = /\[[^\]]+\]/gu;
  let cursor = 0;
  for (const match of value.matchAll(pattern)) {
    const index = match.index;
    if (index > cursor) {
      segments.push({ kind: "text", value: value.slice(cursor, index) });
    }
    const token = match[0];
    segments.push({
      kind: nativeTagSet.has(token) ? "tag" : "text",
      value: token
    });
    cursor = index + token.length;
  }
  if (cursor < value.length) {
    segments.push({ kind: "text", value: value.slice(cursor) });
  }
  return segments.length > 0 ? segments : [{ kind: "text", value: "" }];
}

export function unknownNativeTags(value: string): string[] {
  const tags = new Set<string>();
  const pattern = /\[([a-z][a-z0-9-]*)\]/gu;
  for (const match of value.matchAll(pattern)) {
    const token = `[${match[1]}]`;
    if (!nativeTagSet.has(token)) {
      tags.add(token);
    }
  }
  return Array.from(tags);
}

export function tagsInText(value: string): NativeTag[] {
  return textSegments(value)
    .filter((segment): segment is TextSegment & { kind: "tag"; value: NativeTag } => {
      return segment.kind === "tag" && nativeTagSet.has(segment.value);
    })
    .map((segment) => segment.value);
}

export function tagsByLine(value: string): NativeTag[][] {
  return splitLines(value).map(tagsInText);
}

export function mergeNativeTags(
  currentTags: readonly string[],
  incomingTags: readonly NativeTag[]
): NativeTag[] {
  const merged = new Set<NativeTag>();
  for (const tag of currentTags) {
    if (isNativeTag(tag)) {
      merged.add(tag);
    }
  }
  for (const tag of incomingTags) {
    merged.add(tag);
  }
  return Array.from(merged);
}

export function createLineMetadata(
  line: string,
  baseSettings: NativeSynthesisSettings = defaultNativeSynthesisSettings
): ProjectLineMetadata {
  return {
    tags: tagsInText(line),
    characterId: null,
    notes: null,
    settings: normalizeNativeSynthesisSettings(baseSettings)
  };
}

export function hasLineSpecificSynthesis(metadata: ProjectLineMetadata | undefined): boolean {
  if (!metadata) {
    return false;
  }
  return (
    metadata.tags.length > 0 ||
    metadata.characterId !== null ||
    !nativeSynthesisSettingsEqual(metadata.settings, defaultNativeSynthesisSettings)
  );
}
