import * as Checkbox from "@radix-ui/react-checkbox";
import { Check, Heart, Save } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { PathField } from "../../shared/ui/PathField";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { isTauriRuntime } from "../../shared/tauri/client";
import type { AppConfig, ChunkLimitPolicy, LanguageCode } from "../../shared/tauri/types";
import styles from "./SettingsPanel.module.css";

const authorGithubUrl = "https://github.com/iDiogoRenan";
const sourceLanguages: LanguageCode[] = ["auto", "en", "fr", "sv", "pt"];
const targetLanguages: LanguageCode[] = ["pt", "fr", "sv", "en"];
const synthesisChunkLimits = {
  min: 1,
  max: 20,
  seconds: 30
} as const;
const chunkLimitPolicies: ChunkLimitPolicy[] = [
  "process_in_batches",
  "warn_and_continue",
  "require_confirmation",
  "resegment_first",
  "cancel_with_record"
];

export function SettingsPanel() {
  const config = useWorkspaceStore((state) => state.config);
  const saveConfig = useWorkspaceStore((state) => state.saveConfig);
  const scan = useWorkspaceStore((state) => state.scan);
  const appendLog = useWorkspaceStore((state) => state.appendLog);

  async function patchConfig(patch: Partial<AppConfig>): Promise<void> {
    await saveConfig({ ...config, ...patch });
  }

  async function saveAndScan(nextConfig: AppConfig): Promise<void> {
    await saveConfig(nextConfig);
    await scan();
  }

  function reportPathOpenError(message: string): void {
    appendLog(message, "error");
  }

  return (
    <section className={styles.panel}>
      <div className={styles.pathGrid}>
        <PathField
          label="Origem"
          value={config.inputDir}
          mode="directory"
          placeholder="Pasta com áudios originais"
          onOpenError={reportPathOpenError}
          onChange={(inputDir) => {
            void saveAndScan({ ...config, inputDir });
          }}
        />
        <PathField
          label="Destino"
          value={config.outputDir}
          mode="directory"
          placeholder="Pasta para dublagens"
          onOpenError={reportPathOpenError}
          onChange={(outputDir) => {
            void patchConfig({ outputDir });
          }}
        />
        <PathField
          label="Áudio guia"
          value={config.guideAudio}
          mode="file"
          placeholder="Arquivo de referência opcional"
          onOpenError={reportPathOpenError}
          onChange={(guideAudio) => {
            void patchConfig({ guideAudio });
          }}
        />
        <PathField
          label="Aprovados"
          value={config.approvedDir}
          mode="directory"
          placeholder="Pasta de aprovação final"
          onOpenError={reportPathOpenError}
          onChange={(approvedDir) => {
            void patchConfig({ approvedDir });
          }}
        />
        <PathField
          label="Modelos"
          value={config.modelDir}
          mode="directory"
          placeholder="Pasta local de modelos"
          onOpenError={reportPathOpenError}
          onChange={(modelDir) => {
            void patchConfig({ modelDir });
          }}
        />
      </div>

      <div className={styles.optionsGrid}>
        <label>
          Origem
          <select
            value={config.options.sourceLanguage}
            onChange={(event) => {
              void patchConfig({
                options: {
                  ...config.options,
                  sourceLanguage: event.currentTarget.value as LanguageCode
                }
              });
            }}
          >
            {sourceLanguages.map((language) => (
              <option key={language} value={language}>
                {languageLabel(language)}
              </option>
            ))}
          </select>
        </label>
        <label>
          Destino
          <select
            value={config.options.targetLanguage}
            onChange={(event) => {
              void patchConfig({
                options: {
                  ...config.options,
                  targetLanguage: event.currentTarget.value as LanguageCode
                }
              });
            }}
          >
            {targetLanguages.map((language) => (
              <option key={language} value={language}>
                {languageLabel(language)}
              </option>
            ))}
          </select>
        </label>
        <label>
          Margem final
          <input
            type="number"
            min={0}
            max={2000}
            step={10}
            value={config.options.padMs}
            onChange={(event) => {
              void patchConfig({
                options: { ...config.options, padMs: Number(event.currentTarget.value) }
              });
            }}
          />
        </label>
        <label>
          Limite sugerido de chunks ({synthesisChunkLimits.seconds}s)
          <input
            type="number"
            min={synthesisChunkLimits.min}
            max={synthesisChunkLimits.max}
            step={1}
            value={config.options.maxSynthesisChunks}
            onChange={(event) => {
              void patchConfig({
                options: {
                  ...config.options,
                  maxSynthesisChunks: normalizeMaxSynthesisChunks(event.currentTarget.value)
                }
              });
            }}
          />
        </label>
        <label>
          Excesso de chunks
          <select
            value={config.options.timingAlignment.chunkLimitPolicy}
            onChange={(event) => {
              void patchConfig({
                options: {
                  ...config.options,
                  timingAlignment: {
                    ...config.options.timingAlignment,
                    chunkLimitPolicy: event.currentTarget.value as ChunkLimitPolicy
                  }
                }
              });
            }}
          >
            {chunkLimitPolicies.map((policy) => (
              <option key={policy} value={policy}>
                {chunkLimitPolicyLabel(policy)}
              </option>
            ))}
          </select>
        </label>
        <label>
          Aceitar diferença até
          <input
            type="number"
            min={0}
            max={20}
            step={0.5}
            value={config.options.timingAlignment.acceptDurationDiffPercent}
            onChange={(event) => {
              void patchTimingAlignment(config, patchConfig, {
                acceptDurationDiffPercent: Number(event.currentTarget.value)
              });
            }}
          />
        </label>
        <label>
          Stretch leve até
          <input
            type="number"
            min={0}
            max={30}
            step={0.5}
            value={config.options.timingAlignment.lightStretchDiffPercent}
            onChange={(event) => {
              void patchTimingAlignment(config, patchConfig, {
                lightStretchDiffPercent: Number(event.currentTarget.value)
              });
            }}
          />
        </label>
        <label>
          Stretch máximo até
          <input
            type="number"
            min={0}
            max={40}
            step={0.5}
            value={config.options.timingAlignment.maxStretchDiffPercent}
            onChange={(event) => {
              void patchTimingAlignment(config, patchConfig, {
                maxStretchDiffPercent: Number(event.currentTarget.value)
              });
            }}
          />
        </label>
        <label>
          Tentativas por chunk
          <input
            type="number"
            min={1}
            max={10}
            step={1}
            value={config.options.timingAlignment.maxRegenerationAttempts}
            onChange={(event) => {
              void patchTimingAlignment(config, patchConfig, {
                maxRegenerationAttempts: Math.round(Number(event.currentTarget.value))
              });
            }}
          />
        </label>
      </div>

      <div className={styles.toggles}>
        <Toggle
          label="Palatalização"
          checked={config.options.palatalize}
          onCheckedChange={(palatalize) => {
            void patchConfig({ options: { ...config.options, palatalize } });
          }}
        />
        <Toggle
          label="Vírgula antes de ?"
          checked={config.options.commaBeforeQuestion}
          onCheckedChange={(commaBeforeQuestion) => {
            void patchConfig({ options: { ...config.options, commaBeforeQuestion } });
          }}
        />
        <Toggle
          label="Ponto final extra"
          checked={config.options.trailingPeriod}
          onCheckedChange={(trailingPeriod) => {
            void patchConfig({ options: { ...config.options, trailingPeriod } });
          }}
        />
        <Toggle
          label="Preservar fim de frase"
          checked={config.options.preserveSentenceBoundaries}
          onCheckedChange={(preserveSentenceBoundaries) => {
            void patchConfig({ options: { ...config.options, preserveSentenceBoundaries } });
          }}
        />
        <Toggle
          label="Adaptar texto ao timing"
          checked={config.options.timingAlignment.autoTextAdaptation}
          onCheckedChange={(autoTextAdaptation) => {
            void patchTimingAlignment(config, patchConfig, { autoTextAdaptation });
          }}
        />
        <Toggle
          label="Bloquear chunks críticos"
          checked={config.options.timingAlignment.blockExportOnCriticalChunks}
          onCheckedChange={(blockExportOnCriticalChunks) => {
            void patchTimingAlignment(config, patchConfig, { blockExportOnCriticalChunks });
          }}
        />
      </div>

      <button
        type="button"
        className={styles.saveButton}
        onClick={() => {
          void saveConfig(config);
        }}
      >
        <Save size={16} />
        Salvar configuração
      </button>

      <footer className={styles.authorCredit}>
        <span>Feito com</span>
        <Heart size={13} aria-hidden="true" />
        <span>por</span>
        <a
          href={authorGithubUrl}
          target="_blank"
          rel="noreferrer"
          onClick={(event) => {
            if (!isTauriRuntime()) {
              return;
            }
            event.preventDefault();
            void openUrl(authorGithubUrl);
          }}
        >
          iDiogoRenan
        </a>
      </footer>
    </section>
  );
}

function languageLabel(language: LanguageCode): string {
  const labels: Record<LanguageCode, string> = {
    auto: "Automático",
    en: "Inglês",
    pt: "Português",
    fr: "Francês",
    sv: "Sueco"
  };
  return labels[language];
}

function chunkLimitPolicyLabel(policy: ChunkLimitPolicy): string {
  const labels: Record<ChunkLimitPolicy, string> = {
    process_in_batches: "Processar em lotes",
    warn_and_continue: "Avisar e continuar",
    require_confirmation: "Pedir confirmação",
    resegment_first: "Tentar resegmentar",
    cancel_with_record: "Cancelar com registro"
  };
  return labels[policy];
}

function normalizeMaxSynthesisChunks(value: string): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return synthesisChunkLimits.min;
  }
  return Math.min(synthesisChunkLimits.max, Math.max(synthesisChunkLimits.min, Math.round(parsed)));
}

async function patchTimingAlignment(
  config: AppConfig,
  patchConfig: (patch: Partial<AppConfig>) => Promise<void>,
  patch: Partial<AppConfig["options"]["timingAlignment"]>
): Promise<void> {
  await patchConfig({
    options: {
      ...config.options,
      timingAlignment: {
        ...config.options.timingAlignment,
        ...patch
      }
    }
  });
}

interface ToggleProps {
  label: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}

function Toggle({ label, checked, onCheckedChange }: ToggleProps) {
  return (
    <label className={styles.toggle}>
      <Checkbox.Root
        checked={checked}
        onCheckedChange={(value) => {
          onCheckedChange(value === true);
        }}
      >
        <Checkbox.Indicator>
          <Check size={13} />
        </Checkbox.Indicator>
      </Checkbox.Root>
      <span>{label}</span>
    </label>
  );
}
