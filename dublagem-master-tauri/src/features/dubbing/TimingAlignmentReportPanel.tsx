import { Check, Pencil, RotateCcw } from "lucide-react";
import type {
  TimingAlignmentChunkReport,
  TimingAlignmentReport,
  TimingChunkStatus
} from "../../shared/tauri/types";
import styles from "./TimingAlignmentReportPanel.module.css";

export interface TimingAlignmentReportPanelProps {
  report: TimingAlignmentReport | null;
  isBusy: boolean;
  onRegenerate: () => void;
  onEditChunk: (chunkIndex: number) => void;
  onAcceptChunk: (chunk: TimingAlignmentChunkReport) => void;
}

export function TimingAlignmentReportPanel({
  report,
  isBusy,
  onRegenerate,
  onEditChunk,
  onAcceptChunk
}: TimingAlignmentReportPanelProps) {
  if (!report) {
    return null;
  }

  return (
    <section className={styles.reportPanel} aria-label="Relatório de sincronização temporal">
      <header>
        <div>
          <span>Sincronização temporal</span>
          <strong>
            {report.totalChunks} chunk(s) · limite {report.configuredChunkLimit}
          </strong>
        </div>
        <output data-critical={report.hasCriticalChunks}>
          {report.hasCriticalChunks ? "Revisão" : "OK"}
        </output>
      </header>

      {report.warnings.length > 0 && (
        <ul className={styles.reportWarnings}>
          {report.warnings.map((warning) => (
            <li key={warning}>{warning}</li>
          ))}
        </ul>
      )}

      <div className={styles.chunkList}>
        {report.chunks.map((chunk) => (
          <article key={chunk.segmentId} className={styles.chunkCard}>
            <div className={styles.chunkHeader}>
              <strong>
                {chunk.chunkIndex}/{chunk.totalChunks}
              </strong>
              <span>{statusLabelForChunk(chunk.statuses)}</span>
            </div>
            <dl>
              <div>
                <dt>Início</dt>
                <dd>{formatSeconds(chunk.startOriginal)}</dd>
              </div>
              <div>
                <dt>Fim</dt>
                <dd>{formatSeconds(chunk.endOriginal)}</dd>
              </div>
              <div>
                <dt>Origem</dt>
                <dd>{formatSeconds(chunk.durationOriginal)}</dd>
              </div>
              <div>
                <dt>Gerado</dt>
                <dd>
                  {chunk.durationGenerated === null ? "-" : formatSeconds(chunk.durationGenerated)}
                </dd>
              </div>
              <div>
                <dt>Dif.</dt>
                <dd>
                  {chunk.durationDifferencePercent === null
                    ? "-"
                    : `${chunk.durationDifferencePercent.toFixed(1)}%`}
                </dd>
              </div>
              <div>
                <dt>Stretch</dt>
                <dd>{chunk.stretchRatio === null ? "-" : `${chunk.stretchRatio.toFixed(2)}x`}</dd>
              </div>
            </dl>
            <p>{chunk.textoPtbr}</p>
            <div className={styles.chunkActions}>
              <button type="button" disabled={isBusy} onClick={onRegenerate}>
                <RotateCcw size={14} />
                Reprocessar
              </button>
              <button
                type="button"
                disabled={isBusy}
                onClick={() => {
                  onEditChunk(chunk.chunkIndex);
                }}
              >
                <Pencil size={14} />
                Editar
              </button>
              <button
                type="button"
                disabled={isBusy}
                onClick={() => {
                  onAcceptChunk(chunk);
                }}
              >
                <Check size={14} />
                Aceitar
              </button>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

function formatSeconds(value: number): string {
  return `${value.toFixed(2)}s`;
}

function statusLabelForChunk(statuses: TimingChunkStatus[]): string {
  const priority = [
    "needs_manual_review",
    "overlap_risk",
    "out_of_limit",
    "abrupt_ending_detected",
    "time_stretched",
    "text_adapted",
    "regenerated",
    "batch_processed",
    "chunk_limit_exceeded",
    "ok"
  ] satisfies TimingChunkStatus[];
  const fallbackStatus: TimingChunkStatus = statuses.length > 0 ? statuses[0] : "ok";
  const status = priority.find((item) => statuses.includes(item)) ?? fallbackStatus;
  const labels: Record<TimingChunkStatus, string> = {
    ok: "OK",
    time_stretched: "Stretch",
    regenerated: "Regenerado",
    text_adapted: "Texto adaptado",
    out_of_limit: "Fora do limite",
    needs_manual_review: "Revisão",
    overlap_risk: "Risco de overlap",
    abrupt_ending_detected: "Final abrupto",
    bad_reference: "Referência ruim",
    tts_failed: "Falha TTS",
    chunk_limit_exceeded: "Acima do limite",
    awaiting_confirmation: "Aguardando",
    batch_processed: "Lote"
  };
  return labels[status];
}
