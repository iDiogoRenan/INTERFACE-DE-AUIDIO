import { FolderOpen, FileAudio } from "lucide-react";
import { pickPath } from "../tauri/pathPicker";
import styles from "./PathField.module.css";

interface PathFieldProps {
  label: string;
  value: string | null;
  mode: "directory" | "file";
  placeholder: string;
  onChange: (value: string | null) => void;
}

export function PathField({ label, value, mode, placeholder, onChange }: PathFieldProps) {
  async function pickSelectedPath(): Promise<void> {
    const selected = await pickPath(mode);
    onChange(selected);
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
            void pickSelectedPath();
          }}
        >
          <Icon size={16} />
        </button>
      </div>
    </label>
  );
}
