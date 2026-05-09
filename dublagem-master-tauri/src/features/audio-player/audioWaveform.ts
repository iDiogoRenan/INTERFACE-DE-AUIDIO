export interface WaveformPeak {
  min: number;
  max: number;
}

export interface WaveformTheme {
  background: string;
  centerLine: string;
  grid: string;
  progress: string;
  waveform: string;
  playhead: string;
  text: string;
}

export interface DrawWaveformOptions {
  canvas: HTMLCanvasElement;
  peaks: WaveformPeak[];
  progress: number;
  durationSeconds: number;
  devicePixelRatio: number;
  theme: WaveformTheme;
}

const secondsPerMinute = 60;
const secondsPerHour = 3600;
const defaultPeak: WaveformPeak = { min: 0, max: 0 };

export function formatClockTime(totalSeconds: number): string {
  if (!Number.isFinite(totalSeconds) || totalSeconds <= 0) {
    return "00:00.000";
  }

  const hours = Math.floor(totalSeconds / secondsPerHour);
  const minutes = Math.floor((totalSeconds % secondsPerHour) / secondsPerMinute);
  const seconds = Math.floor(totalSeconds % secondsPerMinute);
  const milliseconds = Math.floor((totalSeconds % 1) * 1000);
  const minuteText = String(minutes).padStart(2, "0");
  const secondText = String(seconds).padStart(2, "0");
  const millisecondText = String(milliseconds).padStart(3, "0");

  if (hours > 0) {
    return `${String(hours).padStart(2, "0")}:${minuteText}:${secondText}.${millisecondText}`;
  }

  return `${minuteText}:${secondText}.${millisecondText}`;
}

export function buildWaveformPeaks(channelData: Float32Array, targetBars: number): WaveformPeak[] {
  const barCount = Math.max(1, Math.floor(targetBars));
  if (channelData.length === 0) {
    return Array.from({ length: barCount }, () => defaultPeak);
  }

  const samplesPerBar = Math.max(1, Math.floor(channelData.length / barCount));

  return Array.from({ length: barCount }, (_, barIndex) => {
    const start = barIndex * samplesPerBar;
    const end = barIndex === barCount - 1 ? channelData.length : start + samplesPerBar;
    let min = 1;
    let max = -1;

    for (let index = start; index < end; index += 1) {
      const sample = channelData[index] ?? 0;
      min = Math.min(min, sample);
      max = Math.max(max, sample);
    }

    return {
      min: clampAmplitude(min),
      max: clampAmplitude(max)
    };
  });
}

export function drawWaveform({
  canvas,
  peaks,
  progress,
  durationSeconds,
  devicePixelRatio,
  theme
}: DrawWaveformOptions): void {
  const context = canvas.getContext("2d");
  if (!context) {
    return;
  }

  const ratio = Math.max(1, devicePixelRatio);
  const width = Math.max(1, Math.floor(canvas.clientWidth * ratio));
  const height = Math.max(1, Math.floor(canvas.clientHeight * ratio));
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }

  context.clearRect(0, 0, width, height);
  context.fillStyle = theme.background;
  context.fillRect(0, 0, width, height);

  const timelineHeight = 24 * ratio;
  const waveTop = timelineHeight;
  const waveHeight = height - timelineHeight;
  const centerY = waveTop + waveHeight / 2;
  const barGap = Math.max(1, Math.floor(1.5 * ratio));
  const barWidth = Math.max(1, Math.floor(width / Math.max(1, peaks.length)) - barGap);
  const clampedProgress = clampProgress(progress);
  const progressX = width * clampedProgress;

  drawTimeline(context, width, timelineHeight, durationSeconds, ratio, theme);

  context.strokeStyle = theme.centerLine;
  context.lineWidth = 1 * ratio;
  context.beginPath();
  context.moveTo(0, centerY);
  context.lineTo(width, centerY);
  context.stroke();

  peaks.forEach((peak, index) => {
    const x = index * (barWidth + barGap);
    const top = centerY - Math.max(1, peak.max * (waveHeight / 2));
    const bottom = centerY - Math.min(-1, peak.min * (waveHeight / 2));
    const barHeight = Math.max(1, bottom - top);
    context.fillStyle = x <= progressX ? theme.progress : theme.waveform;
    context.fillRect(x, top, barWidth, barHeight);
  });

  context.strokeStyle = theme.playhead;
  context.lineWidth = 2 * ratio;
  context.beginPath();
  context.moveTo(progressX, timelineHeight);
  context.lineTo(progressX, height);
  context.stroke();
}

function drawTimeline(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  durationSeconds: number,
  ratio: number,
  theme: WaveformTheme
): void {
  const majorMarks = 4;
  const minorMarks = 16;

  context.fillStyle = theme.grid;
  for (let marker = 0; marker <= minorMarks; marker += 1) {
    const x = (width / minorMarks) * marker;
    const markerHeight = marker % (minorMarks / majorMarks) === 0 ? height * 0.82 : height * 0.42;
    context.fillRect(x, height - markerHeight, 1 * ratio, markerHeight);
  }

  context.fillStyle = theme.text;
  context.font = `${String(11 * ratio)}px ui-monospace, SFMono-Regular, Consolas, monospace`;
  context.textBaseline = "top";

  for (let marker = 0; marker <= majorMarks; marker += 1) {
    const x = (width / majorMarks) * marker + 6 * ratio;
    const seconds = durationSeconds * (marker / majorMarks);
    context.fillText(formatClockTime(seconds), x, 4 * ratio);
  }
}

function clampAmplitude(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.max(-1, Math.min(1, value));
}

function clampProgress(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.max(0, Math.min(1, value));
}
