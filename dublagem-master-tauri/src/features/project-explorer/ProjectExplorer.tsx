import { FileAudio, Filter, RefreshCw } from "lucide-react";
import { useMemo, useState } from "react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import styles from "./ProjectExplorer.module.css";

export function ProjectExplorer() {
  const files = useWorkspaceStore((state) => state.files);
  const selectedPath = useWorkspaceStore((state) => state.selectedPath);
  const selectFile = useWorkspaceStore((state) => state.selectFile);
  const scan = useWorkspaceStore((state) => state.scan);
  const [familyFilter, setFamilyFilter] = useState<string>("all");

  const families = useMemo(
    () => ["all", ...Array.from(new Set(files.map((file) => file.family))).sort()],
    [files]
  );
  const visibleFiles =
    familyFilter === "all" ? files : files.filter((file) => file.family === familyFilter);

  return (
    <aside className={styles.panel}>
      <div className={styles.header}>
        <div>
          <span className={styles.kicker}>Projeto</span>
          <h2>Arquivos</h2>
        </div>
        <button
          type="button"
          className={styles.iconButton}
          aria-label="Atualizar lista"
          onClick={() => {
            void scan();
          }}
        >
          <RefreshCw size={16} />
        </button>
      </div>

      <label className={styles.filter}>
        <Filter size={14} />
        <select
          value={familyFilter}
          onChange={(event) => {
            setFamilyFilter(event.currentTarget.value);
          }}
        >
          {families.map((family) => (
            <option key={family} value={family}>
              {family === "all" ? "Todas as familias" : family}
            </option>
          ))}
        </select>
      </label>

      <div className={styles.list}>
        {visibleFiles.map((file) => (
          <button
            key={file.path}
            type="button"
            className={file.path === selectedPath ? styles.selectedItem : styles.item}
            onClick={() => {
              selectFile(file.path);
            }}
          >
            <FileAudio size={15} />
            <span className={styles.fileName}>{file.name}</span>
            <span className={styles.status} data-status={file.status}>
              {statusLabel(file.status)}
            </span>
          </button>
        ))}
      </div>
    </aside>
  );
}

function statusLabel(status: string): string {
  const labels: Record<string, string> = {
    pending: "Pendente",
    dubbed: "Dublado",
    approved: "Aprovado",
    rejected: "Rejeitado",
    missing_source: "Sem origem",
    failed: "Falhou"
  };
  return labels[status] ?? status;
}
