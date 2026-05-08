import { Loader2, Mic2, Square, Wand2 } from "lucide-react";
import { AudioPlayer } from "../audio-player/AudioPlayer";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import styles from "./DubbingPanel.module.css";

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
  const progress = useWorkspaceStore((state) => state.progress);
  const logs = useWorkspaceStore((state) => state.logs);

  return (
    <div className={styles.layout}>
      <div className={styles.players}>
        <AudioPlayer title="Origem" path={selectedPath} />
        <AudioPlayer title="Resultado" path={null} />
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
            disabled={!isBusy}
            onClick={() => {
              void cancelJob();
            }}
          >
            <Square size={15} />
            Cancelar
          </button>
        </div>
      </section>

      <section className={styles.progress}>
        <div style={{ inlineSize: `${String(progress)}%` }} />
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
