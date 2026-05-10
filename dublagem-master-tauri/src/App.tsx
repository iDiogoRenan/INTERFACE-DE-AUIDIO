import * as Tabs from "@radix-ui/react-tabs";
import { Settings, ShieldCheck, SlidersHorizontal } from "lucide-react";
import { useEffect } from "react";
import { DubbingPanel } from "./features/dubbing/DubbingPanel";
import { ProjectExplorer } from "./features/project-explorer/ProjectExplorer";
import { SettingsPanel } from "./features/settings/SettingsPanel";
import { ValidationPanel } from "./features/validation/ValidationPanel";
import { APP_DISPLAY_NAME } from "./shared/app/metadata";
import {
  activeAsrModel,
  availableSpeechModels,
  configWithActiveSpeechModel,
  modelDescriptor,
  modelRuntimeLabel,
  modelRuntimeState
} from "./shared/speechModels";
import { isTauriRuntime } from "./shared/tauri/client";
import type { AppConfig, JobStage, SpeechModelId } from "./shared/tauri/types";
import { useWorkspaceStore } from "./stores/workspaceStore";
import styles from "./App.module.css";

const WINDOW_SIZE_STORAGE_KEY = "nsg-gaming-dub.window-size.v1";
const WINDOW_SIZE_SAVE_DELAY_MS = 180;
const MIN_WINDOW_WIDTH = 1120;
const MIN_WINDOW_HEIGHT = 720;
const MAX_WINDOW_DIMENSION = 10000;

interface StoredWindowSize {
  width: number;
  height: number;
}

function App() {
  const load = useWorkspaceStore((state) => state.load);
  const config = useWorkspaceStore((state) => state.config);
  const saveConfig = useWorkspaceStore((state) => state.saveConfig);
  const isBusy = useWorkspaceStore((state) => state.isBusy);
  const currentStage = useWorkspaceStore((state) => state.currentStage);
  const appendLog = useWorkspaceStore((state) => state.appendLog);

  useEffect(() => {
    void load().catch((error: unknown) => {
      appendLog(
        error instanceof Error ? error.message : "Falha ao carregar configuração.",
        "error"
      );
    });
  }, [appendLog, load]);

  useEffect(() => {
    document.title = APP_DISPLAY_NAME;
  }, []);

  usePersistentWindowSize();

  return (
    <main className={styles.shell}>
      <ProjectExplorer />
      <Tabs.Root className={styles.workspace} defaultValue="dubbing">
        <header className={styles.topbar}>
          <div>
            <h1>{APP_DISPLAY_NAME}</h1>
            <p>Fluxo local em Rust para transcrição, tradução, síntese e validação.</p>
            <ModelHeaderControl
              config={config}
              currentStage={currentStage}
              isBusy={isBusy}
              onModelChange={(modelId) => {
                if (isBusy) {
                  appendLog("Troca de modelo bloqueada durante geração.", "warning");
                  return;
                }
                void saveConfig(configWithActiveSpeechModel(config, modelId)).catch(
                  (unknownError: unknown) => {
                    appendLog(
                      unknownError instanceof Error
                        ? unknownError.message
                        : "Falha ao salvar modelo ativo.",
                      "error"
                    );
                  }
                );
              }}
            />
          </div>
          <Tabs.List className={styles.tabs}>
            <Tabs.Trigger value="dubbing">
              <SlidersHorizontal size={16} />
              Dublagem
            </Tabs.Trigger>
            <Tabs.Trigger value="validation">
              <ShieldCheck size={16} />
              Validação
            </Tabs.Trigger>
            <Tabs.Trigger value="settings">
              <Settings size={16} />
              Ajustes
            </Tabs.Trigger>
          </Tabs.List>
        </header>

        <Tabs.Content className={styles.content} value="dubbing">
          <DubbingPanel />
        </Tabs.Content>
        <Tabs.Content className={styles.content} value="validation">
          <ValidationPanel />
        </Tabs.Content>
        <Tabs.Content className={styles.content} value="settings">
          <SettingsPanel />
        </Tabs.Content>
      </Tabs.Root>
    </main>
  );
}

interface ModelHeaderControlProps {
  config: AppConfig;
  currentStage: JobStage | null;
  isBusy: boolean;
  onModelChange: (modelId: SpeechModelId) => void;
}

function ModelHeaderControl({
  config,
  currentStage,
  isBusy,
  onModelChange
}: ModelHeaderControlProps) {
  const activeModel = modelDescriptor(config.activeSpeechModel);
  const runtimeState = modelRuntimeState(isBusy, currentStage);

  return (
    <div className={styles.modelControl} aria-label="Modelo de síntese ativo">
      <label>
        <span>Modelo</span>
        <select
          value={config.activeSpeechModel}
          disabled={isBusy}
          onChange={(event) => {
            onModelChange(event.currentTarget.value as SpeechModelId);
          }}
        >
          {availableSpeechModels.map((model) => (
            <option key={model.id} value={model.id}>
              {model.label}
            </option>
          ))}
        </select>
      </label>
      <dl>
        <div>
          <dt>TTS engine</dt>
          <dd>{activeModel.engine}</dd>
        </div>
        <div>
          <dt>TTS versão</dt>
          <dd>{activeModel.version}</dd>
        </div>
        <div>
          <dt>ASR</dt>
          <dd>{activeAsrModel.label}</dd>
        </div>
        <div>
          <dt>ASR modelo</dt>
          <dd>{activeAsrModel.model}</dd>
        </div>
        <div>
          <dt>ASR engine</dt>
          <dd>{activeAsrModel.engine}</dd>
        </div>
        <div>
          <dt>VAD</dt>
          <dd>{activeAsrModel.vadModel}</dd>
        </div>
      </dl>
      <span className={styles.modelStatus} data-state={runtimeState}>
        {modelRuntimeLabel(runtimeState)}
      </span>
    </div>
  );
}

function usePersistentWindowSize(): void {
  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }

    const abortController = new AbortController();
    let unlistenResize: (() => void) | null = null;
    let unlistenClose: (() => void) | null = null;
    let saveTimer: number | null = null;
    let lastKnownSize: StoredWindowSize | null = null;

    const clearSaveTimer = () => {
      if (saveTimer === null) {
        return;
      }
      window.clearTimeout(saveTimer);
      saveTimer = null;
    };

    void (async () => {
      const [{ getCurrentWindow }, { PhysicalSize }] = await Promise.all([
        import("@tauri-apps/api/window"),
        import("@tauri-apps/api/dpi")
      ]);

      if (abortController.signal.aborted) {
        return;
      }

      const appWindow = getCurrentWindow();
      const storedSize = readStoredWindowSize();
      if (storedSize) {
        try {
          await appWindow.setSize(new PhysicalSize(storedSize.width, storedSize.height));
        } catch {
          localStorage.removeItem(WINDOW_SIZE_STORAGE_KEY);
        }
      }

      const nextUnlistenResize = await appWindow.onResized(({ payload }) => {
        lastKnownSize = normalizeWindowSize(payload);
        clearSaveTimer();
        saveTimer = window.setTimeout(() => {
          if (lastKnownSize) {
            writeStoredWindowSize(lastKnownSize);
          }
          saveTimer = null;
        }, WINDOW_SIZE_SAVE_DELAY_MS);
      });
      const nextUnlistenClose = await appWindow.onCloseRequested(() => {
        clearSaveTimer();
        if (lastKnownSize) {
          writeStoredWindowSize(lastKnownSize);
        }
      });

      unlistenResize = nextUnlistenResize;
      unlistenClose = nextUnlistenClose;
      lastKnownSize = normalizeWindowSize(await appWindow.innerSize());
      if (lastKnownSize) {
        writeStoredWindowSize(lastKnownSize);
      }
    })().catch(() => {
      localStorage.removeItem(WINDOW_SIZE_STORAGE_KEY);
    });

    return () => {
      abortController.abort();
      clearSaveTimer();
      unlistenResize?.();
      unlistenClose?.();
    };
  }, []);
}

function readStoredWindowSize(): StoredWindowSize | null {
  const serializedSize = localStorage.getItem(WINDOW_SIZE_STORAGE_KEY);
  if (!serializedSize) {
    return null;
  }

  try {
    const parsedSize: unknown = JSON.parse(serializedSize);
    return normalizeWindowSize(parsedSize);
  } catch {
    localStorage.removeItem(WINDOW_SIZE_STORAGE_KEY);
    return null;
  }
}

function writeStoredWindowSize(size: StoredWindowSize): void {
  const normalizedSize = normalizeWindowSize(size);
  if (!normalizedSize) {
    return;
  }

  try {
    localStorage.setItem(WINDOW_SIZE_STORAGE_KEY, JSON.stringify(normalizedSize));
  } catch {
    return;
  }
}

function normalizeWindowSize(value: unknown): StoredWindowSize | null {
  if (!isWindowSizeRecord(value)) {
    return null;
  }

  const width = value.width;
  const height = value.height;
  if (typeof width !== "number" || typeof height !== "number") {
    return null;
  }
  if (!Number.isFinite(width) || !Number.isFinite(height)) {
    return null;
  }

  return {
    width: clampWindowDimension(width, MIN_WINDOW_WIDTH),
    height: clampWindowDimension(height, MIN_WINDOW_HEIGHT)
  };
}

function isWindowSizeRecord(value: unknown): value is Record<keyof StoredWindowSize, unknown> {
  return typeof value === "object" && value !== null && "width" in value && "height" in value;
}

function clampWindowDimension(value: number, minimum: number): number {
  return Math.min(MAX_WINDOW_DIMENSION, Math.max(minimum, Math.round(value)));
}

export default App;
