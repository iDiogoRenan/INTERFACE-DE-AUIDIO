import { describe, expect, it } from "vitest";
import { audioFamilyFromFilename } from "./families";

describe("audioFamilyFromFilename", () => {
  it("preserves the legacy family extraction behavior", () => {
    expect(audioFamilyFromFilename("_ancientstonegolem_9000_boss_00_00001.wav")).toBe(
      "ancientstonegolem"
    );
    expect(audioFamilyFromFilename("dragon_common_dragon_boss_9000_narration_00005.wav")).toBe(
      "dragon_common_dragon_boss"
    );
    expect(audioFamilyFromFilename("ndw_adult_1_questdialog_hello_00664.wav")).toBe("ndw_adult_1");
    expect(audioFamilyFromFilename("unique_kliff_0090_0120_player_00000.wav")).toBe("unique_kliff");
  });

  it("falls back to outros when no semantic family is present", () => {
    expect(audioFamilyFromFilename("00001.wav")).toBe("outros");
    expect(audioFamilyFromFilename("___.wav")).toBe("outros");
  });

  it("keeps names without sequence markers intact", () => {
    expect(audioFamilyFromFilename("ambient_loop.flac")).toBe("ambient_loop");
  });
});
