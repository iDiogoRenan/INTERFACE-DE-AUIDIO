import * as Checkbox from "@radix-ui/react-checkbox";
import { Check, Heart, Save } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { PathField } from "../../shared/ui/PathField";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { isTauriRuntime } from "../../shared/tauri/client";
import type { AppConfig, LanguageCode } from "../../shared/tauri/types";
import styles from "./SettingsPanel.module.css";

const authorGithubUrl = "https://github.com/iDiogoRenan";
const sourceLanguages: LanguageCode[] = ["auto", "en", "fr", "sv", "pt"];
const targetLanguages: LanguageCode[] = ["pt", "fr", "sv", "en"];
const synthesisChunkLimits = {
  min: 1,
  max: 20,
  seconds: 30
} as const;

export function SettingsPanel() {
  const config = useWorkspaceStore((state) => state.config);
  const saveConfig = useWorkspaceStore((state) => state.saveConfig);
  const scan = useWorkspaceStore((state) => state.scan);

  async function patchConfig(patch: Partial<AppConfig>): Promise<void> {
    await saveConfig({ ...config, ...patch });
  }

  async function saveAndScan(nextConfig: AppConfig): Promise<void> {
    await saveConfig(nextConfig);
    await scan();
  }

  return (
    <section className={styles.panel}>
      <div className={styles.pathGrid}>
        <PathField
          label="Origem"
          value={config.inputDir}
          mode="directory"
          placeholder="Pasta com áudios originais"
          onChange={(inputDir) => {
            void saveAndScan({ ...config, inputDir });
          }}
        />
        <PathField
          label="Destino"
          value={config.outputDir}
          mode="directory"
          placeholder="Pasta para dublagens"
          onChange={(outputDir) => {
            void patchConfig({ outputDir });
          }}
        />
        <PathField
          label="Áudio guia"
          value={config.guideAudio}
          mode="file"
          placeholder="Arquivo de referência opcional"
          onChange={(guideAudio) => {
            void patchConfig({ guideAudio });
          }}
        />
        <PathField
          label="Aprovados"
          value={config.approvedDir}
          mode="directory"
          placeholder="Pasta de aprovação final"
          onChange={(approvedDir) => {
            void patchConfig({ approvedDir });
          }}
        />
        <PathField
          label="Modelos"
          value={config.modelDir}
          mode="directory"
          placeholder="Pasta local de modelos"
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
          Chunks máximos ({synthesisChunkLimits.seconds}s)
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

function normalizeMaxSynthesisChunks(value: string): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return synthesisChunkLimits.min;
  }
  return Math.min(synthesisChunkLimits.max, Math.max(synthesisChunkLimits.min, Math.round(parsed)));
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
