#!/usr/bin/env python3
"""Regressao: arquivos longos nunca podem virar referencia longa no OmniVoice."""
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


def escrever_wav_longo(caminho: str, duracao_s: float = 157.0, sr: int = paf.SAMPLE_RATE_TTS) -> None:
    total = int(duracao_s * sr)
    audio = np.zeros(total, dtype=np.float32)
    inicio = int(2.0 * sr)
    fim = min(total, inicio + int(30.0 * sr))
    t = np.arange(fim - inicio, dtype=np.float32) / sr
    audio[inicio:fim] = 0.18 * np.sin(2.0 * np.pi * 220.0 * t)
    sf.write(caminho, audio, sr)


class FakeWhisper:
    def transcribe(self, _path: str, **_kwargs):
        return {"text": "Reference words for a short clean sample."}


class FakeOmni:
    def __init__(self):
        self.prompt_ref_duration = 0.0
        self.generate_kwargs = None

    def create_voice_clone_prompt(self, ref_audio: str, ref_text: str, preprocess_prompt: bool = True):
        data, sr = sf.read(ref_audio)
        self.prompt_ref_duration = len(data) / sr
        assert ref_text
        assert preprocess_prompt
        return {"prompt": "ok"}

    def generate(self, **kwargs):
        self.generate_kwargs = kwargs
        return [np.sin(np.linspace(0.0, 24.0 * np.pi, paf.SAMPLE_RATE_TTS, dtype=np.float32)) * 0.1]


def fake_sync(y_val, caminho_saida: str, _caminho_original: str, _pad_ms: int):
    sf.write(caminho_saida, y_val, paf.SAMPLE_RATE_TTS)
    return {
        "duracao_original_ms": 1000,
        "duracao_final_ms": 1000,
        "taxa_desejada": 1.0,
        "taxa_aplicada": 1.0,
    }


tmpdir = tempfile.mkdtemp()
try:
    origem = os.path.join(tmpdir, "audio_longo.wav")
    ref_saida = os.path.join(tmpdir, "referencia_curta.wav")
    escrever_wav_longo(origem)

    referencia = paf.preparar_referencia_curta(origem, ref_saida)
    assert referencia.source_duration_seconds > 156.0
    assert referencia.duration_seconds <= paf.REFERENCE_MAX_SECONDS
    assert 7.9 <= referencia.duration_seconds <= 8.1
    print("1. [OK] referencia extraida de arquivo longo fica <= 10s")

    fake_omni = FakeOmni()
    worker = paf.SingleDubbingWorkerV14(
        paths_en=[origem],
        pasta_guia="",
        models_ref={"whisper": FakeWhisper(), "omni": fake_omni},
        custom_texts={"en": "Original text.", "pt": "Texto dublado."},
        omni_temp=0.0,
        pad_ms=0,
        target_lang="pt",
        source_lang="en",
    )

    with mock.patch.object(paf, "validar_qualidade_audio_natural", return_value=(True, "OK")), \
         mock.patch.object(paf, "calcular_similaridade_texto", return_value=1.0), \
         mock.patch.object(paf, "calcular_cobertura_palavras", return_value=(1.0, [], ["texto"], [])), \
         mock.patch.object(paf, "sincronizar_master_v10_1", side_effect=fake_sync), \
         mock.patch.object(paf, "validar_audio_final_completo", return_value=(True, "OK")):
        worker.run()

    assert fake_omni.prompt_ref_duration <= paf.REFERENCE_MAX_SECONDS
    assert fake_omni.generate_kwargs is not None
    assert "voice_clone_prompt" in fake_omni.generate_kwargs
    assert "ref_audio" not in fake_omni.generate_kwargs
    assert "ref_text" not in fake_omni.generate_kwargs
    print("2. [OK] generate usa voice_clone_prompt e nao recebe ref_audio/ref_text longo")
    print("=== TESTE DE REFERENCIA CURTA PASSOU ===")
finally:
    shutil.rmtree(tmpdir, ignore_errors=True)
