import { describe, expect, it } from "vitest";
import { commaBeforeQuestion, palatalizePtBr, synchronizePunctuation } from "./ptBr";

describe("ptBr text transforms", () => {
  it("synchronizes final punctuation from the source", () => {
    expect(synchronizePunctuation("Ola.", "Hello?")).toBe("Ola?");
    expect(synchronizePunctuation("Ola?", "Hello!")).toBe("Ola!");
    expect(synchronizePunctuation("Ola!", "Hello.")).toBe("Ola.");
    expect(synchronizePunctuation("Ola!", "Hello")).toBe("Ola");
    expect(synchronizePunctuation("", "Hello?")).toBe("");
    expect(synchronizePunctuation("Ola", "")).toBe("Ola");
  });

  it("adds comma pacing before question marks", () => {
    expect(commaBeforeQuestion("Tudo bem?")).toBe("Tudo bem, ?");
    expect(commaBeforeQuestion("Tudo bem, ?")).toBe("Tudo bem, ?");
    expect(commaBeforeQuestion("Sem pergunta.")).toBe("Sem pergunta.");
  });

  it("palatalizes pt-br suffixes like the legacy accent fix", () => {
    expect(palatalizePtBr("bati noite pedi mode")).toBe("batchi noitche pedchi modche");
    expect(palatalizePtBr("de te di ti")).toBe("de te di ti");
  });
});
