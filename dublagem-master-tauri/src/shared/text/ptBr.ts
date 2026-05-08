export function synchronizePunctuation(base: string, reference: string): string {
  if (base.trim().length === 0 || reference.trim().length === 0) {
    return base;
  }

  const cleaned = base.trim().replace(/[.?!;:,\s"'"\u201d\u2019]+$/u, "");
  const referenceEnd = reference.trim().replace(/[\s"'"\u201d\u2019]+$/u, "");

  if (referenceEnd.endsWith("?")) {
    return `${cleaned}?`;
  }
  if (referenceEnd.endsWith("!")) {
    return `${cleaned}!`;
  }
  if (referenceEnd.endsWith(".")) {
    return `${cleaned}.`;
  }
  return cleaned;
}

export function commaBeforeQuestion(text: string): string {
  return text.replace(/([^,\s])\s*\?/gu, "$1, ?");
}

export function palatalizePtBr(text: string): string {
  return text.split(/\s+/u).map(palatalizeToken).join(" ");
}

function palatalizeToken(token: string): string {
  const suffixes: readonly (readonly [string, string])[] = [
    ["tis", "tchis"],
    ["tes", "tches"],
    ["ti", "tchi"],
    ["te", "tche"],
    ["dis", "dchis"],
    ["des", "dches"],
    ["di", "dchi"],
    ["de", "dche"]
  ];

  for (const [from, to] of suffixes) {
    if (token.length > from.length && token.endsWith(from)) {
      return `${token.slice(0, -from.length)}${to}`;
    }
  }

  return token;
}
