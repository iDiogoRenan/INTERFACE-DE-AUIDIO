"""Gate textual de qualidade para separar fala real de ruido/transcricao espuria."""

import re
from typing import Final


PADROES_EXPRESSOES: Final[tuple[str, ...]] = (
    r"^a+h+$",
    r"^o+h+$",
    r"^u+h+$",
    r"^h+m+$",
    r"^u+m+$",
    r"^a+w+$",
    r"^e+r+$",
    r"^s+h+$",
    r"^m+$",
    r"^g+r+$",
    r"^a+f+$",
    r"^u+f+a+$",
    r"^v+i+x+e+$",
    r"^x+i+$",
    r"^p+s+i+u+$",
    r"^t+s+c+$",
    r"^e+p+a+$",
    r"^o+p+s+$",
    r"^(h+a+)+$",
    r"^(h+e+)+$",
    r"^(h+i+)+$",
    r"^(h+o+)+$",
    r"^w+o+w+$",
    r"^y+a+y+$",
    r"^o+u+c+h+$",
    r"^o+w+$",
    r"^p+h+e+w+$",
    r"^d+u+h+$",
    r"^g+e+e+$",
    r"^b+a+$",
    r"^b+o+$",
    r"^e+h+$",
    r"^p+f+$",
    r"^p+u+f+$",
    r"^a+i+$",
    r"^u+i+$",
    r"^e+i+$",
    r"^e+c+a+$",
)

OUTROS_RUIDOS: Final[frozenset[str]] = frozenset(
    {"gasp", "sigh", "argh", "yikes", "ahem", "mhm", "uhhuh", "ugh", "eca"}
)

VOGAIS_ISOLADAS_VALIDAS: Final[frozenset[str]] = frozenset(
    {"a", "e", "o", "é", "ó", "á", "í", "ú", "à", "i"}
)

ALUCINACOES_WHISPER: Final[tuple[str, ...]] = (
    "thank you for watching",
    "subscribing",
    "thanks for watching",
    "watching",
    "obrigado por assistir",
    "obrigada por assistir",
    "inscreva-se",
    "legendado por",
)


def eh_ruido(palavra: str) -> bool:
    """Verifica se a palavra e apenas ruido ou expressao sem conteudo semantico."""
    palavra = palavra.lower().strip()
    palavra = re.sub(r"[^\w]", "", palavra)

    if not palavra:
        return True

    if len(palavra) == 1:
        if palavra in VOGAIS_ISOLADAS_VALIDAS:
            return False
        return True

    if len(set(palavra)) == 1:
        return True

    if palavra in OUTROS_RUIDOS:
        return True

    for padrao in PADROES_EXPRESSOES:
        if re.fullmatch(padrao, palavra):
            return True

    return False


def realizar_tri_checagem(texto: str) -> tuple[bool, str]:
    if not texto:
        return False, "Nível 1: Vazio/Silêncio"

    texto_lower = texto.lower()

    if any(alucinacao in texto_lower for alucinacao in ALUCINACOES_WHISPER):
        return False, "Nível 1: Alucinação detectada"

    texto_limpo = re.sub(r"\[.*?\]|\(.*?\)|\*.*?\*|<.*?>|♪|♫", " ", texto_lower)
    texto_limpo = re.sub(r"[^\w\s]", " ", texto_limpo).strip()

    if not texto_limpo:
        return False, "Nível 2: Apenas ruídos/tags"

    palavras = texto_limpo.split()

    if not palavras:
        return False, "Nível 2: Sem palavras após limpeza"

    if len(palavras) >= 3 and len(set(palavras)) == 1:
        return False, f"Nível 3: Loop/Glitch de repetição ('{palavras[0]}')"

    palavras_reais = [palavra for palavra in palavras if not eh_ruido(palavra)]

    if len(palavras_reais) == 0:
        return False, f"Nível 3: Apenas expressões ({' '.join(palavras)})"

    return True, "✅ FALA REAL DETECTADA"
