import * as Checkbox from "@radix-ui/react-checkbox";
import * as Select from "@radix-ui/react-select";
import * as Tooltip from "@radix-ui/react-tooltip";
import {
  Check,
  CheckCircle2,
  ChevronDown,
  Circle,
  Loader2,
  Mic2,
  Play,
  RotateCcw,
  Square,
  Undo2,
  Wand2
} from "lucide-react";
import type { ReactNode } from "react";
import { AudioPlayer } from "../audio-player/AudioPlayer";
import {
  isNativeTag,
  nativeTagDescriptions,
  replaceLine,
  splitLines,
  unknownNativeTags,
  voicePresets,
  type NativeTag,
  type VoicePresetId
} from "../../shared/omnivoice/nativeControls";
import { useWorkspaceStore, selectedLineMetadata } from "../../stores/workspaceStore";
import type {
  JobStage,
  NativeSynthesisSettings,
  ProjectLineMetadata,
  VoiceMode
} from "../../shared/tauri/types";
import { dubbingActionCopy } from "./dubbingAction";
import styles from "./DubbingPanel.module.css";

const pipelineStages: { stage: JobStage; label: string }[] = [
  { stage: "loading_models", label: "Modelos" },
  { stage: "transcribing", label: "Transcricao" },
  { stage: "translating", label: "Traducao" },
  { stage: "synthesizing", label: "Sintese" },
  { stage: "writing_output", label: "Saida" }
];

const stageOrder = new Map<JobStage, number>([
  ["queued", 0],
  ["loading_models", 0],
  ["preparing_file", 1],
  ["transcribing", 1],
  ["transcribed", 2],
  ["translating", 2],
  ["translated", 3],
  ["synthesizing", 3],
  ["writing_output", 4],
  ["file_complete", 5],
  ["finished", 5]
]);

export function DubbingPanel() {
  const config = useWorkspaceStore((state) => state.config);
  const files = useWorkspaceStore((state) => state.files);
  const selectedPath = useWorkspaceStore((state) => state.selectedPath);
  const selectedLineIndex = useWorkspaceStore((state) => state.selectedLineIndex);
  const sourceText = useWorkspaceStore((state) => state.sourceText);
  const targetText = useWorkspaceStore((state) => state.targetText);
  const projectMetadata = useWorkspaceStore((state) => state.projectMetadata);
  const transcriptionBaselines = useWorkspaceStore((state) => state.transcriptionBaselines);
  const setSourceText = useWorkspaceStore((state) => state.setSourceText);
  const setTargetText = useWorkspaceStore((state) => state.setTargetText);
  const setSelectedLineIndex = useWorkspaceStore((state) => state.setSelectedLineIndex);
  const updateSelectedLineMetadata = useWorkspaceStore((state) => state.updateSelectedLineMetadata);
  const updateSelectedLineSettings = useWorkspaceStore((state) => state.updateSelectedLineSettings);
  const removeNativeTag = useWorkspaceStore((state) => state.removeNativeTag);
  const previewSelectedLine = useWorkspaceStore((state) => state.previewSelectedLine);
  const revertTranscription = useWorkspaceStore((state) => state.revertTranscription);
  const startDubbing = useWorkspaceStore((state) => state.startDubbing);
  const cancelJob = useWorkspaceStore((state) => state.cancelJob);
  const appendLog = useWorkspaceStore((state) => state.appendLog);
  const isBusy = useWorkspaceStore((state) => state.isBusy);
  const isCancelling = useWorkspaceStore((state) => state.isCancelling);
  const progress = useWorkspaceStore((state) => state.progress);
  const currentStage = useWorkspaceStore((state) => state.currentStage);
  const currentStatus = useWorkspaceStore((state) => state.currentStatus);
  const currentFileName = useWorkspaceStore((state) => state.currentFileName);
  const currentFileIndex = useWorkspaceStore((state) => state.currentFileIndex);
  const totalFiles = useWorkspaceStore((state) => state.totalFiles);
  const lastOutputPath = useWorkspaceStore((state) => state.lastOutputPath);
  const logs = useWorkspaceStore((state) => state.logs);
  const selectedFile = files.find((file) => file.path === selectedPath) ?? null;
  const lineMetadata = selectedLineMetadata({
    ...useWorkspaceStore.getState(),
    config,
    files,
    projectMetadata,
    selectedPath,
    selectedLineIndex,
    targetText
  });
  const primaryAction = dubbingActionCopy(selectedFile?.status ?? null);
  const PrimaryActionIcon = primaryAction.intent === "redub" ? RotateCcw : Wand2;
  const transcriptionBaseline = selectedPath ? transcriptionBaselines[selectedPath] : undefined;
  const canRevertTranscription =
    transcriptionBaseline !== undefined &&
    (sourceText !== transcriptionBaseline.sourceText ||
      targetText !== transcriptionBaseline.targetText);

  return (
    <div className={styles.layout}>
      <div className={styles.mainColumn}>
        <section className={styles.players} aria-label="Players de áudio">
          <AudioPlayer title="Origem" path={selectedPath} />
          <AudioPlayer title="Resultado" path={lastOutputPath} />
        </section>

        <section className={styles.editorGrid} aria-label="Transcrição editável">
          <TaggedLineEditor
            title="Texto origem"
            language={config.options.sourceLanguage.toUpperCase()}
            text={sourceText}
            selectedLineIndex={selectedLineIndex}
            onSelectLine={setSelectedLineIndex}
            onTextChange={setSourceText}
            onInvalidTag={(tag) => {
              appendLog(`Tag OmniVoice nao suportada: ${tag}`, "warning");
            }}
          />
          <TaggedLineEditor
            title="Texto destino"
            language={config.options.targetLanguage.toUpperCase()}
            text={targetText}
            selectedLineIndex={selectedLineIndex}
            lineTags={lineMetadata.tags.filter(isNativeTag)}
            onSelectLine={setSelectedLineIndex}
            onTextChange={setTargetText}
            onRemoveTag={removeNativeTag}
            onInvalidTag={(tag) => {
              appendLog(`Tag OmniVoice nao suportada: ${tag}`, "warning");
            }}
          />
        </section>

        <section className={styles.controls}>
          <div className={styles.optionSummary}>
            <Mic2 size={16} />
            <span>
              {config.options.mode} · {config.options.sourceLanguage.toUpperCase()} →{" "}
              {config.options.targetLanguage.toUpperCase()} · pad {String(config.options.padMs)} ms
            </span>
          </div>
          <div className={styles.actions}>
            <button
              type="button"
              disabled={isBusy || !canRevertTranscription}
              onClick={() => {
                revertTranscription();
              }}
            >
              <Undo2 size={15} />
              Reverter transcrição
            </button>
            <button
              type="button"
              className={styles.primary}
              disabled={isBusy}
              onClick={() => {
                void startDubbing();
              }}
            >
              {isBusy ? (
                <Loader2 size={16} className={styles.spin} />
              ) : (
                <PrimaryActionIcon size={16} />
              )}
              {isBusy ? primaryAction.busyLabel : primaryAction.idleLabel}
            </button>
            <button
              type="button"
              disabled={!isBusy || isCancelling}
              onClick={() => {
                void cancelJob();
              }}
            >
              {isCancelling ? <Loader2 size={15} className={styles.spin} /> : <Square size={15} />}
              {isCancelling ? "Cancelando" : "Cancelar"}
            </button>
          </div>
        </section>
      </div>

      <LinePropertiesPanel
        selectedFile={selectedFile}
        selectedLineIndex={selectedLineIndex}
        targetText={targetText}
        metadata={lineMetadata}
        isBusy={isBusy}
        onMetadataChange={updateSelectedLineMetadata}
        onSettingsChange={updateSelectedLineSettings}
        onPreview={() => {
          void previewSelectedLine();
        }}
        onRegenerate={() => {
          void startDubbing();
        }}
      />

      <section className={styles.jobStatus}>
        <div className={styles.statusHeader}>
          <div>
            <span>{currentFileName ?? "Nenhum arquivo em execucao"}</span>
            <strong>{currentStatus}</strong>
          </div>
          <output>{Math.round(progress)}%</output>
        </div>
        <div className={styles.progress}>
          <div style={{ inlineSize: `${String(progress)}%` }} />
        </div>
        <div className={styles.stageRail}>
          {pipelineStages.map((item) => {
            const state = stageState(item.stage, currentStage);
            return (
              <span key={item.stage} data-state={state}>
                {state === "done" ? <CheckCircle2 size={15} /> : <Circle size={15} />}
                {item.label}
              </span>
            );
          })}
        </div>
        <p className={styles.fileCounter}>
          {currentFileIndex && totalFiles
            ? `Arquivo ${String(currentFileIndex)} de ${String(totalFiles)}`
            : "Fila sem arquivo ativo"}
        </p>
      </section>

      <section className={styles.logPanel} aria-label="Log de execução">
        {logs.map((entry) => (
          <p key={entry.id} data-level={entry.level}>
            {entry.message}
          </p>
        ))}
      </section>
    </div>
  );
}

interface TaggedLineEditorProps {
  title: string;
  language: string;
  text: string;
  selectedLineIndex: number;
  lineTags?: readonly NativeTag[];
  onSelectLine: (lineIndex: number) => void;
  onTextChange: (value: string) => void;
  onRemoveTag?: (tag: NativeTag) => void;
  onInvalidTag: (tag: string) => void;
}

function TaggedLineEditor({
  title,
  language,
  text,
  selectedLineIndex,
  lineTags,
  onSelectLine,
  onTextChange,
  onRemoveTag,
  onInvalidTag
}: TaggedLineEditorProps) {
  const lines = splitLines(text);

  return (
    <section className={styles.lineEditor}>
      <header>
        <div className={styles.editorHeading}>
          <span>{title}</span>
          <output>{language}</output>
        </div>
        {lineTags ? <SelectedNativeTags tags={lineTags} onRemoveTag={onRemoveTag} /> : null}
      </header>
      <div className={styles.lineList}>
        {lines.map((line, index) => (
          <label
            key={`${String(index)}-${String(lines.length)}`}
            className={styles.lineRow}
            data-selected={index === selectedLineIndex}
          >
            <span className={styles.lineNumber}>{index + 1}</span>
            <textarea
              value={line}
              rows={2}
              onFocus={() => {
                onSelectLine(index);
              }}
              onChange={(event) => {
                const nextLine = event.currentTarget.value;
                const [unknownTag] = unknownNativeTags(nextLine);
                if (unknownTag) {
                  onInvalidTag(unknownTag);
                  return;
                }
                onTextChange(replaceLine(text, index, nextLine));
              }}
            />
          </label>
        ))}
      </div>
    </section>
  );
}

interface SelectedNativeTagsProps {
  tags: readonly NativeTag[];
  onRemoveTag?: (tag: NativeTag) => void;
}

function SelectedNativeTags({ tags, onRemoveTag }: SelectedNativeTagsProps) {
  if (tags.length === 0) {
    return <span className={styles.emptyTagHeader}>Sem tags</span>;
  }

  return (
    <Tooltip.Provider delayDuration={120}>
      <div className={styles.selectedTags} aria-label="Tags nativas da linha selecionada">
        {tags.map((tag) => (
          <Tooltip.Root key={tag}>
            <Tooltip.Trigger asChild>
              <button
                type="button"
                title={nativeTagDescriptions[tag]}
                aria-label={`Remover ${tag}: ${nativeTagDescriptions[tag]}`}
                onClick={() => {
                  onRemoveTag?.(tag);
                }}
              >
                {tag}
              </button>
            </Tooltip.Trigger>
            <Tooltip.Portal>
              <Tooltip.Content className={styles.tooltipContent} side="top" sideOffset={6}>
                {nativeTagDescriptions[tag]}
                <Tooltip.Arrow className={styles.tooltipArrow} />
              </Tooltip.Content>
            </Tooltip.Portal>
          </Tooltip.Root>
        ))}
      </div>
    </Tooltip.Provider>
  );
}

interface LinePropertiesPanelProps {
  selectedFile: { metadata: { durationSeconds: number | null } | null } | null;
  selectedLineIndex: number;
  targetText: string;
  metadata: ProjectLineMetadata;
  isBusy: boolean;
  onMetadataChange: (patch: Partial<ProjectLineMetadata>) => void;
  onSettingsChange: (patch: Partial<NativeSynthesisSettings>) => void;
  onPreview: () => void;
  onRegenerate: () => void;
}

function LinePropertiesPanel({
  selectedFile,
  selectedLineIndex,
  targetText,
  metadata,
  isBusy,
  onMetadataChange,
  onSettingsChange,
  onPreview,
  onRegenerate
}: LinePropertiesPanelProps) {
  const settings = metadata.settings;
  const lineCount = splitLines(targetText).length;
  const currentDuration = selectedFile?.metadata?.durationSeconds ?? null;

  function applyPreset(presetId: VoicePresetId): void {
    const preset = voicePresets.find((item) => item.id === presetId);
    if (!preset) {
      return;
    }
    onMetadataChange({ characterId: preset.id });
    onSettingsChange(preset.settings);
  }

  return (
    <aside className={styles.properties} aria-label="Propriedades da linha">
      <header>
        <strong>Propriedades da linha</strong>
        <span>
          {selectedLineIndex + 1} / {lineCount}
        </span>
      </header>

      <ControlGroup label="Personagem">
        <RadixSelect
          value={metadata.characterId ?? "source_clone"}
          onValueChange={(value) => {
            applyPreset(value as VoicePresetId);
          }}
          items={voicePresets.map((preset) => ({ value: preset.id, label: preset.label }))}
        />
      </ControlGroup>

      <ControlGroup label="Modo de voz">
        <RadixSelect
          value={settings.voiceMode}
          onValueChange={(value) => {
            const voiceMode = value as VoiceMode;
            onSettingsChange({
              voiceMode,
              instruct:
                voiceMode === "design" && !settings.instruct
                  ? "female, young adult, moderate pitch"
                  : settings.instruct
            });
          }}
          items={[
            { value: "clone", label: "Clone" },
            { value: "design", label: "Design" },
            { value: "auto", label: "Auto" }
          ]}
        />
      </ControlGroup>

      <ControlGroup label="Instruct">
        <input
          value={settings.instruct ?? ""}
          disabled={settings.voiceMode !== "design"}
          onChange={(event) => {
            onSettingsChange({ instruct: event.currentTarget.value.trim() || null });
          }}
        />
      </ControlGroup>

      <ControlGroup label="Velocidade">
        <div className={styles.rangeRow}>
          <input
            type="range"
            min={0.5}
            max={2}
            step={0.05}
            value={settings.speed ?? 1}
            onChange={(event) => {
              onSettingsChange({ speed: Number(event.currentTarget.value) });
            }}
          />
          <output>{(settings.speed ?? 1).toFixed(2)}x</output>
        </div>
      </ControlGroup>

      <ControlGroup label="Duracao alvo">
        <input
          type="number"
          min={0.25}
          max={60}
          step={0.05}
          value={settings.durationSeconds ?? ""}
          onChange={(event) => {
            onSettingsChange({
              durationSeconds:
                event.currentTarget.value.length > 0 ? Number(event.currentTarget.value) : null
            });
          }}
        />
      </ControlGroup>

      <dl className={styles.durationFacts}>
        <div>
          <dt>Duracao atual</dt>
          <dd>{currentDuration ? `${currentDuration.toFixed(2)}s` : "-"}</dd>
        </div>
        <div>
          <dt>Tags</dt>
          <dd>{metadata.tags.length}</dd>
        </div>
      </dl>

      <ControlGroup label="Notas">
        <textarea
          value={metadata.notes ?? ""}
          rows={3}
          maxLength={200}
          onChange={(event) => {
            onMetadataChange({ notes: event.currentTarget.value || null });
          }}
        />
      </ControlGroup>

      <details className={styles.advanced}>
        <summary>Ajustes nativos</summary>
        <NumberField
          label="Steps"
          value={settings.numStep}
          min={8}
          max={128}
          step={1}
          onChange={(numStep) => {
            onSettingsChange({ numStep });
          }}
        />
        <NumberField
          label="Guidance"
          value={settings.guidanceScale}
          min={0}
          max={10}
          step={0.1}
          onChange={(guidanceScale) => {
            onSettingsChange({ guidanceScale });
          }}
        />
        <NumberField
          label="Position temp"
          value={settings.positionTemperature}
          min={0}
          max={10}
          step={0.1}
          onChange={(positionTemperature) => {
            onSettingsChange({ positionTemperature });
          }}
        />
        <NumberField
          label="Class temp"
          value={settings.classTemperature}
          min={0}
          max={10}
          step={0.1}
          onChange={(classTemperature) => {
            onSettingsChange({ classTemperature });
          }}
        />
        <NativeCheckbox
          label="Denoise"
          checked={settings.denoise}
          onCheckedChange={(denoise) => {
            onSettingsChange({ denoise });
          }}
        />
        <NativeCheckbox
          label="Preprocess prompt"
          checked={settings.preprocessPrompt}
          onCheckedChange={(preprocessPrompt) => {
            onSettingsChange({ preprocessPrompt });
          }}
        />
        <NativeCheckbox
          label="Postprocess output"
          checked={settings.postprocessOutput}
          onCheckedChange={(postprocessOutput) => {
            onSettingsChange({ postprocessOutput });
          }}
        />
      </details>

      <div className={styles.propertyActions}>
        <button type="button" disabled={isBusy} onClick={onPreview}>
          <Play size={15} />
          Previa desta linha
        </button>
        <button type="button" disabled={isBusy} onClick={onRegenerate}>
          <RotateCcw size={15} />
          Regenerar resultado
        </button>
      </div>
    </aside>
  );
}

interface ControlGroupProps {
  label: string;
  children: ReactNode;
}

function ControlGroup({ label, children }: ControlGroupProps) {
  return (
    <label className={styles.controlGroup}>
      <span>{label}</span>
      {children}
    </label>
  );
}

interface SelectItem {
  value: string;
  label: string;
}

interface RadixSelectProps {
  value: string;
  items: SelectItem[];
  onValueChange: (value: string) => void;
}

function RadixSelect({ value, items, onValueChange }: RadixSelectProps) {
  return (
    <Select.Root value={value} onValueChange={onValueChange}>
      <Select.Trigger className={styles.selectTrigger}>
        <Select.Value />
        <Select.Icon>
          <ChevronDown size={14} />
        </Select.Icon>
      </Select.Trigger>
      <Select.Portal>
        <Select.Content className={styles.selectContent} position="popper" sideOffset={4}>
          <Select.Viewport>
            {items.map((item) => (
              <Select.Item key={item.value} value={item.value} className={styles.selectItem}>
                <Select.ItemText>{item.label}</Select.ItemText>
              </Select.Item>
            ))}
          </Select.Viewport>
        </Select.Content>
      </Select.Portal>
    </Select.Root>
  );
}

interface NumberFieldProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
}

function NumberField({ label, value, min, max, step, onChange }: NumberFieldProps) {
  return (
    <label className={styles.numberField}>
      <span>{label}</span>
      <input
        type="number"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(event) => {
          onChange(Number(event.currentTarget.value));
        }}
      />
    </label>
  );
}

interface NativeCheckboxProps {
  label: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}

function NativeCheckbox({ label, checked, onCheckedChange }: NativeCheckboxProps) {
  return (
    <label className={styles.nativeCheckbox}>
      <Checkbox.Root
        checked={checked}
        onCheckedChange={(value) => {
          onCheckedChange(value === true);
        }}
      >
        <Checkbox.Indicator>
          <Check size={12} />
        </Checkbox.Indicator>
      </Checkbox.Root>
      <span>{label}</span>
    </label>
  );
}

function stageState(stage: JobStage, currentStage: JobStage | null): "pending" | "active" | "done" {
  if (!currentStage) {
    return "pending";
  }
  if (stage === currentStage) {
    return "active";
  }
  const current = stageOrder.get(currentStage) ?? 0;
  const target = stageOrder.get(stage) ?? 0;
  return target < current ? "done" : "pending";
}
