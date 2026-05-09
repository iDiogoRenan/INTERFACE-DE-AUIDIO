import { describe, expect, it } from "vitest";
import { buildWaveformPeaks, formatClockTime } from "./audioWaveform";

describe("formatClockTime", () => {
  it("formats sub-minute and long running audio clocks", () => {
    expect(formatClockTime(8.448)).toBe("00:08.448");
    expect(formatClockTime(65.2)).toBe("01:05.200");
    expect(formatClockTime(3723.009)).toBe("01:02:03.009");
  });

  it("normalizes invalid or negative durations", () => {
    expect(formatClockTime(Number.NaN)).toBe("00:00.000");
    expect(formatClockTime(-4)).toBe("00:00.000");
  });
});

describe("buildWaveformPeaks", () => {
  it("builds bounded min and max peaks for each visual bar", () => {
    const peaks = buildWaveformPeaks(new Float32Array([-2, -0.4, 0.25, 1.8]), 2);

    expect(peaks[0]?.min).toBe(-1);
    expect(peaks[0]?.max).toBeCloseTo(-0.4);
    expect(peaks[1]?.min).toBeCloseTo(0.25);
    expect(peaks[1]?.max).toBe(1);
  });

  it("returns silent bars for empty audio data", () => {
    expect(buildWaveformPeaks(new Float32Array(), 3)).toEqual([
      { min: 0, max: 0 },
      { min: 0, max: 0 },
      { min: 0, max: 0 }
    ]);
  });
});
