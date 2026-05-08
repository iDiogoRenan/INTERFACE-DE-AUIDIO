from dataclasses import dataclass
from typing import Sequence

from PyQt6.QtWidgets import QComboBox


@dataclass(frozen=True, slots=True)
class LanguageOption:
    code: str
    label: str
    badge: str


LANGUAGE_OPTIONS: dict[str, LanguageOption] = {
    "auto": LanguageOption("auto", "Detectar", "AUTO"),
    "en": LanguageOption("en", "Inglês", "EN"),
    "pt": LanguageOption("pt", "Português", "PT-BR"),
    "fr": LanguageOption("fr", "Francês", "FR"),
    "sv": LanguageOption("sv", "Sueco", "SV"),
}

SOURCE_LANGUAGE_CODES: tuple[str, ...] = ("auto", "en", "fr", "sv", "pt")
TARGET_LANGUAGE_CODES: tuple[str, ...] = ("pt", "fr", "sv", "en")

LANGUAGE_COMBO_STYLE = (
    "QComboBox { background:#161b22; color:#c9d1d9; "
    "border:1px solid #30363d; padding:4px; border-radius:4px; }"
)


def language_badge(code: str) -> str:
    option = LANGUAGE_OPTIONS.get(code, LANGUAGE_OPTIONS["auto"])
    return option.badge


def language_display(code: str) -> str:
    option = LANGUAGE_OPTIONS.get(code, LANGUAGE_OPTIONS["auto"])
    return f"{option.badge}  {option.label}"


def configure_language_combo(
    combo: QComboBox,
    codes: Sequence[str],
    default_code: str | None = None,
) -> None:
    combo.clear()
    for code in codes:
        combo.addItem(language_display(code), code)
    combo.setStyleSheet(LANGUAGE_COMBO_STYLE)
    if default_code is not None:
        index = combo.findData(default_code)
        if index >= 0:
            combo.setCurrentIndex(index)


def current_language_code(combo: QComboBox, fallback: str) -> str:
    data = combo.currentData()
    if isinstance(data, str) and data in LANGUAGE_OPTIONS:
        return data
    return fallback
