import { CheckCircle2, Circle, Loader2, Mic2, Square, Wand2 } from "lucide-react";
import { AudioPlayer } from "../audio-player/AudioPlayer";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import type { JobStage } from "../../shared/tauri/types";
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

export function DubbingPanel() {
  const config = useWorkspaceStore((state) => state.config);
  const selectedPath = useWorkspaceStore((state) => state.selectedPath);
  const sourceText = useWorkspaceStore((state) => state.sourceText);
  const targetText = useWorkspaceStore((state) => state.targetText);
  const setSourceText = useWorkspaceStore((state) => state.setSourceText);
  const setTargetText = useWorkspaceStore((state) => state.setTargetText);
  const startDubbing = useWorkspaceStore((state) => state.startDubbing);
  const cancelJob = useWorkspaceStore((state) => state.cancelJob);
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

  return (
    <div className={styles.layout}>
      <div className={styles.players}>
        <AudioPlayer title="Origem" path={selectedPath} />
        <AudioPlayer title="Resultado" path={lastOutputPath} />
      </div>

      <section className={styles.editorGrid}>
        <label>
          <span>Texto origem</span>
          <textarea
            value={sourceText}
            onChange={(event) => {
              setSourceText(event.currentTarget.value);
            }}
          />
        </label>
        <label>
          <span>Texto destino</span>
          <textarea
            value={targetText}
            onChange={(event) => {
              setTargetText(event.currentTarget.value);
            }}
          />
        </label>
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
            className={styles.primary}
            disabled={isBusy}
            onClick={() => {
              void startDubbing();
            }}
          >
            {isBusy ? <Loader2 size={16} className={styles.spin} /> : <Wand2 size={16} />}
            Dublar selecionado
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

      <section className={styles.logPanel}>
        {logs.map((entry) => (
          <p key={entry.id} data-level={entry.level}>
            {entry.message}
          </p>
        ))}
      </section>
    </div>
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
