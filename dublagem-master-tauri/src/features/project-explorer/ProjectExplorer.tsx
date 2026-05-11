import * as Tooltip from "@radix-ui/react-tooltip";
import {
  ChevronDown,
  ChevronRight,
  FileAudio,
  Filter,
  ListChecks,
  Loader2,
  Pin,
  Plus,
  RefreshCw
} from "lucide-react";
import {
  useEffect,
  useMemo,
  useState,
  type Dispatch,
  type MouseEvent as ReactMouseEvent,
  type SetStateAction
} from "react";
import {
  nativeTagDescriptions,
  nativeTagGroups,
  type NativeTag
} from "../../shared/omnivoice/nativeControls";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import styles from "./ProjectExplorer.module.css";

const TAG_PALETTE_OPEN_STORAGE_KEY = "nsg-gaming-dub.tag-palette-open.v1";
const TAG_CONTEXT_MENU_WIDTH = 164;
const TAG_CONTEXT_MENU_HEIGHT = 38;

interface TagContextMenuState {
  tag: NativeTag;
  x: number;
  y: number;
}

export function ProjectExplorer() {
  const files = useWorkspaceStore((state) => state.files);
  const selectedPath = useWorkspaceStore((state) => state.selectedPath);
  const selectFile = useWorkspaceStore((state) => state.selectFile);
  const scan = useWorkspaceStore((state) => state.scan);
  const startDubbingList = useWorkspaceStore((state) => state.startDubbingList);
  const insertNativeTag = useWorkspaceStore((state) => state.insertNativeTag);
  const pinnedNativeTags = useWorkspaceStore((state) => state.pinnedNativeTags);
  const togglePinnedNativeTag = useWorkspaceStore((state) => state.togglePinnedNativeTag);
  const appendLog = useWorkspaceStore((state) => state.appendLog);
  const isBusy = useWorkspaceStore((state) => state.isBusy);
  const [familyFilter, setFamilyFilter] = useState<string>("all");
  const [isTagPaletteOpen, setIsTagPaletteOpen] = usePersistentTagPaletteOpenState();
  const [tagContextMenu, setTagContextMenu] = useState<TagContextMenuState | null>(null);

  const families = useMemo(
    () => ["all", ...Array.from(new Set(files.map((file) => file.family))).sort()],
    [files]
  );
  const visibleFiles =
    familyFilter === "all" ? files : files.filter((file) => file.family === familyFilter);
  const visibleFilePaths = visibleFiles.map((file) => file.path);

  useEffect(() => {
    if (!tagContextMenu) {
      return;
    }

    const closeMenu = () => {
      setTagContextMenu(null);
    };
    const closeMenuOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeMenu();
      }
    };

    window.addEventListener("click", closeMenu);
    window.addEventListener("blur", closeMenu);
    window.addEventListener("resize", closeMenu);
    window.addEventListener("scroll", closeMenu, true);
    window.addEventListener("keydown", closeMenuOnEscape);

    return () => {
      window.removeEventListener("click", closeMenu);
      window.removeEventListener("blur", closeMenu);
      window.removeEventListener("resize", closeMenu);
      window.removeEventListener("scroll", closeMenu, true);
      window.removeEventListener("keydown", closeMenuOnEscape);
    };
  }, [tagContextMenu]);

  function openTagContextMenu(event: ReactMouseEvent<HTMLButtonElement>, tag: NativeTag): void {
    event.preventDefault();
    event.stopPropagation();
    setTagContextMenu({ tag, ...contextMenuPosition(event.clientX, event.clientY) });
  }

  return (
    <aside className={styles.panel} aria-label="Explorador do projeto">
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
              {family === "all" ? "Todas as famílias" : family}
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

      <section className={styles.listActions} aria-label="Ações da lista filtrada">
        <button
          type="button"
          className={styles.listDubbingButton}
          disabled={isBusy || visibleFilePaths.length === 0}
          onClick={() => {
            void startDubbingList(visibleFilePaths);
          }}
        >
          {isBusy ? <Loader2 size={15} className={styles.spin} /> : <ListChecks size={15} />}
          {isBusy ? "Processando" : "Dublar lista"}
        </button>
      </section>

      <section className={styles.tagPalette} aria-label="Paleta de marcadores OmniVoice">
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
          <strong>Paleta de marcadores</strong>
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
                            data-tag={tag}
                            data-pinned={pinnedNativeTags.includes(tag)}
                            data-disabled={!selectedPath}
                            onContextMenu={(event) => {
                              openTagContextMenu(event, tag);
                            }}
                            onClick={() => {
                              if (selectedPath) {
                                insertNativeTag(tag);
                                return;
                              }
                              appendLog(
                                "Selecione um arquivo antes de aplicar o marcador à linha.",
                                "warning"
                              );
                            }}
                          >
                            <span>{tag}</span>
                            {pinnedNativeTags.includes(tag) ? <Pin size={11} /> : null}
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
              Marcadores nativos bloqueados
            </button>
          </div>
        )}
      </section>

      {tagContextMenu ? (
        <div
          className={styles.tagContextMenu}
          role="menu"
          style={{
            insetInlineStart: tagContextMenu.x,
            insetBlockStart: tagContextMenu.y
          }}
          onClick={(event) => {
            event.stopPropagation();
          }}
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              togglePinnedNativeTag(tagContextMenu.tag);
              setTagContextMenu(null);
            }}
          >
            <Pin size={14} />
            {pinnedNativeTags.includes(tagContextMenu.tag) ? "Desfixar tag" : "Fixar tag"}
          </button>
        </div>
      ) : null}
    </aside>
  );
}

function contextMenuPosition(
  clientX: number,
  clientY: number
): Pick<TagContextMenuState, "x" | "y"> {
  if (typeof window === "undefined") {
    return { x: clientX, y: clientY };
  }

  return {
    x: Math.max(0, Math.min(clientX, window.innerWidth - TAG_CONTEXT_MENU_WIDTH)),
    y: Math.max(0, Math.min(clientY, window.innerHeight - TAG_CONTEXT_MENU_HEIGHT))
  };
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
    ignored: "Ignorado",
    missing_source: "Sem origem",
    failed: "Falhou"
  };
  return labels[status] ?? status;
}
