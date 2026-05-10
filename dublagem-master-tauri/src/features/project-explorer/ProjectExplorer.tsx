import * as Tooltip from "@radix-ui/react-tooltip";
import { ChevronDown, ChevronRight, FileAudio, Filter, Plus, RefreshCw } from "lucide-react";
import { useEffect, useMemo, useState, type Dispatch, type SetStateAction } from "react";
import { nativeTagDescriptions, nativeTagGroups } from "../../shared/omnivoice/nativeControls";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import styles from "./ProjectExplorer.module.css";

const TAG_PALETTE_OPEN_STORAGE_KEY = "nsg-gaming-dub.tag-palette-open.v1";

export function ProjectExplorer() {
  const files = useWorkspaceStore((state) => state.files);
  const selectedPath = useWorkspaceStore((state) => state.selectedPath);
  const selectFile = useWorkspaceStore((state) => state.selectFile);
  const scan = useWorkspaceStore((state) => state.scan);
  const insertNativeTag = useWorkspaceStore((state) => state.insertNativeTag);
  const [familyFilter, setFamilyFilter] = useState<string>("all");
  const [isTagPaletteOpen, setIsTagPaletteOpen] = usePersistentTagPaletteOpenState();

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

      <section className={styles.list} aria-label="Arquivos do projeto">
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
      </section>

      <section className={styles.tagPalette} aria-label="Paleta de tags OmniVoice">
        <button
          type="button"
          className={styles.paletteHeader}
          aria-expanded={isTagPaletteOpen}
          aria-controls="tag-palette-content"
          onClick={() => {
            setIsTagPaletteOpen((current) => !current);
          }}
        >
          {isTagPaletteOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          <strong>Paleta de tags</strong>
        </button>

        {isTagPaletteOpen && (
          <div id="tag-palette-content" className={styles.paletteContent}>
            {nativeTagGroups.map((group) => (
              <div key={group.id} className={styles.tagGroup}>
                <span>{group.label}</span>
                <div>
                  <Tooltip.Provider delayDuration={120}>
                    {group.tags.map((tag) => (
                      <Tooltip.Root key={tag}>
                        <Tooltip.Trigger asChild>
                          <button
                            type="button"
                            disabled={!selectedPath}
                            data-tag={tag}
                            onClick={() => {
                              insertNativeTag(tag);
                            }}
                          >
                            {tag}
                          </button>
                        </Tooltip.Trigger>
                        <Tooltip.Portal>
                          <Tooltip.Content
                            className={styles.tooltipContent}
                            side="right"
                            sideOffset={6}
                          >
                            {nativeTagDescriptions[tag]}
                            <Tooltip.Arrow className={styles.tooltipArrow} />
                          </Tooltip.Content>
                        </Tooltip.Portal>
                      </Tooltip.Root>
                    ))}
                  </Tooltip.Provider>
                </div>
              </div>
            ))}
            <button type="button" className={styles.manageTags} disabled>
              <Plus size={14} />
              Tags nativas bloqueadas
            </button>
          </div>
        )}
      </section>
    </aside>
  );
}

function usePersistentTagPaletteOpenState(): readonly [boolean, Dispatch<SetStateAction<boolean>>] {
  const [isOpen, setIsOpen] = useState(readTagPaletteOpenState);

  useEffect(() => {
    writeTagPaletteOpenState(isOpen);
  }, [isOpen]);

  return [isOpen, setIsOpen];
}

function readTagPaletteOpenState(): boolean {
  if (typeof window === "undefined") {
    return true;
  }

  const storedValue = localStorage.getItem(TAG_PALETTE_OPEN_STORAGE_KEY);
  if (!storedValue) {
    return true;
  }

  try {
    const parsedValue: unknown = JSON.parse(storedValue);
    return typeof parsedValue === "boolean" ? parsedValue : true;
  } catch {
    localStorage.removeItem(TAG_PALETTE_OPEN_STORAGE_KEY);
    return true;
  }
}

function writeTagPaletteOpenState(isOpen: boolean): void {
  if (typeof window === "undefined") {
    return;
  }

  try {
    localStorage.setItem(TAG_PALETTE_OPEN_STORAGE_KEY, JSON.stringify(isOpen));
  } catch {
    return;
  }
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
