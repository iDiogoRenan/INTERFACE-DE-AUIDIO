import { FileAudio, FolderOpen } from "lucide-react";
import { useEffect, useState, type MouseEvent as ReactMouseEvent } from "react";
import { openDirectoryLocation, revealFileLocation } from "../tauri/openLocation";
import { pickPath } from "../tauri/pathPicker";
import styles from "./PathField.module.css";

interface PathFieldProps {
  label: string;
  value: string | null;
  mode: "directory" | "file";
  placeholder: string;
  onChange: (value: string | null) => void;
  onOpenError?: (message: string) => void;
}

interface PathMenuState {
  x: number;
  y: number;
}

const PATH_MENU_WIDTH = 156;
const PATH_MENU_HEIGHT = 38;

export function PathField({
  label,
  value,
  mode,
  placeholder,
  onChange,
  onOpenError
}: PathFieldProps) {
  const [pathMenu, setPathMenu] = useState<PathMenuState | null>(null);

  useEffect(() => {
    if (!pathMenu) {
      return;
    }

    const closeMenu = () => {
      setPathMenu(null);
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
  }, [pathMenu]);

  async function pickSelectedPath(): Promise<void> {
    const selected = await pickPath(mode);
    onChange(selected);
  }

  function openPathContextMenu(event: ReactMouseEvent<HTMLElement>): void {
    event.preventDefault();
    event.stopPropagation();
    if (!value) {
      setPathMenu(null);
      return;
    }

    setPathMenu(contextMenuPosition(event.clientX, event.clientY));
  }

  async function goToPath(): Promise<void> {
    setPathMenu(null);
    if (!value) {
      return;
    }

    try {
      if (mode === "directory") {
        await openDirectoryLocation(value);
        return;
      }

      await revealFileLocation(value);
    } catch (unknownError: unknown) {
      onOpenError?.(locationError(unknownError));
    }
  }

  const Icon = mode === "directory" ? FolderOpen : FileAudio;

  return (
    <label className={styles.field}>
      <span>{label}</span>
      <div className={styles.control} onContextMenu={openPathContextMenu}>
        <input value={value ?? ""} placeholder={placeholder} readOnly />
        <button
          type="button"
          aria-label={`Selecionar ${label}`}
          onClick={() => {
            void pickSelectedPath();
          }}
          onContextMenu={openPathContextMenu}
        >
          <Icon size={16} />
        </button>
      </div>
      {pathMenu ? (
        <div
          className={styles.contextMenu}
          role="menu"
          style={{
            insetInlineStart: pathMenu.x,
            insetBlockStart: pathMenu.y
          }}
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              void goToPath();
            }}
          >
            <FolderOpen size={14} />
            Ir para
          </button>
        </div>
      ) : null}
    </label>
  );
}

function contextMenuPosition(clientX: number, clientY: number): PathMenuState {
  if (typeof window === "undefined") {
    return { x: clientX, y: clientY };
  }

  return {
    x: Math.max(0, Math.min(clientX, window.innerWidth - PATH_MENU_WIDTH)),
    y: Math.max(0, Math.min(clientY, window.innerHeight - PATH_MENU_HEIGHT))
  };
}

function locationError(unknownError: unknown): string {
  const details =
    unknownError instanceof Error ? unknownError.message : "Erro desconhecido ao abrir a pasta.";
  return `Não foi possível abrir no Explorador: ${details}`;
}
