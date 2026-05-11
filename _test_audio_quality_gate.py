#!/usr/bin/env python3
"""Regressoes do gate automatico de qualidade usado pela dublagem."""

import os
import shutil
import sys
import tempfile
import unittest.mock as mock

os.environ["QT_QPA_PLATFORM"] = "offscreen"
sys.path.insert(0, os.path.dirname(__file__))
ffmpeg_dir = os.path.join(os.path.dirname(__file__), ".venv", "ffmpeg")
os.environ["PATH"] = ffmpeg_dir + os.pathsep + os.environ.get("PATH", "")
sys.stdout = open(sys.stdout.fileno(), mode="w", encoding="utf-8", buffering=1)

import numpy as np
import soundfile as sf

import _patch_accent_fix as paf
from _audio_quality_gate import realizar_tri_checagem


class FakeWhisper:
    def __init__(self, texto: str):
        self.texto = texto

    def transcribe(self, _path: str, **_kwargs: object) -> dict[str, str]:
        return {"text": self.texto}


class SequencedWhisper:
    def transcribe(self, path: str, **_kwargs: object) -> dict[str, str]:
        if "temp_ref_curta" in os.path.basename(path):
            return {"text": "referencia limpa para clonar voz"}
        return {"text": "eca eca eca"}


class FakeOmni:
    def __init__(self) -> None:
        self.generate_kwargs: dict[str, object] | None = None

    def create_voice_clone_prompt(self, ref_audio: str, ref_text: str, preprocess_prompt: bool = True) -> dict[str, str]:
        assert os.path.exists(ref_audio)
        assert ref_text
        assert preprocess_prompt
        return {"prompt": "ok"}

    def generate(self, **kwargs: object) -> list[np.ndarray]:
        self.generate_kwargs = dict(kwargs)
        sr = paf.SAMPLE_RATE_TTS
        t = np.arange(int(0.35 * sr), dtype=np.float32) / sr
        return [0.16 * np.sin(2.0 * np.pi * 220.0 * t)]


def escrever_audio_final(caminho: str) -> None:
    sr = paf.SAMPLE_RATE_TTS
    t = np.arange(int(0.35 * sr), dtype=np.float32) / sr
    voz = 0.16 * np.sin(2.0 * np.pi * 220.0 * t)
    cauda = np.zeros(int(0.08 * sr), dtype=np.float32)
    sf.write(caminho, np.concatenate([voz, cauda]), sr)


def fake_sync(y_val: np.ndarray, caminho_saida: str, _caminho_original: str, _pad_ms: int) -> dict[str, float]:
    sr = paf.SAMPLE_RATE_TTS
    cauda = np.zeros(int(0.08 * sr), dtype=np.float32)
    sf.write(caminho_saida, np.concatenate([y_val.astype(np.float32), cauda]), sr)
    return {
        "duracao_original_ms": 500.0,
        "duracao_final_ms": 500.0,
        "taxa_desejada": 1.0,
        "taxa_aplicada": 1.0,
    }


def assert_tri(texto: str, esperado: bool, motivo_parcial: str) -> None:
    valido, motivo = realizar_tri_checagem(texto)
    assert valido is esperado, f"{texto!r}: esperado {esperado}, recebido {valido}"
    assert motivo_parcial in motivo, f"{texto!r}: motivo inesperado {motivo!r}"


assert_tri("", False, "Nível 1: Vazio/Silêncio")
assert_tri("Thank you for watching", False, "Nível 1: Alucinação detectada")
assert_tri("[music]", False, "Nível 2: Apenas ruídos/tags")
assert_tri("eca eca eca", False, "Nível 3: Loop/Glitch de repetição")
assert_tri("eca", False, "Nível 3: Apenas expressões")
assert_tri("eu preciso sair agora", True, "FALA REAL DETECTADA")
print("1. [OK] tri-checagem replica o fluxo textual do SEPRAR_AUDIOS")

ok_original, motivo_original = paf.verificar_qualidade_fala_original(
    {
        "text": "eca eca eca",
        "segments": [{"no_speech_prob": 0.0, "avg_logprob": 0.0}],
    }
)
assert not ok_original
assert "tri-checagem SEPRAR_AUDIOS reprovou" in motivo_original
print("2. [OK] dublagem rejeita original antes de sintetizar quando a tri-checagem reprova")

tmpdir = tempfile.mkdtemp()
try:
    audio_path = os.path.join(tmpdir, "audio_final.wav")
    escrever_audio_final(audio_path)
    with mock.patch.object(paf, "validar_qualidade_audio_natural", return_value=(True, "OK")), \
         mock.patch.object(paf, "calcular_similaridade_texto", return_value=1.0), \
         mock.patch.object(paf, "calcular_cobertura_palavras", return_value=(1.0, True, ["texto"], ["texto"])):
        ok_final, motivo_final = paf.validar_audio_final_completo(
            audio_path,
            "texto esperado",
            FakeWhisper("eca eca eca"),
            "pt",
        )

    assert not ok_final
    assert "Tri-checagem SEPRAR_AUDIOS reprovou" in motivo_final
    print("3. [OK] audio final nao e aprovado quando a transcricao cai no fluxo de rejeicao")
finally:
    shutil.rmtree(tmpdir, ignore_errors=True)

tmpdir = tempfile.mkdtemp()
try:
    origem = os.path.join(tmpdir, "origem.wav")
    escrever_audio_final(origem)
    fake_omni = FakeOmni()
    worker = paf.SingleDubbingWorkerV14(
        paths_en=[origem],
        pasta_guia="",
        models_ref={"whisper": SequencedWhisper(), "omni": fake_omni},
        custom_texts={"en": "clean source text", "pt": "texto esperado"},
        omni_temp=0.0,
        pad_ms=0,
        target_lang="pt",
        source_lang="en",
    )
    eventos: list[tuple[bool, str, str, str, str, str]] = []
    worker.file_done_signal.connect(lambda *args: eventos.append(args))

    with mock.patch.object(paf, "validar_qualidade_audio_natural", return_value=(True, "OK")), \
         mock.patch.object(paf, "calcular_similaridade_texto", return_value=1.0), \
         mock.patch.object(paf, "calcular_cobertura_palavras", return_value=(1.0, True, ["texto"], ["texto"])), \
         mock.patch.object(paf, "sincronizar_master_v10_1", side_effect=fake_sync):
        worker.run()

    assert fake_omni.generate_kwargs is not None
    assert eventos
    ok_worker, motivo_worker, path_out, nome_original, _txt_en, _txt_pt = eventos[-1]
    assert not ok_worker
    assert not path_out
    assert os.path.basename(origem) == nome_original
    assert "Tri-checagem SEPRAR_AUDIOS reprovou" in motivo_worker, motivo_worker
    print("4. [OK] worker nao emite sucesso quando o QC final reprova")
finally:
    shutil.rmtree(tmpdir, ignore_errors=True)

print("=== TESTE DO GATE DE QUALIDADE PASSOU ===")
