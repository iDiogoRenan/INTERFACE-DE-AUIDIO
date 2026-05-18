import * as Checkbox from "@radix-ui/react-checkbox";
import * as Select from "@radix-ui/react-select";
import * as Tooltip from "@radix-ui/react-tooltip";
import { save } from "@tauri-apps/plugin-dialog";
import {
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Circle,
  Download,
  Loader2,
  Pin,
  Play,
  RotateCcw,
  Square,
  Undo2,
  Wand2
} from "lucide-react";
import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { AudioPlayer } from "../audio-player/AudioPlayer";
import {
  effectiveNativeTags,
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
import { activeNativeSynthesisSettings } from "../../shared/speechModels";
import type {
  AudioFileEntry,
  JobStage,
  NativeSynthesisSettings,
  ProjectLineMetadata,
  VoiceMode
} from "../../shared/tauri/types";
import { dubbingActionCopy } from "./dubbingAction";
import { TimingAlignmentReportPanel } from "./TimingAlignmentReportPanel";
import styles from "./DubbingPanel.module.css";

const pipelineStages: { stage: JobStage; label: string }[] = [
  { stage: "loading_models", label: "Modelos" },
  { stage: "transcribing", label: "Transcrição" },
  { stage: "translating", label: "Tradução" },
  { stage: "synthesizing", label: "Síntese" },
  { stage: "writing_output", label: "Saída" }
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
  const updateGlobalSynthesisSettings = useWorkspaceStore(
    (state) => state.updateGlobalSynthesisSettings
  );
  const saveGlobalSynthesisSettings = useWorkspaceStore(
    (state) => state.saveGlobalSynthesisSettings
  );
  const resetGlobalSynthesisSettings = useWorkspaceStore(
    (state) => state.resetGlobalSynthesisSettings
  );
  const removeNativeTag = useWorkspaceStore((state) => state.removeNativeTag);
  const pinnedNativeTags = useWorkspaceStore((state) => state.pinnedNativeTags);
  const togglePinnedNativeTag = useWorkspaceStore((state) => state.togglePinnedNativeTag);
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
  const lastAlignmentReport = useWorkspaceStore((state) => state.lastAlignmentReport);
  const logs = useWorkspaceStore((state) => state.logs);
  const [isLogPanelCollapsed, setLogPanelCollapsed] = useState(false);
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
  const globalSynthesisSettings = activeNativeSynthesisSettings(config);
  const primaryAction = dubbingActionCopy(selectedFile?.status ?? null);
  const PrimaryActionIcon = primaryAction.intent === "redub" ? RotateCcw : Wand2;
  const transcriptionBaseline = selectedPath ? transcriptionBaselines[selectedPath] : undefined;
  const canRevertTranscription =
    transcriptionBaseline !== undefined &&
    (sourceText !== transcriptionBaseline.sourceText ||
      targetText !== transcriptionBaseline.targetText);

  const handleStartDubbing = () => {
    void startDubbing();
  };
  const handleStartDubbingAndSave = () => {
    void startSelectedDubbingWithSaveDialog({
      selectedFile,
      outputDir: config.outputDir,
      startDubbing,
      appendLog
    });
  };

  return (
    <div className={styles.layout} data-log-collapsed={isLogPanelCollapsed}>
      <div className={styles.mainColumn}>
        <section className={styles.players} aria-label="Reprodutores de áudio">
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
            pinnedTags={pinnedNativeTags}
            onSelectLine={setSelectedLineIndex}
            onTextChange={setTargetText}
            onRemoveTag={removeNativeTag}
            onTogglePinnedTag={togglePinnedNativeTag}
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
              onClick={handleStartDubbing}
            >
              {isBusy ? (
                <Loader2 size={16} className={styles.spin} />
              ) : (
                <PrimaryActionIcon size={16} />
              )}
              {isBusy ? primaryAction.busyLabel : primaryAction.idleLabel}
            </button>
            <button type="button" disabled={isBusy} onClick={handleStartDubbingAndSave}>
              <Download size={16} />
              {primaryAction.intent === "redub" ? "Redublar e salvar" : "Dublar e salvar"}
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

        <TimingAlignmentReportPanel
          report={lastAlignmentReport}
          isBusy={isBusy}
          onRegenerate={handleStartDubbing}
          onEditChunk={(chunkIndex) => {
            setSelectedLineIndex(Math.max(0, chunkIndex - 1));
          }}
          onAcceptChunk={(chunk) => {
            appendLog(
              `Chunk ${String(chunk.chunkIndex)} aceito manualmente no relatório temporal.`,
              "success"
            );
          }}
        />
      </div>

      <LinePropertiesSidebar
        selectedFile={selectedFile}
        selectedLineIndex={selectedLineIndex}
        targetText={targetText}
        metadata={lineMetadata}
        settings={globalSynthesisSettings}
        pinnedTags={pinnedNativeTags}
        isBusy={isBusy}
        onMetadataChange={updateSelectedLineMetadata}
        onSettingsChange={updateGlobalSynthesisSettings}
        onSaveSettingsAsDefault={() => {
          void saveGlobalSynthesisSettings();
        }}
        onResetSettingsToDefault={() => {
          void resetGlobalSynthesisSettings();
        }}
        onPreview={() => {
          void previewSelectedLine();
        }}
        onRegenerate={handleStartDubbing}
      />

      <section className={styles.jobStatus}>
        <div className={styles.statusHeader}>
          <div>
            <span>{currentFileName ?? "Nenhum arquivo em execução"}</span>
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

      <section
        className={styles.logPanel}
        data-collapsed={isLogPanelCollapsed}
        aria-label="Registro de execução"
      >
        <header className={styles.logHeader}>
          <div>
            <span>Registro de execução</span>
            <strong>{logs.length === 1 ? "1 evento" : `${String(logs.length)} eventos`}</strong>
          </div>
          <button
            type="button"
            aria-expanded={!isLogPanelCollapsed}
            onClick={() => {
              setLogPanelCollapsed((current) => !current);
            }}
          >
            {isLogPanelCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
            {isLogPanelCollapsed ? "Expandir logs" : "Recolher logs"}
          </button>
        </header>
        {!isLogPanelCollapsed ? (
          <div className={styles.logEntries}>
            {logs.map((entry) => (
              <p key={entry.id} data-level={entry.level}>
                <time className={styles.logTimestamp} dateTime={entry.timestamp}>
                  {formatLogTimestamp(entry.timestamp)}
                </time>
                <span>{entry.message}</span>
              </p>
            ))}
          </div>
        ) : null}
      </section>
    </div>
  );
}

const logTimestampFormatter = new Intl.DateTimeFormat("pt-BR", {
  day: "2-digit",
  month: "2-digit",
  year: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false
});

function formatLogTimestamp(timestamp: string): string {
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) {
    return timestamp;
  }
  return logTimestampFormatter.format(date).replace(",", "");
}

interface TaggedLineEditorProps {
  title: string;
  language: string;
  text: string;
  selectedLineIndex: number;
  lineTags?: readonly NativeTag[];
  pinnedTags?: readonly NativeTag[];
  onSelectLine: (lineIndex: number) => void;
  onTextChange: (value: string) => void;
  onRemoveTag?: (tag: NativeTag) => void;
  onTogglePinnedTag?: (tag: NativeTag) => void;
  onInvalidTag: (tag: string) => void;
}

function TaggedLineEditor({
  title,
  language,
  text,
  selectedLineIndex,
  lineTags,
  pinnedTags = [],
  onSelectLine,
  onTextChange,
  onRemoveTag,
  onTogglePinnedTag,
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
        {lineTags ? (
          <SelectedNativeTags
            lineTags={lineTags}
            pinnedTags={pinnedTags}
            onRemoveTag={onRemoveTag}
            onTogglePinnedTag={onTogglePinnedTag}
          />
        ) : null}
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
  const resizeAnimationFrameRef = useRef<number | null>(null);
  const observedInlineSizeRef = useRef<number | null>(null);

  useLayoutEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea || typeof ResizeObserver === "undefined") {
      return;
    }

    observedInlineSizeRef.current = textarea.getBoundingClientRect().width;

    const observer = new ResizeObserver((entries) => {
      const [entry] = entries;
      const nextInlineSize = resizeObserverInlineSize(entry);
      const previousInlineSize = observedInlineSizeRef.current;
      if (previousInlineSize !== null && Math.abs(nextInlineSize - previousInlineSize) < 0.5) {
        return;
      }

      observedInlineSizeRef.current = nextInlineSize;
      queueTextareaResize(textarea, resizeAnimationFrameRef);
    });
    observer.observe(textarea);

    return () => {
      observer.disconnect();
      cancelQueuedTextareaResize(resizeAnimationFrameRef);
      observedInlineSizeRef.current = null;
    };
  }, []);

  useLayoutEffect(() => {
    cancelQueuedTextareaResize(resizeAnimationFrameRef);
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

function resizeObserverInlineSize(entry: ResizeObserverEntry): number {
  return entry.contentBoxSize[0]?.inlineSize ?? entry.contentRect.width;
}

function queueTextareaResize(
  textarea: HTMLTextAreaElement,
  frameRef: { current: number | null }
): void {
  cancelQueuedTextareaResize(frameRef);
  frameRef.current = requestAnimationFrame(() => {
    frameRef.current = null;
    resizeLineTextarea(textarea);
  });
}

function cancelQueuedTextareaResize(frameRef: { current: number | null }): void {
  if (frameRef.current === null) {
    return;
  }
  cancelAnimationFrame(frameRef.current);
  frameRef.current = null;
}

function resizeLineTextarea(textarea: HTMLTextAreaElement | null): void {
  if (!textarea) {
    return;
  }
  textarea.style.height = "auto";
  textarea.style.height = `${String(textarea.scrollHeight)}px`;
}

interface SelectedNativeTagsProps {
  lineTags: readonly NativeTag[];
  pinnedTags: readonly NativeTag[];
  onRemoveTag?: (tag: NativeTag) => void;
  onTogglePinnedTag?: (tag: NativeTag) => void;
}

function SelectedNativeTags({
  lineTags,
  pinnedTags,
  onRemoveTag,
  onTogglePinnedTag
}: SelectedNativeTagsProps) {
  const tags = effectiveNativeTags(lineTags, pinnedTags);

  if (tags.length === 0) {
    return <span className={styles.emptyTagHeader}>Sem marcadores</span>;
  }

  const lineTagSet = new Set<NativeTag>(lineTags);
  const pinnedTagSet = new Set<NativeTag>(pinnedTags);

  return (
    <Tooltip.Provider delayDuration={120}>
      <div className={styles.selectedTags} aria-label="Marcadores nativos da linha selecionada">
        {tags.map((tag) => {
          const isPinned = pinnedTagSet.has(tag);
          const isLineTag = lineTagSet.has(tag);

          return (
            <Tooltip.Root key={tag}>
              <Tooltip.Trigger asChild>
                <button
                  type="button"
                  aria-label={`${isPinned && !isLineTag ? "Desfixar" : "Remover"} ${tag}: ${nativeTagDescriptions[tag]}`}
                  data-pinned={isPinned}
                  onClick={() => {
                    if (isLineTag) {
                      onRemoveTag?.(tag);
                      return;
                    }
                    if (isPinned) {
                      onTogglePinnedTag?.(tag);
                    }
                  }}
                >
                  <span>{tag}</span>
                  {isPinned ? <Pin size={10} /> : null}
                </button>
              </Tooltip.Trigger>
              <Tooltip.Portal>
                <Tooltip.Content className={styles.tooltipContent} side="top" sideOffset={6}>
                  {nativeTagDescriptions[tag]}
                  <Tooltip.Arrow className={styles.tooltipArrow} />
                </Tooltip.Content>
              </Tooltip.Portal>
            </Tooltip.Root>
          );
        })}
      </div>
    </Tooltip.Provider>
  );
}

interface LinePropertiesSidebarProps {
  selectedFile: { metadata: { durationSeconds: number | null } | null } | null;
  selectedLineIndex: number;
  targetText: string;
  metadata: ProjectLineMetadata;
  settings: NativeSynthesisSettings;
  pinnedTags: readonly NativeTag[];
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
  settings,
  pinnedTags,
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
        settings={settings}
        pinnedTags={pinnedTags}
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
  settings: NativeSynthesisSettings;
  pinnedTags: readonly NativeTag[];
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
  settings,
  pinnedTags,
  controlsDisabled,
  sectionOpenState,
  onSectionOpenChange,
  onMetadataChange,
  onSettingsChange
}: LinePropertiesPanelProps) {
  const acceptedSettings = normalizeNativeSynthesisSettings(settings);
  const effectiveTagCount = effectiveNativeTags(metadata.tags, pinnedTags).length;
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
            title="Propriedades básicas"
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
                        ? (acceptedSettings.instruct ?? "mulher, jovem adulta, tom moderado")
                        : null
                  });
                }}
                items={[
                  { value: "clone", label: "Clonagem" },
                  { value: "design", label: "Desenho" },
                  { value: "auto", label: "Automático" }
                ]}
              />
            </ControlGroup>

            <ControlGroup label="Instrução">
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

            <ControlGroup label="Duração alvo">
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
                <dt>Duração atual</dt>
                <dd>{currentDuration ? `${currentDuration.toFixed(2)}s` : "-"}</dd>
              </div>
              <div>
                <dt>Marcadores</dt>
                <dd>{effectiveTagCount}</dd>
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
            title="Ajustes nativos globais"
            isOpen={sectionOpenState.nativeAdjustments}
            onOpenChange={(isOpen) => {
              onSectionOpenChange("nativeAdjustments", isOpen);
            }}
          >
            <NumberField
              label="Número de passos"
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
              label="Orientação"
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
              label="Temperatura de posição"
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
              label="Temperatura de classe"
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
              label="Redução de ruído"
              checked={acceptedSettings.denoise}
              disabled={controlsDisabled}
              onCheckedChange={(denoise) => {
                onSettingsChange({ denoise });
              }}
            />
            <NativeCheckbox
              label="Pré-processar instrução"
              checked={acceptedSettings.preprocessPrompt}
              disabled={controlsDisabled}
              onCheckedChange={(preprocessPrompt) => {
                onSettingsChange({ preprocessPrompt });
              }}
            />
            <NativeCheckbox
              label="Pós-processar saída"
              checked={acceptedSettings.postprocessOutput}
              disabled={controlsDisabled}
              onCheckedChange={(postprocessOutput) => {
                onSettingsChange({ postprocessOutput });
              }}
            />
          </CollapsibleSection>

          <CollapsibleSection
            title="Polimento de áudio global"
            isOpen={sectionOpenState.audioPolish}
            onOpenChange={(isOpen) => {
              onSectionOpenChange("audioPolish", isOpen);
            }}
          >
            <NativeCheckbox
              label="Nivelar volume automaticamente"
              checked={acceptedSettings.matchSourceLoudness}
              disabled={controlsDisabled}
              onCheckedChange={(matchSourceLoudness) => {
                onSettingsChange({ matchSourceLoudness });
              }}
            />
            <RangeField
              label="Quanto seguir o volume da origem"
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
              label="Redução de sibilância"
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
              label="Redução de metalizado"
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
    <section className={styles.propertyActions} aria-label="Controles de síntese">
      <button type="button" disabled={controlsDisabled} onClick={onSaveSettingsAsDefault}>
        <CheckCircle2 size={15} />
        Salvar ajustes globais
      </button>
      <button type="button" disabled={controlsDisabled} onClick={onResetSettingsToDefault}>
        <Undo2 size={15} />
        Restaurar globais
      </button>
      <button type="button" disabled={controlsDisabled || !canPreviewLine} onClick={onPreview}>
        <Play size={15} />
        Prévia desta linha
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
  return percent === 0 ? "desligado" : `-${String(percent)}%`;
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
    return "OmniVoice não suporta [pause] como marcador nativo. Use pontuação ou duração alvo para controlar pausas.";
  }

  return `Marcador OmniVoice não suportado: ${tag}`;
}

interface StartSelectedDubbingWithSaveDialogOptions {
  selectedFile: AudioFileEntry | null;
  outputDir: string | null;
  startDubbing: (saveOutputAs?: string | null) => Promise<void>;
  appendLog: (message: string, level?: "info" | "warning" | "error" | "success") => void;
}

async function startSelectedDubbingWithSaveDialog({
  selectedFile,
  outputDir,
  startDubbing,
  appendLog
}: StartSelectedDubbingWithSaveDialogOptions): Promise<void> {
  if (!selectedFile || !outputDir) {
    await startDubbing(null);
    return;
  }

  try {
    const saveOutputAs = await save({
      title: "Salvar arquivo dublado",
      defaultPath: joinNativePath(outputDir, dubbedOutputFileName(selectedFile.name)),
      filters: [{ name: "Audio WAV", extensions: ["wav"] }]
    });

    if (saveOutputAs === null) {
      appendLog("Dublagem cancelada: nenhum arquivo de destino foi escolhido.", "warning");
      return;
    }

    await startDubbing(saveOutputAs);
  } catch (unknownError: unknown) {
    appendLog(
      `Não foi possível abrir o diálogo de salvamento: ${dialogError(unknownError)}`,
      "error"
    );
  }
}

function dubbedOutputFileName(sourceFileName: string): string {
  const normalizedName = sourceFileName.trim() || "dublagem";
  const fileNameStart = Math.max(normalizedName.lastIndexOf("/"), normalizedName.lastIndexOf("\\"));
  const fileName = normalizedName.slice(fileNameStart + 1);
  const extensionStart = fileName.lastIndexOf(".");
  const stem = extensionStart > 0 ? fileName.slice(0, extensionStart) : fileName;
  return `${stem || "dublagem"}_dublado.wav`;
}

function joinNativePath(directory: string, fileName: string): string {
  const separator = directory.includes("\\") && !directory.includes("/") ? "\\" : "/";
  return `${directory.replace(/[\\/]+$/, "")}${separator}${fileName}`;
}

function dialogError(unknownError: unknown): string {
  return unknownError instanceof Error ? unknownError.message : String(unknownError);
}
