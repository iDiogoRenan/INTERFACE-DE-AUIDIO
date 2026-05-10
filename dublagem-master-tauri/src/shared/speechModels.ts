import {
  defaultOptions,
  type AppConfig,
  type JobStage,
  type NativeSynthesisSettings,
  type SpeechModelId,
  type SpeechModelPreset
} from "./tauri/types";
import { normalizeNativeSynthesisSettings } from "./omnivoice/nativeControls";

export interface SpeechModelDescriptor {
  id: SpeechModelId;
  label: string;
  engine: string;
  version: string;
}

export interface AsrModelDescriptor {
  label: string;
  engine: string;
  model: string;
  vadModel: string;
}

export type ModelRuntimeState = "loading" | "ready" | "generating" | "error";

const omnivoiceDescriptor: SpeechModelDescriptor = {
  id: "omnivoice",
  label: "OmniVoice",
  engine: "Candle CUDA",
  version: "0.1.5"
};

export const availableSpeechModels: readonly SpeechModelDescriptor[] = [omnivoiceDescriptor];

export const activeAsrModel: AsrModelDescriptor = {
  label: "Whisper",
  engine: "whisper-rs",
  model: "large-v3",
  vadModel: "Silero VAD v6.2.0"
};

export const defaultSpeechModelPresets: Record<SpeechModelId, SpeechModelPreset> = {
  omnivoice: {
    nativeSynthesis: defaultOptions.nativeSynthesis
  }
};

export function activeNativeSynthesisSettings(config: AppConfig): NativeSynthesisSettings {
  return normalizeNativeSynthesisSettings(
    config.speechModelPresets[config.activeSpeechModel]?.nativeSynthesis ??
      config.options.nativeSynthesis
  );
}

export function configWithActiveSpeechModel(
  config: AppConfig,
  activeSpeechModel: SpeechModelId
): AppConfig {
  const preset = speechModelPreset(config, activeSpeechModel);
  return {
    ...config,
    activeSpeechModel,
    speechModelPresets: normalizeSpeechModelPresets(config.speechModelPresets),
    options: {
      ...config.options,
      nativeSynthesis: preset.nativeSynthesis
    }
  };
}

export function configWithActiveNativeSynthesis(
  config: AppConfig,
  nativeSynthesis: NativeSynthesisSettings
): AppConfig {
  const normalizedSynthesis = normalizeNativeSynthesisSettings(nativeSynthesis);
  return {
    ...config,
    speechModelPresets: {
      ...normalizeSpeechModelPresets(config.speechModelPresets),
      [config.activeSpeechModel]: {
        nativeSynthesis: normalizedSynthesis
      }
    },
    options: {
      ...config.options,
      nativeSynthesis: normalizedSynthesis
    }
  };
}

export function modelDescriptor(modelId: SpeechModelId): SpeechModelDescriptor {
  const descriptors: Record<SpeechModelId, SpeechModelDescriptor> = {
    omnivoice: omnivoiceDescriptor
  };
  return descriptors[modelId];
}

export function modelRuntimeState(
  isBusy: boolean,
  currentStage: JobStage | null
): ModelRuntimeState {
  if (currentStage === "failed") {
    return "error";
  }
  if (currentStage === "loading_models") {
    return "loading";
  }
  if (isBusy) {
    return "generating";
  }
  return "ready";
}

export function modelRuntimeLabel(state: ModelRuntimeState): string {
  const labels: Record<ModelRuntimeState, string> = {
    loading: "carregando",
    ready: "pronto",
    generating: "gerando",
    error: "erro"
  };
  return labels[state];
}

function speechModelPreset(config: AppConfig, modelId: SpeechModelId): SpeechModelPreset {
  return normalizeSpeechModelPresets(config.speechModelPresets)[modelId];
}

function normalizeSpeechModelPresets(
  presets: AppConfig["speechModelPresets"]
): Record<SpeechModelId, SpeechModelPreset> {
  return {
    omnivoice: {
      nativeSynthesis: normalizeNativeSynthesisSettings(
        presets.omnivoice?.nativeSynthesis ?? defaultSpeechModelPresets.omnivoice.nativeSynthesis
      )
    }
  };
}
