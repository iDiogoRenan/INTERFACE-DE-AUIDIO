import { describe, expect, it } from "vitest";
import {
  insertTagIntoLine,
  nativeTags,
  normalizeNativeSynthesisSettings,
  removeNativeTagsFromText,
  replaceLine,
  splitLines,
  tagsByLine,
  tagsInText,
  textSegments,
  unknownNativeTags
} from "./nativeControls";
import { defaultOptions, type NativeSynthesisSettings } from "../tauri/types";

describe("native OmniVoice synthesis settings", () => {
  it("normalizes UI values to the backend accepted ranges", () => {
    const settings: NativeSynthesisSettings = {
      ...defaultOptions.nativeSynthesis,
      voiceMode: "design",
      instruct: "   ",
      speed: Number.NaN,
      durationSeconds: 99,
      numStep: 2,
      guidanceScale: 99,
      positionTemperature: -2,
      classTemperature: Number.NaN,
      loudnessMatchStrength: 99,
      outputGainDb: -99,
      sibilanceReduction: Number.NaN,
      artifactReduction: 2
    };

    expect(normalizeNativeSynthesisSettings(settings)).toMatchObject({
      voiceMode: "design",
      instruct: "female, young adult, moderate pitch",
      speed: null,
      durationSeconds: 60,
      numStep: 8,
      guidanceScale: 10,
      positionTemperature: 0,
      classTemperature: 0,
      loudnessMatchStrength: 1,
      outputGainDb: -12,
      sibilanceReduction: 0,
      artifactReduction: 1
    });
  });
});

describe("native OmniVoice controls", () => {
  it("contains only the official native non-verbal tags", () => {
    expect(nativeTags).toEqual([
      "[laughter]",
      "[sigh]",
      "[confirmation-en]",
      "[question-en]",
      "[question-ah]",
      "[question-oh]",
      "[question-ei]",
      "[question-yi]",
      "[surprise-ah]",
      "[surprise-oh]",
      "[surprise-wa]",
      "[surprise-yo]",
      "[dissatisfaction-hnn]"
    ]);
  });

  it("renders supported tags as distinct text segments", () => {
    expect(textSegments("[sigh] Ola [surprise-oh]!")).toEqual([
      { kind: "tag", value: "[sigh]" },
      { kind: "text", value: " Ola " },
      { kind: "tag", value: "[surprise-oh]" },
      { kind: "text", value: "!" }
    ]);
  });

  it("rejects unknown lowercase bracket tags while allowing pronunciation hints", () => {
    expect(unknownNativeTags("Texto [angry] invalido [B EY1 S].")).toEqual(["[angry]"]);
  });

  it("updates line-oriented editor content without touching adjacent lines", () => {
    const text = "linha um\nlinha dois";

    expect(replaceLine(text, 1, "linha dois [sigh]")).toBe("linha um\nlinha dois [sigh]");
    expect(insertTagIntoLine(text, 0, "[laughter]")).toBe("linha um [laughter] \nlinha dois");
    expect(splitLines("")).toEqual([""]);
  });

  it("extracts native tags from text in order", () => {
    expect(tagsInText("[sigh] Ola [question-ah]?")).toEqual(["[sigh]", "[question-ah]"]);
  });

  it("can lift official native tags out of spoken text", () => {
    expect(removeNativeTagsFromText("[sigh] Ola [question-ah]?\n[B EY1 S] [surprise-oh]!")).toBe(
      "Ola?\n[B EY1 S]!"
    );
    expect(removeNativeTagsFromText("Precisa manter espaco ")).toBe("Precisa manter espaco ");
    expect(tagsByLine("[sigh] Ola\nOpa [surprise-oh]!")).toEqual([["[sigh]"], ["[surprise-oh]"]]);
  });
});
