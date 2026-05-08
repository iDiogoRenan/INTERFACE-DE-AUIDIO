import { Check, RotateCcw, X } from "lucide-react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { tauriClient } from "../../shared/tauri/client";
import styles from "./ValidationPanel.module.css";

export function ValidationPanel() {
  const config = useWorkspaceStore((state) => state.config);
  const selectedPath = useWorkspaceStore((state) => state.selectedPath);
  const appendLog = useWorkspaceStore((state) => state.appendLog);
  const startDubbing = useWorkspaceStore((state) => state.startDubbing);

  async function approve(): Promise<void> {
    if (!selectedPath || !config.approvedDir) {
      appendLog("Selecione arquivo e pasta de aprovados.", "warning");
      return;
    }
    await tauriClient.approveFile(selectedPath, config.approvedDir);
    appendLog("Arquivo aprovado.", "success");
  }

  async function reject(): Promise<void> {
    if (!selectedPath || !config.outputDir) {
      appendLog("Selecione arquivo e pasta de destino.", "warning");
      return;
    }
    await tauriClient.rejectFile(selectedPath, `${config.outputDir}/rejeitados_revisao_manual`);
    appendLog("Arquivo marcado para revisao manual.", "warning");
  }

  return (
    <div className={styles.panel}>
      <header>
        <h2>Validação manual</h2>
        <p>Revise o áudio selecionado, aprove para a pasta final ou retorne para redublagem.</p>
      </header>
      <div className={styles.actions}>
        <button
          type="button"
          onClick={() => {
            void approve();
          }}
        >
          <Check size={16} />
          Aprovar
        </button>
        <button
          type="button"
          onClick={() => {
            void reject();
          }}
        >
          <X size={16} />
          Rejeitar
        </button>
        <button
          type="button"
          onClick={() => {
            void startDubbing();
          }}
        >
          <RotateCcw size={16} />
          Redublar
        </button>
      </div>
    </div>
  );
}
