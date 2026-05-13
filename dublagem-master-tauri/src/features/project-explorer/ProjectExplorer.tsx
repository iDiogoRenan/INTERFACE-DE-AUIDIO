import * as Tooltip from "@radix-ui/react-tooltip";
import {
  ChevronDown,
  ChevronRight,
  FileAudio,
  Filter,
  FolderOpen,
  FolderInput,
  FolderOutput,
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
  type ReactElement,
  type SetStateAction
} from "react";
import {
  nativeTagDescriptions,
  nativeTagGroups,
  type NativeTag
} from "../../shared/omnivoice/nativeControls";
import { openDirectoryLocation, revealFileLocation } from "../../shared/tauri/openLocation";
import { hasConfiguredInputDirectory } from "../../shared/tauri/configGuards";
import { pickDirectory } from "../../shared/tauri/pathPicker";
import type { AppConfig } from "../../shared/tauri/types";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import styles from "./ProjectExplorer.module.css";

const TAG_PALETTE_OPEN_STORAGE_KEY = "nsg-gaming-dub.tag-palette-open.v1";
const CONTEXT_MENU_WIDTH = 164;
const CONTEXT_MENU_HEIGHT = 38;

interface TagContextMenuState {
  tag: NativeTag;
  x: number;
  y: number;
}

interface LocationContextMenuState {
  path: string;
  target: "directory" | "file";
  x: number;
  y: number;
}

type WorkspaceDirectoryKey = "inputDir" | "outputDir";

const workspaceDirectoryActions = [
  { configKey: "inputDir", label: "Pasta de entrada", Icon: FolderInput },
  { configKey: "outputDir", label: "Pasta de saída", Icon: FolderOutput }
] as const satisfies readonly {
  configKey: WorkspaceDirectoryKey;
  label: string;
  Icon: typeof FolderInput;
}[];

export function ProjectExplorer() {
  const config = useWorkspaceStore((state) => state.config);
  const files = useWorkspaceStore((state) => state.files);
  const selectedPath = useWorkspaceStore((state) => state.selectedPath);
  const saveConfig = useWorkspaceStore((state) => state.saveConfig);
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
  const [activeDirectoryPicker, setActiveDirectoryPicker] = useState<WorkspaceDirectoryKey | null>(
    null
  );
  const [tagContextMenu, setTagContextMenu] = useState<TagContextMenuState | null>(null);
  const [locationContextMenu, setLocationContextMenu] = useState<LocationContextMenuState | null>(
    null
  );

  const families = useMemo(
    () => ["all", ...Array.from(new Set(files.map((file) => file.family))).sort()],
    [files]
  );
  const visibleFiles =
    familyFilter === "all" ? files : files.filter((file) => file.family === familyFilter);
  const visibleFilePaths = visibleFiles.map((file) => file.path);

  useEffect(() => {
    if (!tagContextMenu && !locationContextMenu) {
      return;
    }

    const closeMenu = () => {
      setTagContextMenu(null);
      setLocationContextMenu(null);
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
  }, [tagContextMenu, locationContextMenu]);

  function openTagContextMenu(event: ReactMouseEvent<HTMLButtonElement>, tag: NativeTag): void {
    event.preventDefault();
    event.stopPropagation();
    setLocationContextMenu(null);
    setTagContextMenu({
      tag,
      ...contextMenuPosition(event.clientX, event.clientY)
    });
  }

  function openLocationContextMenu(
    event: ReactMouseEvent<HTMLElement>,
    path: string | null,
    target: LocationContextMenuState["target"]
  ): void {
    event.preventDefault();
    event.stopPropagation();
    setTagContextMenu(null);

    if (!path) {
      setLocationContextMenu(null);
      appendLog("Nenhuma pasta configurada para abrir.", "warning");
      return;
    }

    setLocationContextMenu({
      path,
      target,
      ...contextMenuPosition(event.clientX, event.clientY)
    });
  }

  async function goToLocation(menu: LocationContextMenuState): Promise<void> {
    setLocationContextMenu(null);
    try {
      if (menu.target === "directory") {
        await openDirectoryLocation(menu.path);
        return;
      }

      await revealFileLocation(menu.path);
    } catch (unknownError: unknown) {
      appendLog(locationError(unknownError), "error");
    }
  }

  async function selectWorkspaceDirectory(configKey: WorkspaceDirectoryKey): Promise<void> {
    if (activeDirectoryPicker) {
      return;
    }

    setActiveDirectoryPicker(configKey);
    try {
      const selectedDirectory = await pickDirectory();
      if (!selectedDirectory) {
        return;
      }

      const nextConfig: AppConfig = { ...config, [configKey]: selectedDirectory };
      await saveConfig(nextConfig);
      if (hasConfiguredInputDirectory(nextConfig)) {
        await scan();
      }
    } catch (unknownError: unknown) {
      appendLog(directoryPickerError(unknownError), "error");
    } finally {
      setActiveDirectoryPicker(null);
    }
  }

  return (
    <aside className={styles.panel} aria-label="Explorador do projeto">
      <div className={styles.header}>
        <div className={styles.title}>
          <span className={styles.kicker}>Projeto</span>
          <h2>Arquivos</h2>
        </div>
        <div className={styles.headerActions} aria-label="Ações do projeto">
          <Tooltip.Provider delayDuration={120}>
            {workspaceDirectoryActions.map((action) => {
              const Icon = action.Icon;
              const isPickingThisDirectory = activeDirectoryPicker === action.configKey;
              const configuredPath = config[action.configKey];
              return (
                <HeaderActionTooltip key={action.configKey} label={action.label}>
                  <button
                    type="button"
                    className={styles.iconButton}
                    aria-label={action.label}
                    disabled={activeDirectoryPicker !== null}
                    onClick={() => {
                      void selectWorkspaceDirectory(action.configKey);
                    }}
                    onContextMenu={(event) => {
                      openLocationContextMenu(event, configuredPath, "directory");
                    }}
                  >
                    {isPickingThisDirectory ? (
                      <Loader2 size={16} className={styles.spin} />
                    ) : (
                      <Icon size={16} />
                    )}
                  </button>
                </HeaderActionTooltip>
              );
            })}
            <HeaderActionTooltip label="Atualizar lista">
              <button
                type="button"
                className={styles.iconButton}
                aria-label="Atualizar lista"
                disabled={activeDirectoryPicker !== null}
                onClick={() => {
                  void scan();
                }}
              >
                <RefreshCw size={16} />
              </button>
            </HeaderActionTooltip>
          </Tooltip.Provider>
        </div>
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
            className={`${styles.item} ${file.path === selectedPath ? styles.selectedItem : ""}`}
            onClick={() => {
              selectFile(file.path);
            }}
            onContextMenu={(event) => {
              selectFile(file.path);
              openLocationContextMenu(event, file.path, "file");
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
        <div className={styles.contextMenu} role="menu" style={contextMenuStyle(tagContextMenu)}>
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

      {locationContextMenu ? (
        <div
          className={styles.contextMenu}
          role="menu"
          style={contextMenuStyle(locationContextMenu)}
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              void goToLocation(locationContextMenu);
            }}
          >
            <FolderOpen size={14} />
            Ir para
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
    x: Math.max(0, Math.min(clientX, window.innerWidth - CONTEXT_MENU_WIDTH)),
    y: Math.max(0, Math.min(clientY, window.innerHeight - CONTEXT_MENU_HEIGHT))
  };
}

function contextMenuStyle(position: Pick<TagContextMenuState, "x" | "y">) {
  return {
    insetInlineStart: position.x,
    insetBlockStart: position.y
  };
}

interface HeaderActionTooltipProps {
  label: string;
  children: ReactElement;
}

function HeaderActionTooltip({ label, children }: HeaderActionTooltipProps) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>{children}</Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={styles.tooltipContent} side="bottom" sideOffset={6}>
          {label}
          <Tooltip.Arrow className={styles.tooltipArrow} />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
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
    ignored: "Ignorado",
    missing_source: "Sem origem",
    failed: "Falhou"
  };
  return labels[status] ?? status;
}

function directoryPickerError(unknownError: unknown): string {
  const details =
    unknownError instanceof Error
      ? unknownError.message
      : "Erro desconhecido ao abrir o seletor de pasta.";
  return `Não foi possível selecionar a pasta: ${details}`;
}

function locationError(unknownError: unknown): string {
  const details =
    unknownError instanceof Error ? unknownError.message : "Erro desconhecido ao abrir a pasta.";
  return `Não foi possível abrir no Explorador: ${details}`;
}
