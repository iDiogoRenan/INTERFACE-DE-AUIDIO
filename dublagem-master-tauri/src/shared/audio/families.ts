const familyMarkerTokens = new Set(["questdialog", "narration", "player"]);

export function audioFamilyFromFilename(filename: string): string {
  const stem = filename
    .replace(/\.[^.]+$/, "")
    .toLowerCase()
    .replace(/^_+|_+$/g, "");
  const tokens = stem.split("_").filter(Boolean);

  if (tokens.length === 0) {
    return "outros";
  }

  const markerIndex = tokens.findIndex((token) => familyMarkerTokens.has(token));
  const sequenceIndex = tokens.findIndex(isSequenceToken);
  const takeUntil =
    markerIndex >= 0 ? markerIndex : sequenceIndex >= 0 ? sequenceIndex : tokens.length;
  const familyTokens = tokens.slice(0, takeUntil);

  while (familyTokens.length > 0 && isSequenceToken(familyTokens[familyTokens.length - 1] ?? "")) {
    familyTokens.pop();
  }

  return familyTokens.length > 0 ? familyTokens.join("_") : "outros";
}

function isSequenceToken(token: string): boolean {
  return /^\d+$/.test(token) && (token.length > 1 || token.startsWith("0"));
}
