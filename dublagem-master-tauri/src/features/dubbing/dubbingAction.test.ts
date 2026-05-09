import { describe, expect, it } from "vitest";
import { dubbingActionCopy } from "./dubbingAction";

describe("dubbingActionCopy", () => {
  it("keeps the first-pass dubbing label for files without rendered output", () => {
    expect(dubbingActionCopy(null)).toMatchObject({
      intent: "dub",
      idleLabel: "Dublar selecionado",
      busyLabel: "Dublando"
    });
    expect(dubbingActionCopy("pending")).toMatchObject({
      intent: "dub",
      idleLabel: "Dublar selecionado",
      busyLabel: "Dublando"
    });
  });

  it("uses redubbing copy when the selected file has a reviewable output", () => {
    expect(dubbingActionCopy("dubbed")).toMatchObject({
      intent: "redub",
      idleLabel: "Redublar selecionado",
      busyLabel: "Redublando"
    });
    expect(dubbingActionCopy("rejected")).toMatchObject({
      intent: "redub",
      idleLabel: "Redublar selecionado",
      busyLabel: "Redublando"
    });
  });
});
