import { convertFileSrc } from "@tauri-apps/api/core";
import { Pause, Play } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { isTauriRuntime, tauriClient } from "../../shared/tauri/client";
import styles from "./AudioPlayer.module.css";

interface AudioPlayerProps {
  title: string;
  path: string | null;
}

interface PreviewState {
  path: string;
  source: string | null;
  error: string | null;
}

interface PlaybackError {
  path: string;
  message: string;
}

export function AudioPlayer({ title, path }: AudioPlayerProps) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [preview, setPreview] = useState<PreviewState | null>(null);
  const [playbackError, setPlaybackError] = useState<PlaybackError | null>(null);
  const currentPreview = preview?.path === path ? preview : null;
  const source = currentPreview?.source ?? null;
  const error =
    playbackError?.path === path ? playbackError.message : (currentPreview?.error ?? null);

  useEffect(() => {
    let cancelled = false;
    audioRef.current?.pause();

    if (!path) {
      return () => {
        cancelled = true;
      };
    }

    if (!isTauriRuntime()) {
      void Promise.resolve().then(() => {
        if (!cancelled) {
          setPreview({
            path,
            source: null,
            error: "Player disponivel apenas no app desktop."
          });
        }
      });
      return () => {
        cancelled = true;
      };
    }

    void tauriClient
      .prepareAudioPreview(path)
      .then((previewPath) => {
        if (!cancelled) {
          setPreview({
            path,
            source: convertFileSrc(previewPath),
            error: null
          });
        }
      })
      .catch((unknownError: unknown) => {
        if (!cancelled) {
          setPreview({
            path,
            source: null,
            error: errorMessage(unknownError)
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [path]);

  async function toggle(): Promise<void> {
    const audio = audioRef.current;
    if (!audio || !source) {
      return;
    }
    if (audio.paused) {
      try {
        setPlaybackError(null);
        await audio.play();
        setIsPlaying(true);
      } catch (unknownError: unknown) {
        setPlaybackError({ path: path ?? "", message: errorMessage(unknownError) });
      }
    } else {
      audio.pause();
      setIsPlaying(false);
    }
  }

  const isAudioPlaying = Boolean(source && isPlaying);
  const Icon = isAudioPlaying ? Pause : Play;

  return (
    <section className={styles.player}>
      <div className={styles.header}>
        <span>{title}</span>
        <button
          type="button"
          aria-label={isAudioPlaying ? "Pausar audio" : "Tocar audio"}
          disabled={!source}
          onClick={() => {
            void toggle();
          }}
        >
          <Icon size={16} />
        </button>
      </div>
      {source ? (
        <audio
          key={source}
          ref={audioRef}
          src={source}
          controls
          onPause={() => {
            setIsPlaying(false);
          }}
          onEnded={() => {
            setIsPlaying(false);
          }}
          onError={() => {
            setPlaybackError({
              path: path ?? "",
              message: "Nao foi possivel carregar o audio selecionado."
            });
          }}
        />
      ) : path ? (
        <div className={styles.empty}>{error ?? "Preparando audio..."}</div>
      ) : (
        <div className={styles.empty}>Nenhum audio selecionado</div>
      )}
    </section>
  );
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
