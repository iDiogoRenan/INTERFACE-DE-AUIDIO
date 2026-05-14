import { convertFileSrc } from "@tauri-apps/api/core";
import { Pause, Play, RotateCcw, ScanLine } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { isTauriRuntime, tauriClient } from "../../shared/tauri/client";
import {
  buildWaveformPeaks,
  drawWaveform,
  formatClockTime,
  type WaveformPeak,
  type WaveformTheme
} from "./audioWaveform";
import styles from "./AudioPlayer.module.css";

interface AudioPlayerProps {
  title: string;
  path: string | null;
  revision?: number;
  playbackRequest?: AudioPlaybackRequest | null;
}

export interface AudioPlaybackRequest {
  id: number;
  path: string;
  startSeconds: number;
  endSeconds: number | null;
  autoplay: boolean;
}

interface PreviewState {
  path: string;
  revision: number;
  source: string | null;
  error: string | null;
}

interface WaveformState {
  source: string;
  peaks: WaveformPeak[];
  durationSeconds: number;
  error: string | null;
}

interface PlaybackError {
  path: string;
  message: string;
}

const waveformBars = 240;
const waveformTheme: WaveformTheme = {
  background: "#16202a",
  centerLine: "#344453",
  grid: "#516273",
  progress: "#e35d52",
  waveform: "#61b7ef",
  playhead: "#f0b64a",
  text: "#c6d0d9"
};

function createAudioContextDisposer(audioContext: AudioContext): () => void {
  let closePromise: Promise<void> | null = null;

  return () => {
    if (closePromise !== null) {
      return;
    }

    closePromise = releaseAudioContext(audioContext);
    void closePromise.catch(reportAudioContextReleaseFailure);
  };
}

async function releaseAudioContext(audioContext: AudioContext): Promise<void> {
  if (audioContext.state === "closed") {
    return;
  }

  try {
    await audioContext.close();
  } catch (unknownError: unknown) {
    if (!isInvalidStateError(unknownError)) {
      throw unknownError;
    }
  }
}

function isInvalidStateError(unknownError: unknown): boolean {
  return (
    (unknownError instanceof DOMException || unknownError instanceof Error) &&
    unknownError.name === "InvalidStateError"
  );
}

function reportAudioContextReleaseFailure(unknownError: unknown): void {
  console.error("Falha ao liberar recursos do analisador de audio.", unknownError);
}

export function AudioPlayer({
  title,
  path,
  revision = 0,
  playbackRequest = null
}: AudioPlayerProps) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const handledPlaybackRequestIdRef = useRef<number | null>(null);
  const completedPlaybackRequestIdRef = useRef<number | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [nativeDuration, setNativeDuration] = useState(0);
  const [preview, setPreview] = useState<PreviewState | null>(null);
  const [waveform, setWaveform] = useState<WaveformState | null>(null);
  const [playbackError, setPlaybackError] = useState<PlaybackError | null>(null);
  const currentPreview = preview?.path === path && preview.revision === revision ? preview : null;
  const source = currentPreview?.source ?? null;
  const currentWaveform = waveform?.source === source ? waveform : null;
  const decodedDuration = currentWaveform?.durationSeconds ?? 0;
  const durationSeconds = Math.max(decodedDuration, nativeDuration);
  const progress = durationSeconds > 0 ? currentTime / durationSeconds : 0;
  const error =
    playbackError?.path === path
      ? playbackError.message
      : (currentPreview?.error ?? currentWaveform?.error ?? null);

  const statusLabel = useMemo(() => {
    if (!path) {
      return "Sem arquivo";
    }
    if (!source) {
      return error ?? "Preparando prévia";
    }
    if (!currentWaveform) {
      return "Analisando forma de onda";
    }
    if (currentWaveform.error) {
      return "Prévia carregada";
    }
    return "Forma de onda pronta";
  }, [currentWaveform, error, path, source]);

  const renderWaveform = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }

    drawWaveform({
      canvas,
      peaks: currentWaveform?.peaks ?? [],
      progress,
      durationSeconds,
      devicePixelRatio: window.devicePixelRatio,
      theme: waveformTheme
    });
  }, [currentWaveform, durationSeconds, progress]);

  useEffect(() => {
    let cancelled = false;
    audioRef.current?.pause();

    void Promise.resolve().then(() => {
      if (cancelled) {
        return;
      }

      setCurrentTime(0);
      setNativeDuration(0);
      setIsPlaying(false);
      setPlaybackError(null);
      setWaveform(null);
    });

    if (!path) {
      void Promise.resolve().then(() => {
        if (!cancelled) {
          setPreview(null);
        }
      });
      return () => {
        cancelled = true;
      };
    }

    if (!isTauriRuntime()) {
      void Promise.resolve().then(() => {
        if (!cancelled) {
          setPreview({
            path,
            revision,
            source: null,
            error: "Reprodutor disponível apenas no aplicativo."
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
            revision,
            source: withRevision(convertFileSrc(previewPath), revision),
            error: null
          });
        }
      })
      .catch((unknownError: unknown) => {
        if (!cancelled) {
          setPreview({
            path,
            revision,
            source: null,
            error: errorMessage(unknownError)
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [path, revision]);

  useEffect(() => {
    if (!source) {
      return;
    }

    let cancelled = false;
    const abortController = new AbortController();
    const audioContext = new AudioContext();
    const disposeAudioContext = createAudioContextDisposer(audioContext);

    void fetch(source, { signal: abortController.signal })
      .then((response) => {
        if (!response.ok) {
          throw new Error(`Falha ao carregar prévia (${String(response.status)})`);
        }
        return response.arrayBuffer();
      })
      .then((buffer) => audioContext.decodeAudioData(buffer))
      .then((audioBuffer) => {
        if (cancelled) {
          return;
        }

        setWaveform({
          source,
          peaks: buildWaveformPeaks(audioBuffer.getChannelData(0), waveformBars),
          durationSeconds: audioBuffer.duration,
          error: null
        });
      })
      .catch((unknownError: unknown) => {
        if (!cancelled) {
          setWaveform({
            source,
            peaks: [],
            durationSeconds: 0,
            error: errorMessage(unknownError)
          });
        }
      })
      .finally(() => {
        disposeAudioContext();
      });

    return () => {
      cancelled = true;
      abortController.abort();
      disposeAudioContext();
    };
  }, [source]);

  useEffect(() => {
    renderWaveform();
  }, [renderWaveform]);

  useEffect(() => {
    const audio = audioRef.current;
    const request = playbackRequest;
    if (
      !audio ||
      !source ||
      request?.path !== path ||
      handledPlaybackRequestIdRef.current === request.id
    ) {
      return;
    }

    handledPlaybackRequestIdRef.current = request.id;
    completedPlaybackRequestIdRef.current = null;
    const boundedStart = clampTime(request.startSeconds, normalizeDuration(audio.duration));
    audio.currentTime = boundedStart;
    setCurrentTime(boundedStart);

    if (!request.autoplay) {
      return;
    }

    setPlaybackError(null);
    void audio
      .play()
      .then(() => {
        setIsPlaying(true);
      })
      .catch((unknownError: unknown) => {
        setPlaybackError({ path, message: errorMessage(unknownError) });
      });
  }, [path, playbackRequest, source]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }

    const observer = new ResizeObserver(() => {
      renderWaveform();
    });
    observer.observe(canvas);

    return () => {
      observer.disconnect();
    };
  }, [renderWaveform]);

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

  function restart(): void {
    const audio = audioRef.current;
    if (!audio) {
      return;
    }

    audio.currentTime = 0;
    setCurrentTime(0);
  }

  function seek(clientX: number): void {
    const audio = audioRef.current;
    const canvas = canvasRef.current;
    if (!audio || !canvas || durationSeconds <= 0) {
      return;
    }

    const rect = canvas.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    const nextTime = durationSeconds * ratio;
    audio.currentTime = nextTime;
    setCurrentTime(nextTime);
  }

  const isAudioPlaying = Boolean(source && isPlaying);
  const Icon = isAudioPlaying ? Pause : Play;

  return (
    <section className={styles.player}>
      <div className={styles.header}>
        <div className={styles.titleBlock}>
          <span>{title}</span>
          <strong>{statusLabel}</strong>
        </div>
        <div className={styles.transport}>
          <button
            type="button"
            aria-label={isAudioPlaying ? "Pausar áudio" : "Tocar áudio"}
            disabled={!source}
            onClick={() => {
              void toggle();
            }}
          >
            <Icon size={16} />
          </button>
          <button type="button" aria-label="Voltar ao início" disabled={!source} onClick={restart}>
            <RotateCcw size={15} />
          </button>
        </div>
      </div>

      <div className={styles.waveformShell}>
        <canvas
          ref={canvasRef}
          className={styles.waveform}
          role="slider"
          aria-label={`Linha do tempo do áudio ${title}`}
          aria-valuemin={0}
          aria-valuemax={Math.round(durationSeconds * 1000)}
          aria-valuenow={Math.round(currentTime * 1000)}
          tabIndex={source ? 0 : -1}
          onClick={(event) => {
            seek(event.clientX);
          }}
          onKeyDown={(event) => {
            handleSeekKey(event, durationSeconds, currentTime, (nextTime) => {
              const audio = audioRef.current;
              if (!audio) {
                return;
              }
              audio.currentTime = nextTime;
              setCurrentTime(nextTime);
            });
          }}
        />
        {!source ? <div className={styles.empty}>{error ?? "Nenhum áudio selecionado"}</div> : null}
      </div>

      <div className={styles.footer}>
        <span className={styles.timeReadout}>
          {formatClockTime(currentTime)} / {formatClockTime(durationSeconds)}
        </span>
        <span className={styles.formatBadge}>
          <ScanLine size={14} />
          {currentWaveform?.peaks.length
            ? `${String(currentWaveform.peaks.length)} picos`
            : "sem picos"}
        </span>
      </div>

      {source ? (
        <audio
          key={source}
          ref={audioRef}
          src={source}
          preload="metadata"
          onLoadedMetadata={(event) => {
            setNativeDuration(normalizeDuration(event.currentTarget.duration));
          }}
          onDurationChange={(event) => {
            setNativeDuration(normalizeDuration(event.currentTarget.duration));
          }}
          onTimeUpdate={(event) => {
            const nextTime = event.currentTarget.currentTime;
            const request = playbackRequest;
            if (
              request !== null &&
              request.endSeconds !== null &&
              nextTime >= request.endSeconds &&
              completedPlaybackRequestIdRef.current !== request.id
            ) {
              completedPlaybackRequestIdRef.current = request.id;
              const boundedEnd = clampTime(request.endSeconds, durationSeconds);
              event.currentTarget.currentTime = boundedEnd;
              event.currentTarget.pause();
              setCurrentTime(boundedEnd);
              setIsPlaying(false);
              return;
            }

            setCurrentTime(nextTime);
          }}
          onPause={() => {
            setIsPlaying(false);
          }}
          onPlay={() => {
            setIsPlaying(true);
          }}
          onEnded={() => {
            setIsPlaying(false);
          }}
          onError={() => {
            setPlaybackError({
              path: path ?? "",
              message: "Não foi possível carregar o áudio selecionado."
            });
          }}
        />
      ) : null}
    </section>
  );
}

function handleSeekKey(
  event: React.KeyboardEvent<HTMLCanvasElement>,
  durationSeconds: number,
  currentTime: number,
  onSeek: (timeSeconds: number) => void
): void {
  if (durationSeconds <= 0) {
    return;
  }

  const stepSeconds = event.shiftKey ? 5 : 1;
  const keyOffset = seekKeyOffset(event.key, stepSeconds);
  if (keyOffset === null) {
    return;
  }

  event.preventDefault();
  onSeek(Math.max(0, Math.min(durationSeconds, currentTime + keyOffset)));
}

function seekKeyOffset(key: string, stepSeconds: number): number | null {
  if (key === "ArrowLeft") {
    return -stepSeconds;
  }
  if (key === "ArrowRight") {
    return stepSeconds;
  }
  if (key === "Home") {
    return Number.NEGATIVE_INFINITY;
  }
  if (key === "End") {
    return Number.POSITIVE_INFINITY;
  }
  return null;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function normalizeDuration(durationSeconds: number): number {
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) {
    return 0;
  }

  return durationSeconds;
}

function clampTime(timeSeconds: number, durationSeconds: number): number {
  if (!Number.isFinite(timeSeconds) || timeSeconds <= 0) {
    return 0;
  }

  if (durationSeconds <= 0) {
    return timeSeconds;
  }

  return Math.min(timeSeconds, durationSeconds);
}

function withRevision(source: string, revision: number): string {
  const separator = source.includes("?") ? "&" : "?";
  return `${source}${separator}audioRevision=${encodeURIComponent(String(revision))}`;
}
