import { convertFileSrc } from "@tauri-apps/api/core";
import { Pause, Play } from "lucide-react";
import { useMemo, useRef, useState } from "react";
import styles from "./AudioPlayer.module.css";

interface AudioPlayerProps {
  title: string;
  path: string | null;
}

export function AudioPlayer({ title, path }: AudioPlayerProps) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const source = useMemo(() => (path ? convertFileSrc(path) : null), [path]);

  async function toggle(): Promise<void> {
    const audio = audioRef.current;
    if (!audio || !source) {
      return;
    }
    if (audio.paused) {
      await audio.play();
      setIsPlaying(true);
    } else {
      audio.pause();
      setIsPlaying(false);
    }
  }

  const Icon = isPlaying ? Pause : Play;

  return (
    <section className={styles.player}>
      <div className={styles.header}>
        <span>{title}</span>
        <button
          type="button"
          aria-label={isPlaying ? "Pausar audio" : "Tocar audio"}
          onClick={() => {
            void toggle();
          }}
        >
          <Icon size={16} />
        </button>
      </div>
      {source ? (
        <audio
          ref={audioRef}
          src={source}
          controls
          onPause={() => {
            setIsPlaying(false);
          }}
          onEnded={() => {
            setIsPlaying(false);
          }}
        />
      ) : (
        <div className={styles.empty}>Nenhum audio selecionado</div>
      )}
    </section>
  );
}
