import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, FileAudio } from "lucide-react";
import styles from "./PathField.module.css";

interface PathFieldProps {
  label: string;
  value: string | null;
  mode: "directory" | "file";
  placeholder: string;
  onChange: (value: string | null) => void;
}

export function PathField({ label, value, mode, placeholder, onChange }: PathFieldProps) {
  async function pickPath(): Promise<void> {
    const selected = await open({
      directory: mode === "directory",
      multiple: false,
      filters:
        mode === "file"
          ? [{ name: "Audio", extensions: ["wav", "mp3", "wem", "ogg", "flac"] }]
          : undefined
    });
    onChange(typeof selected === "string" ? selected : null);
  }

  const Icon = mode === "directory" ? FolderOpen : FileAudio;

  return (
    <label className={styles.field}>
      <span>{label}</span>
      <div className={styles.control}>
        <input value={value ?? ""} placeholder={placeholder} readOnly />
        <button
          type="button"
          aria-label={`Selecionar ${label}`}
          onClick={() => {
            void pickPath();
          }}
        >
          <Icon size={16} />
        </button>
      </div>
    </label>
  );
}
