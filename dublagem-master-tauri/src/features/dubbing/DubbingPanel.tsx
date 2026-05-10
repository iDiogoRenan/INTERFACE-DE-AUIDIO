import * as Checkbox from "@radix-ui/react-checkbox";
import * as Select from "@radix-ui/react-select";
import * as Tooltip from "@radix-ui/react-tooltip";
import {
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Circle,
  Loader2,
  Play,
  RotateCcw,
  Square,
  Undo2,
  Wand2
} from "lucide-react";
import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { AudioPlayer } from "../audio-player/AudioPlayer";
import {
  isNativeTag,
  nativeTagDescriptions,
  nativeSynthesisNumberControls,
  normalizeNativeSynthesisSettings,
  replaceLine,
  splitLines,
  unknownNativeTags,
  type NativeTag
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

const LINE_SECTION_OPEN_STORAGE_KEY = "nsg-gaming-dub.line-sidebar-sections.v1";

type LineSectionKey = "properties" | "basicSettings" | "nativeAdjustments" | "audioPolish";
type LineSectionOpenState = Record<LineSectionKey, boolean>;
type SetLineSectionOpen = (section: LineSectionKey, isOpen: boolean) => void;

const DEFAULT_LINE_SECTION_OPEN_STATE: LineSectionOpenState = {
  properties: true,
  basicSettings: true,
  nativeAdjustments: false,
  audioPolish: true
};

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
  const saveSelectedLineSettingsAsDefault = useWorkspaceStore(
    (state) => state.saveSelectedLineSettingsAsDefault
  );
  const resetSelectedLineSettingsToDefault = useWorkspaceStore(
    (state) => state.resetSelectedLineSettingsToDefault
  );
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
  const lastOutputRevision = useWorkspaceStore((state) => state.lastOutputRevision);
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
          <AudioPlayer title="Resultado" path={lastOutputPath} revision={lastOutputRevision} />
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
              appendLog(unsupportedNativeTagMessage(tag), "warning");
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
              appendLog(unsupportedNativeTagMessage(tag), "warning");
            }}
          />
        </section>

        <section className={styles.controls}>
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

      <LinePropertiesSidebar
        selectedFile={selectedFile}
        selectedLineIndex={selectedLineIndex}
        targetText={targetText}
        metadata={lineMetadata}
        isBusy={isBusy}
        onMetadataChange={updateSelectedLineMetadata}
        onSettingsChange={updateSelectedLineSettings}
        onSaveSettingsAsDefault={() => {
          void saveSelectedLineSettingsAsDefault();
        }}
        onResetSettingsToDefault={() => {
          void resetSelectedLineSettingsToDefault();
        }}
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
            <AutoSizingLineTextarea
              value={line}
              onFocus={() => {
                onSelectLine(index);
              }}
              onChange={(nextLine) => {
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

interface AutoSizingLineTextareaProps {
  value: string;
  onFocus: () => void;
  onChange: (value: string) => void;
}

function AutoSizingLineTextarea({ value, onFocus, onChange }: AutoSizingLineTextareaProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useLayoutEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea || typeof ResizeObserver === "undefined") {
      return;
    }

    const observer = new ResizeObserver(() => {
      resizeLineTextarea(textarea);
    });
    observer.observe(textarea);

    return () => {
      observer.disconnect();
    };
  }, []);

  useLayoutEffect(() => {
    resizeLineTextarea(textareaRef.current);
  }, [value]);

  return (
    <textarea
      ref={textareaRef}
      value={value}
      rows={2}
      onFocus={onFocus}
      onChange={(event) => {
        onChange(event.currentTarget.value);
      }}
    />
  );
}

function resizeLineTextarea(textarea: HTMLTextAreaElement | null): void {
  if (!textarea) {
    return;
  }
  textarea.style.height = "auto";
  textarea.style.height = `${String(textarea.scrollHeight)}px`;
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

interface LinePropertiesSidebarProps {
  selectedFile: { metadata: { durationSeconds: number | null } | null } | null;
  selectedLineIndex: number;
  targetText: string;
  metadata: ProjectLineMetadata;
  isBusy: boolean;
  onMetadataChange: (patch: Partial<ProjectLineMetadata>) => void;
  onSettingsChange: (patch: Partial<NativeSynthesisSettings>) => void;
  onSaveSettingsAsDefault: () => void;
  onResetSettingsToDefault: () => void;
  onPreview: () => void;
  onRegenerate: () => void;
}

function LinePropertiesSidebar({
  selectedFile,
  selectedLineIndex,
  targetText,
  metadata,
  isBusy,
  onMetadataChange,
  onSettingsChange,
  onSaveSettingsAsDefault,
  onResetSettingsToDefault,
  onPreview,
  onRegenerate
}: LinePropertiesSidebarProps) {
  const [sectionOpenState, setSectionOpen] = usePersistentLineSectionOpenState();
  const targetLines = splitLines(targetText);
  const lineCount = targetLines.length;
  const selectedLineText = targetLines[selectedLineIndex]?.trim() ?? "";
  const currentDuration = selectedFile?.metadata?.durationSeconds ?? null;
  const controlsDisabled = isBusy || !selectedFile;

  return (
    <aside className={styles.lineSidebar} aria-label="Propriedades da linha">
      <LinePropertiesPanel
        selectedLineIndex={selectedLineIndex}
        lineCount={lineCount}
        currentDuration={currentDuration}
        metadata={metadata}
        controlsDisabled={controlsDisabled}
        sectionOpenState={sectionOpenState}
        onSectionOpenChange={setSectionOpen}
        onMetadataChange={onMetadataChange}
        onSettingsChange={onSettingsChange}
      />
      <LineActionDock
        controlsDisabled={controlsDisabled}
        canPreviewLine={selectedLineText.length > 0}
        onSaveSettingsAsDefault={onSaveSettingsAsDefault}
        onResetSettingsToDefault={onResetSettingsToDefault}
        onPreview={onPreview}
        onRegenerate={onRegenerate}
      />
    </aside>
  );
}

interface LinePropertiesPanelProps {
  selectedLineIndex: number;
  lineCount: number;
  currentDuration: number | null;
  metadata: ProjectLineMetadata;
  controlsDisabled: boolean;
  sectionOpenState: LineSectionOpenState;
  onSectionOpenChange: SetLineSectionOpen;
  onMetadataChange: (patch: Partial<ProjectLineMetadata>) => void;
  onSettingsChange: (patch: Partial<NativeSynthesisSettings>) => void;
}

function LinePropertiesPanel({
  selectedLineIndex,
  lineCount,
  currentDuration,
  metadata,
  controlsDisabled,
  sectionOpenState,
  onSectionOpenChange,
  onMetadataChange,
  onSettingsChange
}: LinePropertiesPanelProps) {
  const settings = metadata.settings;
  const acceptedSettings = normalizeNativeSynthesisSettings(settings);
  const isPropertiesOpen = sectionOpenState.properties;

  return (
    <section
      className={`${styles.properties} ${isPropertiesOpen ? "" : styles.propertiesCollapsed}`}
      aria-label="Conteúdo de propriedades da linha"
    >
      <button
        type="button"
        className={styles.propertiesHeader}
        aria-expanded={isPropertiesOpen}
        onClick={() => {
          onSectionOpenChange("properties", !isPropertiesOpen);
        }}
      >
        {isPropertiesOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        <span className={styles.propertiesTitle}>
          <strong>Propriedades da linha</strong>
          <output>
            {selectedLineIndex + 1} / {lineCount}
          </output>
        </span>
      </button>

      {isPropertiesOpen && (
        <div className={styles.propertiesBody}>
          <CollapsibleSection
            title="Propriedades basicas"
            isOpen={sectionOpenState.basicSettings}
            onOpenChange={(isOpen) => {
              onSectionOpenChange("basicSettings", isOpen);
            }}
          >
            <ControlGroup label="Modo de voz">
              <RadixSelect
                value={acceptedSettings.voiceMode}
                onValueChange={(value) => {
                  const voiceMode = value as VoiceMode;
                  onSettingsChange({
                    voiceMode,
                    instruct:
                      voiceMode === "design"
                        ? (acceptedSettings.instruct ?? "female, young adult, moderate pitch")
                        : null
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
                value={acceptedSettings.instruct ?? ""}
                disabled={controlsDisabled || acceptedSettings.voiceMode !== "design"}
                onChange={(event) => {
                  onSettingsChange({ instruct: event.currentTarget.value.trim() || null });
                }}
              />
            </ControlGroup>

            <ControlGroup label="Velocidade">
              <div className={styles.rangeRow}>
                <input
                  type="range"
                  min={nativeSynthesisNumberControls.speed.min}
                  max={nativeSynthesisNumberControls.speed.max}
                  step={nativeSynthesisNumberControls.speed.step}
                  value={acceptedSettings.speed ?? 1}
                  disabled={controlsDisabled}
                  onChange={(event) => {
                    onSettingsChange({ speed: Number(event.currentTarget.value) });
                  }}
                />
                <output>{(acceptedSettings.speed ?? 1).toFixed(2)}x</output>
              </div>
            </ControlGroup>

            <ControlGroup label="Duracao alvo">
              <input
                type="number"
                min={nativeSynthesisNumberControls.durationSeconds.min}
                max={nativeSynthesisNumberControls.durationSeconds.max}
                step={nativeSynthesisNumberControls.durationSeconds.step}
                value={acceptedSettings.durationSeconds ?? ""}
                disabled={controlsDisabled}
                onChange={(event) => {
                  const nextDuration = optionalNumberFromInput(event.currentTarget.value);
                  onSettingsChange({
                    durationSeconds: nextDuration
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
                disabled={controlsDisabled}
                onChange={(event) => {
                  onMetadataChange({ notes: event.currentTarget.value || null });
                }}
              />
            </ControlGroup>
          </CollapsibleSection>

          <CollapsibleSection
            title="Ajustes nativos"
            isOpen={sectionOpenState.nativeAdjustments}
            onOpenChange={(isOpen) => {
              onSectionOpenChange("nativeAdjustments", isOpen);
            }}
          >
            <NumberField
              label="Steps"
              value={acceptedSettings.numStep}
              min={nativeSynthesisNumberControls.numStep.min}
              max={nativeSynthesisNumberControls.numStep.max}
              step={nativeSynthesisNumberControls.numStep.step}
              disabled={controlsDisabled}
              onChange={(numStep) => {
                onSettingsChange({ numStep });
              }}
            />
            <NumberField
              label="Guidance"
              value={acceptedSettings.guidanceScale}
              min={nativeSynthesisNumberControls.guidanceScale.min}
              max={nativeSynthesisNumberControls.guidanceScale.max}
              step={nativeSynthesisNumberControls.guidanceScale.step}
              disabled={controlsDisabled}
              onChange={(guidanceScale) => {
                onSettingsChange({ guidanceScale });
              }}
            />
            <NumberField
              label="Position temp"
              value={acceptedSettings.positionTemperature}
              min={nativeSynthesisNumberControls.positionTemperature.min}
              max={nativeSynthesisNumberControls.positionTemperature.max}
              step={nativeSynthesisNumberControls.positionTemperature.step}
              disabled={controlsDisabled}
              onChange={(positionTemperature) => {
                onSettingsChange({ positionTemperature });
              }}
            />
            <NumberField
              label="Class temp"
              value={acceptedSettings.classTemperature}
              min={nativeSynthesisNumberControls.classTemperature.min}
              max={nativeSynthesisNumberControls.classTemperature.max}
              step={nativeSynthesisNumberControls.classTemperature.step}
              disabled={controlsDisabled}
              onChange={(classTemperature) => {
                onSettingsChange({ classTemperature });
              }}
            />
            <NativeCheckbox
              label="Denoise"
              checked={acceptedSettings.denoise}
              disabled={controlsDisabled}
              onCheckedChange={(denoise) => {
                onSettingsChange({ denoise });
              }}
            />
            <NativeCheckbox
              label="Preprocess prompt"
              checked={acceptedSettings.preprocessPrompt}
              disabled={controlsDisabled}
              onCheckedChange={(preprocessPrompt) => {
                onSettingsChange({ preprocessPrompt });
              }}
            />
            <NativeCheckbox
              label="Postprocess output"
              checked={acceptedSettings.postprocessOutput}
              disabled={controlsDisabled}
              onCheckedChange={(postprocessOutput) => {
                onSettingsChange({ postprocessOutput });
              }}
            />
          </CollapsibleSection>

          <CollapsibleSection
            title="Polimento de audio"
            isOpen={sectionOpenState.audioPolish}
            onOpenChange={(isOpen) => {
              onSectionOpenChange("audioPolish", isOpen);
            }}
          >
            <NativeCheckbox
              label="Casar loudness com origem"
              checked={acceptedSettings.matchSourceLoudness}
              disabled={controlsDisabled}
              onCheckedChange={(matchSourceLoudness) => {
                onSettingsChange({ matchSourceLoudness });
              }}
            />
            <RangeField
              label="Forca loudness"
              value={acceptedSettings.loudnessMatchStrength}
              min={nativeSynthesisNumberControls.loudnessMatchStrength.min}
              max={nativeSynthesisNumberControls.loudnessMatchStrength.max}
              step={nativeSynthesisNumberControls.loudnessMatchStrength.step}
              disabled={controlsDisabled || !acceptedSettings.matchSourceLoudness}
              formatValue={formatPercent}
              onChange={(loudnessMatchStrength) => {
                onSettingsChange({ loudnessMatchStrength });
              }}
            />
            <RangeField
              label="Ganho final"
              value={acceptedSettings.outputGainDb}
              min={nativeSynthesisNumberControls.outputGainDb.min}
              max={nativeSynthesisNumberControls.outputGainDb.max}
              step={nativeSynthesisNumberControls.outputGainDb.step}
              disabled={controlsDisabled}
              formatValue={formatDecibels}
              onChange={(outputGainDb) => {
                onSettingsChange({ outputGainDb });
              }}
            />
            <RangeField
              label="Reducao de sibilancia"
              value={acceptedSettings.sibilanceReduction}
              min={nativeSynthesisNumberControls.sibilanceReduction.min}
              max={nativeSynthesisNumberControls.sibilanceReduction.max}
              step={nativeSynthesisNumberControls.sibilanceReduction.step}
              disabled={controlsDisabled}
              formatValue={formatReduction}
              onChange={(sibilanceReduction) => {
                onSettingsChange({ sibilanceReduction });
              }}
            />
            <RangeField
              label="Reducao de metalizado"
              value={acceptedSettings.artifactReduction}
              min={nativeSynthesisNumberControls.artifactReduction.min}
              max={nativeSynthesisNumberControls.artifactReduction.max}
              step={nativeSynthesisNumberControls.artifactReduction.step}
              disabled={controlsDisabled}
              formatValue={formatReduction}
              onChange={(artifactReduction) => {
                onSettingsChange({ artifactReduction });
              }}
            />
          </CollapsibleSection>
        </div>
      )}
    </section>
  );
}

interface LineActionDockProps {
  controlsDisabled: boolean;
  canPreviewLine: boolean;
  onSaveSettingsAsDefault: () => void;
  onResetSettingsToDefault: () => void;
  onPreview: () => void;
  onRegenerate: () => void;
}

function LineActionDock({
  controlsDisabled,
  canPreviewLine,
  onSaveSettingsAsDefault,
  onResetSettingsToDefault,
  onPreview,
  onRegenerate
}: LineActionDockProps) {
  return (
    <section className={styles.propertyActions} aria-label="Controles da linha">
      <button type="button" disabled={controlsDisabled} onClick={onSaveSettingsAsDefault}>
        <CheckCircle2 size={15} />
        Salvar padrao global
      </button>
      <button type="button" disabled={controlsDisabled} onClick={onResetSettingsToDefault}>
        <Undo2 size={15} />
        Restaurar defaults
      </button>
      <button type="button" disabled={controlsDisabled || !canPreviewLine} onClick={onPreview}>
        <Play size={15} />
        Previa desta linha
      </button>
      <button type="button" disabled={controlsDisabled} onClick={onRegenerate}>
        <RotateCcw size={15} />
        Regenerar resultado
      </button>
    </section>
  );
}

interface CollapsibleSectionProps {
  title: string;
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  children: ReactNode;
}

function CollapsibleSection({ title, isOpen, onOpenChange, children }: CollapsibleSectionProps) {
  const contentId = useId();

  return (
    <section className={styles.collapsibleSection}>
      <button
        type="button"
        className={styles.collapsibleHeader}
        aria-expanded={isOpen}
        aria-controls={contentId}
        onClick={() => {
          onOpenChange(!isOpen);
        }}
      >
        {isOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        <span>{title}</span>
      </button>
      {isOpen && (
        <div id={contentId} className={styles.collapsibleContent}>
          {children}
        </div>
      )}
    </section>
  );
}

function usePersistentLineSectionOpenState(): readonly [LineSectionOpenState, SetLineSectionOpen] {
  const [sectionOpenState, setSectionOpenState] =
    useState<LineSectionOpenState>(readLineSectionOpenState);

  useEffect(() => {
    writeLineSectionOpenState(sectionOpenState);
  }, [sectionOpenState]);

  const setSectionOpen: SetLineSectionOpen = (section, isOpen) => {
    setSectionOpenState((current) => {
      if (current[section] === isOpen) {
        return current;
      }
      return { ...current, [section]: isOpen };
    });
  };

  return [sectionOpenState, setSectionOpen];
}

function readLineSectionOpenState(): LineSectionOpenState {
  if (typeof window === "undefined") {
    return DEFAULT_LINE_SECTION_OPEN_STATE;
  }

  const storedState = localStorage.getItem(LINE_SECTION_OPEN_STORAGE_KEY);
  if (!storedState) {
    return DEFAULT_LINE_SECTION_OPEN_STATE;
  }

  try {
    const parsedState: unknown = JSON.parse(storedState);
    if (!isLineSectionOpenStateRecord(parsedState)) {
      return DEFAULT_LINE_SECTION_OPEN_STATE;
    }

    return {
      properties: readStoredBoolean(parsedState.properties, "properties"),
      basicSettings: readStoredBoolean(parsedState.basicSettings, "basicSettings"),
      nativeAdjustments: readStoredBoolean(parsedState.nativeAdjustments, "nativeAdjustments"),
      audioPolish: readStoredBoolean(parsedState.audioPolish, "audioPolish")
    };
  } catch {
    localStorage.removeItem(LINE_SECTION_OPEN_STORAGE_KEY);
    return DEFAULT_LINE_SECTION_OPEN_STATE;
  }
}

function writeLineSectionOpenState(sectionOpenState: LineSectionOpenState): void {
  if (typeof window === "undefined") {
    return;
  }

  try {
    localStorage.setItem(LINE_SECTION_OPEN_STORAGE_KEY, JSON.stringify(sectionOpenState));
  } catch {
    return;
  }
}

function isLineSectionOpenStateRecord(
  value: unknown
): value is Partial<Record<LineSectionKey, unknown>> {
  return typeof value === "object" && value !== null;
}

function readStoredBoolean(value: unknown, fallbackKey: LineSectionKey): boolean {
  return typeof value === "boolean" ? value : DEFAULT_LINE_SECTION_OPEN_STATE[fallbackKey];
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
  disabled: boolean;
  onChange: (value: number) => void;
}

function NumberField({ label, value, min, max, step, disabled, onChange }: NumberFieldProps) {
  return (
    <label className={styles.numberField}>
      <span>{label}</span>
      <input
        type="number"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(event) => {
          const nextValue = Number(event.currentTarget.value);
          if (Number.isFinite(nextValue)) {
            onChange(nextValue);
          }
        }}
      />
    </label>
  );
}

interface RangeFieldProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  disabled: boolean;
  formatValue: (value: number) => string;
  onChange: (value: number) => void;
}

function RangeField({
  label,
  value,
  min,
  max,
  step,
  disabled,
  formatValue,
  onChange
}: RangeFieldProps) {
  return (
    <label className={styles.controlGroup}>
      <span>{label}</span>
      <div className={styles.rangeRow}>
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          onChange={(event) => {
            const nextValue = Number(event.currentTarget.value);
            if (Number.isFinite(nextValue)) {
              onChange(nextValue);
            }
          }}
        />
        <output>{formatValue(value)}</output>
      </div>
    </label>
  );
}

function optionalNumberFromInput(value: string): number | null {
  if (value.length === 0) {
    return null;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function formatPercent(value: number): string {
  return `${String(Math.round(value * 100))}%`;
}

function formatDecibels(value: number): string {
  return `${value > 0 ? "+" : ""}${value.toFixed(1)} dB`;
}

function formatReduction(value: number): string {
  const percent = Math.round(value * 100);
  return percent === 0 ? "off" : `-${String(percent)}%`;
}

interface NativeCheckboxProps {
  label: string;
  checked: boolean;
  disabled: boolean;
  onCheckedChange: (checked: boolean) => void;
}

function NativeCheckbox({ label, checked, disabled, onCheckedChange }: NativeCheckboxProps) {
  return (
    <label className={styles.nativeCheckbox}>
      <Checkbox.Root
        checked={checked}
        disabled={disabled}
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

function unsupportedNativeTagMessage(tag: string): string {
  if (tag === "[pause]") {
    return "OmniVoice nao suporta [pause] como tag nativa. Use pontuacao ou duracao alvo para controlar pausas.";
  }

  return `Tag OmniVoice nao suportada: ${tag}`;
}
