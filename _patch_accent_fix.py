# _patch_accent_fix.py — v5.1 COMPATÍVEL COM ACCENT_FIX1
# Core INTOCADO do omni11. Adições: palatização (checkbox) + vírgula (checkbox)

import os, re, gc, time, traceback, difflib, unicodedata
from dataclasses import dataclass
from typing import Callable, Optional
import numpy as np
import soundfile as sf
import librosa
from pydub import AudioSegment
from PyQt6.QtCore import QThread, pyqtSignal

from _audio_quality_gate import realizar_tri_checagem

# ─── FUNÇÕES AUXILIARES ──────────────────────────────────────────────────────

VOICE_PROFILES_PTBR = {
    "male_adult":   {"instruct": "male, young adult, moderate pitch",  "ref_text": "Olá, estou pronto para falar com você hoje."},
    "female_adult": {"instruct": "female, young adult, moderate pitch", "ref_text": "Olá, estou pronta para falar com você."},
    "male_old":     {"instruct": "male, elderly, low pitch",           "ref_text": "Há muitos anos aprendi sobre essas coisas."},
    "female_old":   {"instruct": "female, elderly, moderate pitch",    "ref_text": "Há muito tempo aprendi que a paciência é uma virtude."},
    "male_child":   {"instruct": "male, child, high pitch",            "ref_text": "Ei, vamos brincar juntos hoje!"},
    "female_child": {"instruct": "female, child, high pitch",          "ref_text": "Que dia lindo para uma aventura nova!"},
}

SAMPLE_RATE_TTS = 24000
OMNIVOICE_MAX_SYNTHESIS_SECONDS = 30.0
REFERENCE_TARGET_SECONDS = 8.0
REFERENCE_MAX_SECONDS = 10.0
REFERENCE_MIN_SECONDS = 3.0


@dataclass(frozen=True)
class ShortReference:
    path: str
    duration_seconds: float
    source_duration_seconds: float
    start_seconds: float


def _dtype_label(dtype_obj) -> str:
    return str(dtype_obj).replace("torch.", "")


def descrever_modelo_omnivoice(model) -> tuple[str, str]:
    try:
        param = next(model.parameters())
        return str(param.device), _dtype_label(param.dtype)
    except Exception:
        return "modelo existente", "dtype desconhecido"


def carregar_omnivoice_otimizado(OmniVoice, torch_module, device: str) -> tuple[object, str, str]:
    device_map = "cuda:0" if device == "cuda" else device
    dtype = torch_module.float16 if device == "cuda" else torch_module.float32
    try:
        model = OmniVoice.from_pretrained(
            "k2-fsa/OmniVoice",
            device_map=device_map,
            dtype=dtype,
        )
    except TypeError:
        model = OmniVoice.from_pretrained("k2-fsa/OmniVoice")
        model.to(device)
    return model, device_map, _dtype_label(dtype)


def _limpar_texto_referencia(texto: str) -> str:
    normalizado = unicodedata.normalize("NFKC", str(texto or ""))
    caracteres = []
    for char in normalizado:
        categoria = unicodedata.category(char)
        if char.isspace():
            caracteres.append(" ")
        elif categoria[0] in {"L", "N"}:
            caracteres.append(char)
    return re.sub(r"\s+", " ", "".join(caracteres)).strip()


def _concatenar_blocos_de_fala(audio: np.ndarray, blocos: np.ndarray, limite_amostras: int) -> np.ndarray:
    partes = []
    total = 0
    for inicio, fim in blocos:
        if total >= limite_amostras:
            break
        trecho = audio[int(inicio):int(fim)]
        disponivel = limite_amostras - total
        if len(trecho) > disponivel:
            trecho = trecho[:disponivel]
        if len(trecho) > 0:
            partes.append(trecho)
            total += len(trecho)
    if not partes:
        return np.array([], dtype=np.float32)
    return np.concatenate(partes).astype(np.float32, copy=False)


def preparar_referencia_curta(
    caminho_origem: str,
    caminho_saida: str,
    sample_rate: int = SAMPLE_RATE_TTS,
) -> ShortReference:
    if not os.path.exists(caminho_origem):
        raise FileNotFoundError(f"Audio de referencia nao encontrado: {caminho_origem}")

    audio_raw, _ = librosa.load(caminho_origem, sr=sample_rate, mono=True)
    audio = np.asarray(audio_raw, dtype=np.float32).flatten()
    audio = np.nan_to_num(audio, nan=0.0, posinf=0.0, neginf=0.0)
    if len(audio) == 0:
        raise ValueError("Audio de referencia vazio.")

    origem_segundos = len(audio) / sample_rate
    alvo_amostras = max(1, int(REFERENCE_TARGET_SECONDS * sample_rate))
    max_amostras = max(1, int(REFERENCE_MAX_SECONDS * sample_rate))
    min_amostras = max(1, int(REFERENCE_MIN_SECONDS * sample_rate))

    blocos = librosa.effects.split(audio, top_db=40)
    inicio = int(blocos[0][0]) if len(blocos) else 0
    fim = min(len(audio), inicio + alvo_amostras)
    referencia = audio[inicio:fim]

    if len(referencia) < min_amostras and len(blocos):
        referencia = _concatenar_blocos_de_fala(audio, blocos, alvo_amostras)
        inicio = int(blocos[0][0])

    if len(referencia) == 0:
        referencia = audio[:alvo_amostras]
        inicio = 0

    if len(referencia) > max_amostras:
        referencia = referencia[:max_amostras]

    if len(referencia) == 0:
        raise ValueError("Nao foi possivel extrair uma referencia de voz valida.")

    os.makedirs(os.path.dirname(caminho_saida) or ".", exist_ok=True)
    sf.write(caminho_saida, np.clip(referencia, -0.999, 0.999), sample_rate)
    duracao_ref = len(referencia) / sample_rate
    if duracao_ref > REFERENCE_MAX_SECONDS + (1.0 / sample_rate):
        raise RuntimeError(
            f"Referencia curta excedeu {REFERENCE_MAX_SECONDS:.1f}s: {duracao_ref:.2f}s"
        )
    return ShortReference(
        path=caminho_saida,
        duration_seconds=duracao_ref,
        source_duration_seconds=origem_segundos,
        start_seconds=inicio / sample_rate,
    )


def transcrever_referencia_curta(
    whisper_model,
    caminho_ref: str,
    source_lang: str,
    fallback_text: str,
    log: Optional[Callable[[str, str], None]] = None,
) -> str:
    kwargs: dict[str, object] = {"temperature": 0.0}
    if source_lang in {"en", "pt", "fr", "sv"}:
        kwargs["language"] = source_lang
    texto_ref = ""
    try:
        resultado = whisper_model.transcribe(caminho_ref, **kwargs)
        texto_ref = _limpar_texto_referencia(resultado.get("text", ""))
    except Exception as exc:
        if log:
            log(f"⚠️ Transcricao da referencia curta falhou: {exc}", "warning")

    if not texto_ref:
        texto_ref = _limpar_texto_referencia(fallback_text)
    if not texto_ref:
        raise ValueError("Nao foi possivel obter texto para a referencia curta.")
    return texto_ref


def criar_prompt_referencia_curta(
    omni_model,
    whisper_model,
    caminho_origem: str,
    caminho_ref_saida: str,
    source_lang: str,
    fallback_text: str,
    log: Callable[[str, str], None],
) -> tuple[object, ShortReference, str]:
    referencia = preparar_referencia_curta(caminho_origem, caminho_ref_saida)
    texto_ref = transcrever_referencia_curta(
        whisper_model,
        referencia.path,
        source_lang,
        fallback_text,
        log,
    )
    log(
        (
            f"Referencia de voz curta: {referencia.duration_seconds:.2f}s "
            f"(origem {referencia.source_duration_seconds:.2f}s, inicio {referencia.start_seconds:.2f}s)."
        ),
        "info",
    )
    prompt = omni_model.create_voice_clone_prompt(
        ref_audio=referencia.path,
        ref_text=texto_ref,
        preprocess_prompt=True,
    )
    return prompt, referencia, texto_ref


def get_duracao_exata(caminho):
    """Mantida para compatibilidade com ACCENT_FIX1."""
    data, sr = sf.read(caminho)
    return len(data) / sr


def aplicar_virgula_interrogacao(texto):
    """Antes de toda '?' coloca ', ?' — pausa natural."""
    texto = re.sub(r',\s*\?', ', ?', texto)
    texto = re.sub(r'([^,\s])\s*\?', r'\1, ?', texto)
    return texto


def palatalizar_ptbr(texto):
    """
    Palatização PT-BR: palavras terminadas em ti/te/di/de → tchi/tche/dchi/dche.
    """
    letra = r'A-Za-zÀ-ÖØ-öø-ÿ'
    texto = re.sub(fr'(?<=[{letra}])tis\b', 'chis', texto)
    texto = re.sub(fr'(?<=[{letra}])Tis\b', 'Chis', texto)
    texto = re.sub(fr'(?<=[{letra}])tes\b', 'ches', texto)
    texto = re.sub(fr'(?<=[{letra}])Tes\b', 'Ches', texto)
    texto = re.sub(fr'(?<=[{letra}])ti\b',  'chi',  texto)
    texto = re.sub(fr'(?<=[{letra}])Ti\b',  'Chi',  texto)
    texto = re.sub(fr'(?<=[{letra}])te\b',  'che',  texto)
    texto = re.sub(fr'(?<=[{letra}])Te\b',  'Che',  texto)
    texto = re.sub(fr'(?<=[{letra}])dis\b', 'dchis', texto)
    texto = re.sub(fr'(?<=[{letra}])Dis\b', 'Dchis', texto)
    texto = re.sub(fr'(?<=[{letra}])des\b', 'dches', texto)
    texto = re.sub(fr'(?<=[{letra}])Des\b', 'Dches', texto)
    texto = re.sub(fr'(?<=[{letra}])di\b',  'dchi',  texto)
    texto = re.sub(fr'(?<=[{letra}])Di\b',  'Dchi',  texto)
    texto = re.sub(fr'(?<=[{letra}])de\b',  'dche',  texto)
    texto = re.sub(fr'(?<=[{letra}])De\b',  'Dche',  texto)
    return texto

def sincronizar_virgulas_proporcional(texto_pt, texto_en):
    """
    Injeta vírgulas no texto PT nas posições proporcionais ao EN original.
    Objetivo: preservar o ritmo/pausas do original na dublagem PT.

    Algoritmo:
    1. Localiza vírgulas no EN (posição como fração do total de palavras)
    2. Mapeia para a posição equivalente no PT
    3. Insere vírgula se não houver pontuação próxima (±1 palavra)

    Conservador: só adiciona, nunca remove. Se PT já tem >= vírgulas que EN, não toca.
    """
    if not texto_pt or not texto_en:
        return texto_pt

    # Tokenizar em palavras, preservando pontuação colada
    palavras_en = texto_en.split()
    palavras_pt = texto_pt.split()

    if not palavras_en or not palavras_pt:
        return texto_pt

    # Contar e localizar vírgulas no EN (posição = índice da palavra com vírgula)
    virgulas_en = []
    for i, w in enumerate(palavras_en):
        if ',' in w or ';' in w:
            virgulas_en.append(i / len(palavras_en))  # fração 0.0–1.0

    # Contar vírgulas no PT
    virgulas_pt_atual = sum(1 for w in palavras_pt if ',' in w or ';' in w)

    # Se PT já tem vírgulas suficientes, não interferir
    if virgulas_pt_atual >= len(virgulas_en) or not virgulas_en:
        return texto_pt

    # Inserir vírgulas faltantes nas posições proporcionais
    resultado = list(palavras_pt)
    inseridas = 0
    for fracao in virgulas_en:
        pos_pt = int(fracao * len(resultado))
        pos_pt = max(1, min(pos_pt, len(resultado) - 2))  # nunca no início ou fim

        # Não inserir se já há pontuação na palavra atual ou vizinha
        vizinhas = resultado[max(0, pos_pt-1):pos_pt+2]
        if any(',' in v or ';' in v or '.' in v or '?' in v or '!' in v for v in vizinhas):
            continue

        # Adicionar vírgula ao final da palavra na posição
        resultado[pos_pt] = resultado[pos_pt].rstrip() + ','
        inseridas += 1

    return ' '.join(resultado)


# ─── FUNÇÕES COPIADAS DO OMNI11 ─────────────────────────────────────────────

def sincronizar_pontuacao(texto_base, texto_referencia):
    if not texto_base or not texto_referencia: return texto_base
    texto_base = str(texto_base).strip()
    texto_referencia = str(texto_referencia).strip()
    texto_limpo = re.sub(r'[\.?\!;:,\s\"\'""'']+$', '', texto_base)
    ref_final = re.sub(r'[\s\"\'""'']+$', '', texto_referencia)
    if ref_final.endswith('?'): return texto_limpo + '?'
    if ref_final.endswith('!'): return texto_limpo + '!'
    if ref_final.endswith('.'): return texto_limpo + '.'
    return texto_limpo

def validar_qualidade_zcr(audio_np):
    zcr = librosa.feature.zero_crossing_rate(audio_np)[0]
    avg_zcr = np.mean(zcr)
    if avg_zcr > 0.45: return False, avg_zcr
    return True, avg_zcr

def validar_qualidade_audio_natural(audio_np, sr=24000):
    """Barreira contra audio metalico/chiado. Reprova em vez de salvar voz destruida."""
    try:
        y = np.asarray(audio_np, dtype=np.float32).flatten()
        y = np.nan_to_num(y, nan=0.0, posinf=0.0, neginf=0.0)
        if len(y) < int(sr * 0.05):
            return False, "Audio gerado curto demais."

        blocos = librosa.effects.split(y, top_db=38)
        if len(blocos) > 0:
            partes = [y[int(a):int(b)] for a, b in blocos if int(b) > int(a)]
            y_voz = np.concatenate(partes) if partes else y
        else:
            y_voz = y

        pico = float(np.max(np.abs(y_voz))) if len(y_voz) else 0.0
        if pico <= 1e-4:
            return False, "Audio gerado praticamente mudo."
        clip_ratio = float(np.mean(np.abs(y_voz) > 0.985))
        if clip_ratio > 0.01:
            return False, f"Audio clipando/saturado ({clip_ratio:.1%})."

        zcr = librosa.feature.zero_crossing_rate(y_voz)[0]
        zcr_med = float(np.mean(zcr))
        zcr_p95 = float(np.percentile(zcr, 95))
        flat = librosa.feature.spectral_flatness(y=y_voz)[0]
        flat_med = float(np.mean(flat))
        centroid = librosa.feature.spectral_centroid(y=y_voz, sr=sr)[0]
        centroid_med = float(np.mean(centroid))

        if zcr_med > 0.28 or (zcr_med > 0.18 and zcr_p95 > 0.62):
            return False, f"Audio metalico/chiado detectado (ZCR={zcr_med:.2f}, p95={zcr_p95:.2f})."
        if flat_med > 0.22 and centroid_med > 4200:
            return False, f"Audio com textura metalica detectada (flat={flat_med:.2f}, centroid={centroid_med:.0f})."
        return True, f"Qualidade natural OK (ZCR={zcr_med:.2f}, flat={flat_med:.2f})."
    except Exception as e:
        return False, f"Falha na analise anti-metal: {e}"

def calcular_similaridade_texto(texto1, texto2):
    t1 = re.sub(r'[^\w\s]', '', str(texto1).lower().strip())
    t2 = re.sub(r'[^\w\s]', '', str(texto2).lower().strip())
    return difflib.SequenceMatcher(None, t1, t2).ratio()

def verificar_qualidade_fala_original(resultado_whisper):
    texto_bruto = resultado_whisper.get("text", "").strip()
    tri_ok, tri_motivo = realizar_tri_checagem(texto_bruto)
    if not tri_ok:
        return False, f"ORIGINAL_RUIM: tri-checagem SEPRAR_AUDIOS reprovou: {tri_motivo}."
    texto = texto_bruto.lower().strip('.!?,;:"\' ')
    segments = resultado_whisper.get("segments", [])
    if not segments:
        return False, "ORIGINAL_RUIM: Whisper nao encontrou segmentos de voz."
    no_speech_prob = float(np.mean([s.get("no_speech_prob", 0) for s in segments]))
    avg_logprob = float(np.mean([s.get("avg_logprob", 0) for s in segments]))
    if no_speech_prob > 0.65:
        return False, f"ORIGINAL_RUIM: alta chance de ruido/grunhido (no_speech={no_speech_prob:.2f})."
    if avg_logprob < -1.2:
        return False, f"ORIGINAL_RUIM: fala confusa/inaudivel (logprob={avg_logprob:.2f})."
    grunhidos = {"ah", "oh", "uh", "hmm", "hm", "huh", "ugh", "gasp", "sigh", "ha", "eh", "whoa", "argh", "grr", "wow", "mhm"}
    if texto in grunhidos or len(texto) <= 3:
        return False, f"ORIGINAL_RUIM: detectado apenas grunhido ou fala curta ('{texto}')."
    return True, "OK"

def _normalizar_texto_qc(texto):
    """Normaliza texto para comparar ASR sem prender em acento/fonetica."""
    texto = str(texto or "").lower()
    texto = unicodedata.normalize("NFD", texto)
    texto = "".join(c for c in texto if unicodedata.category(c) != "Mn")
    for origem, destino in (
        ("dchi", "di"), ("dche", "de"),
        ("tchi", "ti"), ("tche", "te"),
        ("chi", "ti"), ("che", "te"),
    ):
        texto = texto.replace(origem, destino)
    return re.findall(r"[a-z0-9]+", texto)

def _token_parece_igual(a, b):
    if a == b:
        return True
    if len(a) <= 2 or len(b) <= 2:
        return False
    return difflib.SequenceMatcher(None, a, b).ratio() >= 0.78

def _lcs_fuzzy_count(esperado, ouvido):
    if not esperado or not ouvido:
        return 0
    anterior = [0] * (len(ouvido) + 1)
    for tok_esp in esperado:
        atual = [0] * (len(ouvido) + 1)
        for j, tok_ouv in enumerate(ouvido, 1):
            if _token_parece_igual(tok_esp, tok_ouv):
                atual[j] = anterior[j - 1] + 1
            else:
                atual[j] = max(anterior[j], atual[j - 1])
        anterior = atual
    return anterior[-1]

def calcular_cobertura_palavras(texto_esperado, texto_ouvido):
    esperado = _normalizar_texto_qc(texto_esperado)
    ouvido = _normalizar_texto_qc(texto_ouvido)
    if not esperado:
        return 1.0, True, esperado, ouvido
    cobertura = _lcs_fuzzy_count(esperado, ouvido) / max(1, len(esperado))

    palavras_finais = [t for t in esperado if len(t) > 2] or esperado
    palavras_finais = palavras_finais[-min(3, len(palavras_finais)):]
    janela_ouvida = ouvido[-max(8, len(palavras_finais) + 3):]
    pos = 0
    cauda_ok = True
    for tok in palavras_finais:
        achou = False
        while pos < len(janela_ouvida):
            if _token_parece_igual(tok, janela_ouvida[pos]):
                achou = True
                pos += 1
                break
            pos += 1
        if not achou:
            cauda_ok = False
            break
    return cobertura, cauda_ok, esperado, ouvido

def validar_audio_final_completo(caminho_saida, texto_esperado, whisper_model=None, target_lang="pt"):
    """Confere arquivo final: tem margem apos a ultima voz e o texto termina inteiro."""
    try:
        y, sr = librosa.load(caminho_saida, sr=24000, mono=True)
        if len(y) < int(sr * 0.05):
            return False, "Audio final vazio/curto demais."
        blocos = librosa.effects.split(y, top_db=38)
        if len(blocos) == 0:
            return False, "Audio final sem fala audivel."
        dur_ms = len(y) / sr * 1000.0
        fim_voz_ms = blocos[-1][1] / sr * 1000.0
        margem_final_ms = dur_ms - fim_voz_ms
        voz_partes = [y[int(a):int(b)] for a, b in blocos if int(b) > int(a)]
        voz_junta = np.concatenate(voz_partes) if voz_partes else y
        natural_ok, natural_msg = validar_qualidade_audio_natural(voz_junta, sr=sr)
        if not natural_ok:
            return False, natural_msg
        voz_rms = float(np.sqrt(np.mean(np.square(voz_junta)))) if len(voz_junta) else 0.0
        tail_n = max(1, int(sr * 0.04))
        tail_rms = float(np.sqrt(np.mean(np.square(y[-tail_n:])))) if len(y) else 0.0
        if tail_rms > max(1e-5, voz_rms * 0.08):
            return False, "Ultimos 40 ms ainda tem energia de voz/ruido; risco de letra final cortada."
        if margem_final_ms < 20:
            return False, f"Risco de corte no fim: so {margem_final_ms:.0f} ms apos a ultima voz."
    except Exception as e:
        return False, f"Falha ao inspecionar cauda do audio final: {e}"

    if whisper_model is None:
        return True, "Cauda de audio OK."

    try:
        lang = target_lang if target_lang in ("pt", "en", "fr", "sv") else None
        res = whisper_model.transcribe(caminho_saida, language=lang, temperature=0.0)
        texto_ouvido = res.get("text", "").strip()
    except Exception as e:
        return False, f"Falha na transcricao final de conferencia: {e}"

    tri_ok, tri_motivo = realizar_tri_checagem(texto_ouvido)
    if not tri_ok:
        preview = texto_ouvido if texto_ouvido else "[Vazio]"
        return False, f"Tri-checagem SEPRAR_AUDIOS reprovou: {tri_motivo}. Ouvido: '{preview}'"

    if not texto_esperado:
        return True, f"Tri-checagem SEPRAR_AUDIOS OK. Ouvido: '{texto_ouvido}'"

    similaridade = calcular_similaridade_texto(texto_esperado, texto_ouvido)
    cobertura, cauda_ok, esperado_tokens, ouvido_tokens = calcular_cobertura_palavras(texto_esperado, texto_ouvido)

    if similaridade < 0.55:
        return False, f"Texto final divergente (sim={similaridade:.2f}). Ouvido: '{texto_ouvido}'"
    if len(esperado_tokens) > 2 and cobertura < 0.70:
        return False, f"Palavras faltando (cobertura={cobertura:.2f}). Ouvido: '{texto_ouvido}'"
    if not cauda_ok:
        fim_esperado = " ".join(esperado_tokens[-3:])
        fim_ouvido = " ".join(ouvido_tokens[-8:])
        return False, f"Final incompleto. Esperado terminar com '{fim_esperado}', ouvido no fim: '{fim_ouvido}'"

    return True, f"Final completo (sim={similaridade:.2f}, cobertura={cobertura:.2f}, margem={margem_final_ms:.0f} ms)."

def calcular_duracao_tts_alvo(caminho_original, pad_ms=200):
    perfil = _perfil_temporal_original(caminho_original, 24000)
    total_ms = int(perfil.get("total_ms") or 0)
    if total_ms <= 0:
        return None
    pad_usuario = int(pad_ms or 0)
    guarda_final_ms = max(80, min(450, pad_usuario if pad_usuario > 0 else 120))
    inicio_ms = int(perfil.get("inicio_silencio_ms") or 0)
    fim_ms = max(int(perfil.get("fim_silencio_ms") or 0), guarda_final_ms)
    fim_ms = min(fim_ms, max(35, int(total_ms * 0.30)))
    inicio_ms = min(inicio_ms, max(0, total_ms - fim_ms - 80))
    alvo_voz_ms = max(80, total_ms - inicio_ms - fim_ms)
    return alvo_voz_ms / 1000.0

def corrigir_pronuncia_br(texto):
    substituicoes = {
        r'\bolho\b': 'ólho', r'\bposso\b': 'pósso', r'\bjogo\b': 'jógo',
        r'\bgosto\b': 'gósto', r'\bfora\b': 'fóra', r'\bagora\b': 'agóra',
        r'\bmilha\b': 'mílha'
    }
    for padrao, sub in substituicoes.items():
        texto = re.sub(padrao, sub, texto, flags=re.IGNORECASE)
    return texto

def _sincronizar_master_v10_1_legacy(y_gen, caminho_saida, caminho_original, silence_pad_ms=200):
    sr_gen = 24000
    y = y_gen - np.mean(y_gen)
    
    # Trim clássico (ambos os lados)
    y_trimmed, _ = librosa.effects.trim(y, top_db=35)
    y = y_trimmed

    try:
        y_orig, sr_orig = sf.read(caminho_original)
        duracao_original_ms = int((len(y_orig) / sr_orig) * 1000)
        pico_orig = np.max(np.abs(y_orig)) if len(y_orig) > 0 else 0.95
        max_gen = np.max(np.abs(y))
        if max_gen > 0:
            y = (y / max_gen) * pico_orig
    except:
        duracao_original_ms = 0
        y = y / (np.max(np.abs(y)) + 1e-6) * 0.95

    audio_int16 = (y * 32767).astype(np.int16)
    seg = AudioSegment(audio_int16.tobytes(), frame_rate=sr_gen, sample_width=2, channels=1)
    
    seg = seg.fade_in(5)
    
    if duracao_original_ms > 0:
        if len(seg) > duracao_original_ms:
            # Compatibilidade: nunca cortar fala para encaixar no tempo.
            seg = seg + AudioSegment.silent(duration=max(80, int(silence_pad_ms or 0)))
        else:
            padd = min(50, duracao_original_ms - len(seg))
            seg = seg + AudioSegment.silent(duration=padd)
            if len(seg) < duracao_original_ms:
                seg = seg + AudioSegment.silent(duration=duracao_original_ms - len(seg))
    elif silence_pad_ms > 0:
        seg = seg + AudioSegment.silent(duration=int(silence_pad_ms))

    seg.export(caminho_saida, format="wav")


# Redefinicao anti-corte: a funcao acima ficava para compatibilidade historica,
# mas esta versao e a usada pelo worker abaixo.
def _perfil_temporal_original(caminho_original, sr_alvo=24000):
    perfil = {
        "total_ms": 0,
        "inicio_silencio_ms": 0,
        "fim_silencio_ms": 0,
        "voz_ms": 0,
        "pico": 0.95,
    }
    try:
        y_orig, sr_orig = librosa.load(caminho_original, sr=sr_alvo, mono=True)
        if len(y_orig) == 0:
            return perfil
        perfil["total_ms"] = int(round(len(y_orig) / sr_orig * 1000))
        perfil["pico"] = float(np.max(np.abs(y_orig))) if len(y_orig) else 0.95
        blocos = librosa.effects.split(y_orig, top_db=35)
        if len(blocos) > 0:
            ini = int(blocos[0][0])
            fim = int(blocos[-1][1])
            perfil["inicio_silencio_ms"] = int(round(ini / sr_orig * 1000))
            perfil["fim_silencio_ms"] = int(round((len(y_orig) - fim) / sr_orig * 1000))
            perfil["voz_ms"] = int(round((fim - ini) / sr_orig * 1000))
        else:
            perfil["voz_ms"] = perfil["total_ms"]
    except Exception:
        pass
    return perfil


def _aplicar_time_stretch_seguro(y, sr, alvo_ms):
    if alvo_ms <= 0 or len(y) < int(sr * 0.05):
        return y, 1.0, 1.0
    atual_ms = len(y) / sr * 1000.0
    if atual_ms <= 0:
        return y, 1.0, 1.0
    taxa_desejada = atual_ms / alvo_ms
    # Nao usar phase-vocoder aqui: em voz, respiracao e grunhidos ele cria artefato metalico.
    # O tempo agora e buscado pelo controle nativo de duracao do OmniVoice; se nao bater, reprova.
    return y, taxa_desejada, 1.0


def sincronizar_master_v10_1(y_gen, caminho_saida, caminho_original, silence_pad_ms=200):
    """Sincroniza sem cortar a fala final. Se precisar, ajusta velocidade; nunca trunca voz."""
    sr_gen = 24000
    y = np.asarray(y_gen, dtype=np.float32).flatten()
    if len(y) == 0:
        AudioSegment.silent(duration=100, frame_rate=sr_gen).export(caminho_saida, format="wav")
        return {"status": "vazio", "duracao_final_ms": 100}

    y = np.nan_to_num(y - np.mean(y), nan=0.0, posinf=0.0, neginf=0.0)

    # Remove apenas silencio inicial. O fim fica intacto para nao amputar ultima palavra.
    blocos = librosa.effects.split(y, top_db=35)
    if len(blocos) > 0 and blocos[0][0] > 0:
        y = y[int(blocos[0][0]):]

    perfil = _perfil_temporal_original(caminho_original, sr_gen)
    total_original_ms = int(perfil.get("total_ms") or 0)
    pico_orig = float(perfil.get("pico") or 0.95)
    pico_orig = max(0.05, min(0.98, pico_orig))

    max_gen = float(np.max(np.abs(y))) if len(y) else 0.0
    if max_gen > 0:
        y = (y / max_gen) * (pico_orig * 0.96)
    else:
        y = np.zeros(max(1, int(sr_gen * 0.1)), dtype=np.float32)

    pad_usuario = int(silence_pad_ms or 0)
    guarda_final_ms = max(80, min(450, pad_usuario if pad_usuario > 0 else 120))

    inicio_ms = int(perfil.get("inicio_silencio_ms") or 0)
    fim_original_ms = int(perfil.get("fim_silencio_ms") or 0)
    fim_ms = max(fim_original_ms, guarda_final_ms)

    if total_original_ms > 0:
        # Em clips curtos, a margem precisa caber no proprio tamanho do arquivo.
        fim_ms = min(fim_ms, max(35, int(total_original_ms * 0.30)))
        inicio_ms = min(inicio_ms, max(0, total_original_ms - fim_ms - 80))
        alvo_voz_ms = max(80, total_original_ms - inicio_ms - fim_ms)
        y, taxa_desejada, taxa_aplicada = _aplicar_time_stretch_seguro(y, sr_gen, alvo_voz_ms)
    else:
        taxa_desejada = taxa_aplicada = 1.0

    y = np.clip(y, -0.999, 0.999)
    audio_int16 = (y * 32767).astype(np.int16)
    voz = AudioSegment(audio_int16.tobytes(), frame_rate=sr_gen, sample_width=2, channels=1)
    voz = voz.fade_in(5)

    seg = AudioSegment.silent(duration=inicio_ms, frame_rate=sr_gen) + voz
    seg = seg + AudioSegment.silent(duration=fim_ms, frame_rate=sr_gen)

    if total_original_ms > 0 and len(seg) < total_original_ms:
        seg = seg + AudioSegment.silent(duration=total_original_ms - len(seg), frame_rate=sr_gen)
    # Importante: se ficou maior que o original, nao corta. Cortar aqui e o bug original.

    seg.export(caminho_saida, format="wav")
    return {
        "status": "ok",
        "duracao_original_ms": total_original_ms,
        "duracao_final_ms": len(seg),
        "inicio_ms": inicio_ms,
        "fim_ms": fim_ms,
        "taxa_desejada": taxa_desejada,
        "taxa_aplicada": taxa_aplicada,
    }


# ─── POOL WORKER (compatibilidade) ──────────────────────────────────────────

class GeradorPoolWorker(QThread):
    log_signal = pyqtSignal(str, str)
    finished_signal = pyqtSignal()
    def __init__(self, pasta_pool, models_ref, parent=None):
        super().__init__(parent)
        self.pasta_pool = pasta_pool; self.models = models_ref
    def log(self, msg, level="info"): self.log_signal.emit(msg, level)
    def run(self):
        try:
            import torch
            from omnivoice import OmniVoice
            os.makedirs(self.pasta_pool, exist_ok=True)
            dev = "cuda" if torch.cuda.is_available() else "cpu"
            if self.models["omni"] is None:
                self.log("📥 Carregando OmniVoice...", "info")
                self.models["omni"], device_label, dtype_label = carregar_omnivoice_otimizado(OmniVoice, torch, dev)
            else:
                device_label, dtype_label = descrever_modelo_omnivoice(self.models["omni"])
            self.log(f"OmniVoice pronto: device={device_label}, dtype={dtype_label}", "info")
            for nome, perfil in VOICE_PROFILES_PTBR.items():
                p = os.path.join(self.pasta_pool, f"{nome}.wav")
                if os.path.exists(p): self.log(f"✅ Já existe: {nome}.wav","info"); continue
                self.log(f"🎙️ Gerando: {nome}...","info")
                a = self.models["omni"].generate(text=perfil["ref_text"],language="pt",instruct=perfil["instruct"],num_step=32,postprocess_output=True)
                sf.write(p, a[0], 24000)
                self.log(f"💾 {nome}.wav salvo","success")
            self.log("🎉 Pool PT-BR pronto!","success")
        except Exception as e:
            self.log(f"❌ {e}\n{traceback.format_exc()}","error")
        finally:
            self.finished_signal.emit()


# ─── WORKER PRINCIPAL — CORE IDÊNTICO AO OMNI11 ─────────────────────────────

class SingleDubbingWorkerV14(QThread):
    """
    Core de geração IDÊNTICO ao dublagem_omni11_kliffFIX_TESTE.py.
    Adições opcionais controladas por checkboxes (só tocam no texto):
      - palatalizar: ti→tchi, de→dche etc.
      - virgula_interrogacao: adiciona ', ?' antes de interrogações
    """
    log_signal                 = pyqtSignal(str, str)
    file_done_signal           = pyqtSignal(bool, str, str, str, str, str) # (sucesso, msg_erro, temp_path, nome_original, txt_en, txt_pt_final)
    finished_signal            = pyqtSignal()
    transcription_ready_signal = pyqtSignal(str, str)
    progress_signal            = pyqtSignal(int)

    def __init__(self, paths_en, pasta_guia, models_ref, custom_texts,
                 omni_temp, pad_ms, modo_voz="classico", pasta_pool="",
                 palatalizar=False, virgula_interrogacao=False,
                 trailing_ponto=False, target_lang="pt", source_lang="auto", parent=None):
        super().__init__(parent)
        self.target_lang  = target_lang
        self.source_lang  = source_lang
        self.paths_en     = paths_en if isinstance(paths_en, list) else [paths_en]
        self.pasta_guia   = pasta_guia
        self.models       = models_ref
        self.custom_texts = custom_texts or {}
        self.omni_temp    = float(omni_temp or 0.0)
        self.pad_ms       = pad_ms
        self.modo_voz     = modo_voz
        self.palatalizar  = palatalizar
        self.virgula_interrogacao = virgula_interrogacao
        self.trailing_ponto = trailing_ponto
        self._temp_dir    = os.path.join(os.path.dirname(__file__), "_temp_dublagem")
        os.makedirs(self._temp_dir, exist_ok=True)

    def log(self, msg, level="info"): self.log_signal.emit(msg, level)
    def prog(self, val):              self.progress_signal.emit(val)

    def run(self):
        torch = whisper = GoogleTranslator = OmniVoice = None
        for lib, cmd in [("torch","pip install torch"),("whisper","pip install openai-whisper"),
                         ("deep_translator","pip install deep-translator"),("omnivoice","pip install -e OmniVoice-master/")]:
            try:
                if lib=="torch":             import torch
                elif lib=="whisper":         import whisper
                elif lib=="deep_translator": from deep_translator import GoogleTranslator
                elif lib=="omnivoice":       from omnivoice import OmniVoice
            except ImportError as e:
                self.log(f"❌ '{lib}' não instalado: {e}","error")
                self.file_done_signal.emit(False, str(e), "", f"{lib} ausente", "", "")
                self.finished_signal.emit(); return
            except Exception as e:
                self.log(f"❌ Erro ao carregar '{lib}': {e}","error")
                self.file_done_signal.emit(False, str(e), "", f"{lib} falhou", "", "")
                self.finished_signal.emit(); return

        try:
            if self.isInterruptionRequested():
                self.log("Dublagem cancelada antes de carregar os modelos.", "warning")
                self.finished_signal.emit()
                return
            dev = "cuda" if torch.cuda.is_available() else "cpu"
            self.log(f"💻 Dispositivo: {dev.upper()}","info")

            if self.models["whisper"] is None:
                self.log("📥 Carregando Whisper MEDIUM...","info")
                self.models["whisper"] = whisper.load_model("medium", device=dev)
            if self.models["omni"] is None:
                self.log("📥 Carregando OmniVoice...","info")
                self.models["omni"], omni_device_label, omni_dtype_label = carregar_omnivoice_otimizado(
                    OmniVoice,
                    torch,
                    dev,
                )
            else:
                omni_device_label, omni_dtype_label = descrever_modelo_omnivoice(self.models["omni"])
            self.log(f"OmniVoice pronto: device={omni_device_label}, dtype={omni_dtype_label}", "info")

            # Áudio Guia
            guia_prompt = None
            guia_ref_info = None
            guia_ref_text = ""
            temp_guia_path = os.path.join(self._temp_dir, "temp_guia_referencia_curta.wav")
            guia_valido = False

            if self.pasta_guia and os.path.exists(self.pasta_guia):
                self.prog(5)
                self.log("🎙️ Lendo Áudio Guia...","info")
                try:
                    guia_prompt, guia_ref_info, guia_ref_text = criar_prompt_referencia_curta(
                        self.models["omni"],
                        self.models["whisper"],
                        self.pasta_guia,
                        temp_guia_path,
                        "auto",
                        "",
                        self.log,
                    )
                    guia_valido = True
                    self.log("✅ Áudio guia carregado como referencia curta.","success")
                except Exception as exc:
                    self.log(f"⚠️ Audio guia ignorado: {exc}", "warning")

            temp_gen_path = os.path.join(self._temp_dir, "temp_verificacao.wav")

            total = len(self.paths_en)
            for idx_p, curr_path in enumerate(self.paths_en):
                if self.isInterruptionRequested():
                    self.log("Dublagem cancelada pelo usuário.", "warning")
                    break
                nome = os.path.basename(curr_path)
                ts   = int(time.time()*1000)
                saida_final = os.path.join(self._temp_dir, f"v14_{ts}_{nome}")
                self.log(f"🎤 [{idx_p+1}/{total}] {nome}","info")

                # Progresso global proporcional ao arquivo atual
                prog_base = int((idx_p / total) * 100)
                self.prog(prog_base)

                try:
                    duracao_original = get_duracao_exata(curr_path)
                except Exception as exc:
                    motivo = f"Falha ao medir duracao do audio: {exc}"
                    self.log(f"❌ {motivo}", "error")
                    self.file_done_signal.emit(False, motivo, "", nome, "", "")
                    self.prog(int(((idx_p + 1) / total) * 100))
                    continue
                if duracao_original > OMNIVOICE_MAX_SYNTHESIS_SECONDS:
                    motivo = (
                        f"IGNORADO: audio com {duracao_original:.2f}s excede "
                        f"o limite OmniVoice de {OMNIVOICE_MAX_SYNTHESIS_SECONDS:.2f}s."
                    )
                    self.log(f"⏭️ {nome} {motivo}", "warning")
                    self.file_done_signal.emit(False, motivo, "", nome, "", "")
                    self.prog(int(((idx_p + 1) / total) * 100))
                    continue

                use_custom   = (len(self.paths_en)==1)
                txt_en       = self.custom_texts.get("en","") if use_custom else ""
                txt_pt_final = self.custom_texts.get("pt","") if use_custom else ""
                tem_texto_forcado = bool(str(txt_pt_final or "").strip())

                self.prog(prog_base + 5)

                # ═══ TRANSCRIÇÃO — IGUAL AO OMNI11 ═══
                if not txt_en:
                    self.log("Transcrevendo original...", "info")
                    w_lang = self.source_lang if self.source_lang in ['en', 'pt', 'fr', 'sv'] else None
                    res = self.models["whisper"].transcribe(curr_path, language=w_lang, temperature=0.0)
                    txt_en = res["text"].strip()
                    if not tem_texto_forcado:
                        fala_valida, motivo_original = verificar_qualidade_fala_original(res)
                        if not fala_valida:
                            self.log(f"⏩ {nome} enviado para revisao: {motivo_original}", "warning")
                            self.file_done_signal.emit(False, motivo_original, "", nome, txt_en, "")
                            self.prog(int(((idx_p + 1) / total) * 100))
                            continue
                self.prog(25)

                # ═══ TRADUÇÃO + CORREÇÃO ═══
                if not txt_pt_final:
                    if self.source_lang == self.target_lang and use_custom and txt_en:
                        txt_target = txt_en
                    else:
                        txt_target = GoogleTranslator(source='auto', target=self.target_lang).translate(txt_en)

                    if self.target_lang == "pt":
                        txt_pt_ia = corrigir_pronuncia_br(txt_target)
                        txt_pt_final = sincronizar_pontuacao(txt_pt_ia, txt_en)

                        if self.modo_voz == "antisotaque" and self.trailing_ponto:
                            txt_pt_final = txt_pt_final.rstrip() + " ."

                        if self.virgula_interrogacao:
                            txt_pt_final = aplicar_virgula_interrogacao(txt_pt_final)
                        if self.palatalizar:
                            txt_pt_final = palatalizar_ptbr(txt_pt_final)
                    else:
                        txt_pt_final = sincronizar_pontuacao(txt_target, txt_en)

                self.log(f"📄 Origem: {txt_en}","info")
                self.log(f"📄 Destino: {txt_pt_final}","info")
                if use_custom:
                    self.transcription_ready_signal.emit(txt_en, txt_pt_final)

                # ═══ REFERÊNCIA CURTA — evita condicionar o modelo com clipes longos ═══
                if guia_valido and guia_prompt is not None:
                    voice_clone_prompt = guia_prompt
                    ref_info = guia_ref_info
                    ref_text_uso = guia_ref_text
                else:
                    temp_ref_path = os.path.join(self._temp_dir, f"temp_ref_curta_{idx_p}_{ts}.wav")
                    try:
                        voice_clone_prompt, ref_info, ref_text_uso = criar_prompt_referencia_curta(
                            self.models["omni"],
                            self.models["whisper"],
                            curr_path,
                            temp_ref_path,
                            self.source_lang,
                            txt_en,
                            self.log,
                        )
                    except Exception as exc:
                        motivo = f"Falha ao preparar referencia curta: {exc}"
                        self.log(f"❌ {motivo}", "error")
                        self.file_done_signal.emit(False, motivo, "", nome, txt_en, txt_pt_final)
                        self.prog(int(((idx_p + 1) / total) * 100))
                        continue

                self.prog(40)
                duracao_tts_alvo = calcular_duracao_tts_alvo(curr_path, self.pad_ms)
                if duracao_tts_alvo:
                    self.log(f"Alvo nativo de duracao TTS: {duracao_tts_alvo:.2f}s (sem esticar depois).", "info")
                ref_dur = ref_info.duration_seconds if ref_info is not None else 0.0
                self.log(
                    (
                        "Config geracao: "
                        f"ref={ref_dur:.2f}s, alvo={duracao_tts_alvo or 0.0:.2f}s, "
                        f"device={omni_device_label}, dtype={omni_dtype_label}, "
                        f"num_step=48, limite={OMNIVOICE_MAX_SYNTHESIS_SECONDS:.0f}s, sintese unica."
                    ),
                    "info",
                )
                if ref_text_uso:
                    self.log(f"Texto da referencia curta: {ref_text_uso[:120]}", "info")

                # ═══ GERAÇÃO — contrato único OmniVoice <= 30s ═══
                audio_final_aprovado = None
                motivo_falha = ""
                if self.isInterruptionRequested():
                    motivo_falha = "Cancelado pelo usuário"
                else:
                    try:
                        self.log("⚙️ Gerando síntese única...","info")

                        kwargs_geracao = dict(
                            text=txt_pt_final,
                            voice_clone_prompt=voice_clone_prompt,
                            language=self.target_lang,
                            num_step=48,
                            guidance_scale=2.0,
                            class_temperature=0.0,
                            position_temperature=1.0,
                            temperature=0.0,
                            postprocess_output=True,
                        )
                        if duracao_tts_alvo:
                            if duracao_tts_alvo > OMNIVOICE_MAX_SYNTHESIS_SECONDS:
                                raise ValueError(
                                    "Duracao TTS acima do limite OmniVoice: "
                                    f"{duracao_tts_alvo:.2f}s > {OMNIVOICE_MAX_SYNTHESIS_SECONDS:.2f}s."
                                )
                            kwargs_geracao["duration"] = duracao_tts_alvo

                        temp_audio = self.models["omni"].generate(**kwargs_geracao)

                        if temp_audio is None:
                            motivo_falha = "OmniVoice não retornou áudio."
                        else:
                            y_raw = temp_audio[0] if isinstance(temp_audio, (list, tuple)) else temp_audio
                            y_val = np.array(y_raw).astype(np.float32).flatten()

                            qualidade_ok, qualidade_msg = validar_qualidade_audio_natural(y_val, sr=24000)
                            if not qualidade_ok:
                                motivo_falha = qualidade_msg
                                self.log(f"⚠️ {motivo_falha}", "warning")
                            else:
                                sf.write(temp_gen_path, y_val, 24000)
                                res_temp = self.models["whisper"].transcribe(temp_gen_path, language=self.target_lang, temperature=0.0)
                                texto_ouvido = res_temp["text"].strip()
                                similaridade = calcular_similaridade_texto(txt_pt_final, texto_ouvido)
                                cobertura, _, esperado_tokens, _ = calcular_cobertura_palavras(txt_pt_final, texto_ouvido)

                                if similaridade > 0.52 and (len(esperado_tokens) <= 2 or cobertura >= 0.62):
                                    meta_sync = sincronizar_master_v10_1(y_val, saida_final, curr_path, self.pad_ms)
                                    taxa_d = float(meta_sync.get("taxa_desejada", 1.0)) if isinstance(meta_sync, dict) else 1.0
                                    taxa_a = float(meta_sync.get("taxa_aplicada", 1.0)) if isinstance(meta_sync, dict) else 1.0
                                    if abs(taxa_d - taxa_a) > 0.08:
                                        self.log(
                                            f"⚠️ Ajuste de tempo limitado ({taxa_a:.2f}x pedido {taxa_d:.2f}x); nao cortei a fala.",
                                            "warning",
                                        )
                                    if isinstance(meta_sync, dict):
                                        orig_ms = int(meta_sync.get("duracao_original_ms") or 0)
                                        final_ms = int(meta_sync.get("duracao_final_ms") or 0)
                                        tolerancia_ms = max(120, int(orig_ms * 0.10))
                                        if orig_ms > 0 and final_ms > 0 and abs(final_ms - orig_ms) > tolerancia_ms:
                                            motivo_falha = (
                                                f"Tempo fora do original ({final_ms} ms vs {orig_ms} ms; "
                                                f"tol={tolerancia_ms} ms)."
                                            )
                                            self.log(f"⚠️ {motivo_falha}", "warning")

                                    if not motivo_falha:
                                        ok_final, detalhe_final = validar_audio_final_completo(
                                            saida_final, txt_pt_final, self.models["whisper"], self.target_lang
                                        )
                                        if ok_final:
                                            audio_final_aprovado = saida_final
                                            self.log(f"✅ Aprovado: {detalhe_final}","success")
                                        else:
                                            motivo_falha = f"Conferencia final reprovou: {detalhe_final}"
                                            self.log(f"⚠️ {motivo_falha}","warning")
                                else:
                                    motivo_falha = f"Alucinação/omissao (sim={similaridade:.2f}, cobertura={cobertura:.2f})"
                                    self.log(f"⚠️ {motivo_falha}","warning")

                    except Exception as e:
                        motivo_falha = str(e)
                        self.log(f"❌ {e}","error")

                # ═══ APROVACAO FINAL — arquivo ja sincronizado e conferido ═══
                if audio_final_aprovado is not None:
                    self.log(f"✅ [{idx_p+1}/{total}] {nome} — Concluído!","success")
                    self.file_done_signal.emit(True, "", saida_final, nome, txt_en, txt_pt_final)
                else:
                    try:
                        if os.path.exists(saida_final):
                            os.remove(saida_final)
                    except Exception:
                        pass
                    self.log(f"❌ [{idx_p+1}/{total}] {nome} Falhou: {motivo_falha}","error")
                    self.file_done_signal.emit(False, motivo_falha, "", nome, "", "")

                # ── Limpeza de memória após cada arquivo (evita degradação em batch) ──
                del audio_final_aprovado
                audio_final_aprovado = None
                gc.collect()
                if torch.cuda.is_available():
                    torch.cuda.empty_cache()
                    # Log de VRAM a cada 10 arquivos
                    if (idx_p + 1) % 10 == 0:
                        vram_livre = torch.cuda.mem_get_info()[0] / 1024**3
                        self.log(f"📊 [{idx_p+1}/{total}] VRAM livre: {vram_livre:.1f} GB", "info")

                self.prog(int(((idx_p + 1) / total) * 100))

            gc.collect()
            if torch.cuda.is_available(): torch.cuda.empty_cache()
            self.finished_signal.emit()

        except Exception as e:
            self.log(f"❌ Erro fatal:\n{traceback.format_exc()}","error")
            self.file_done_signal.emit(False, str(e), "", "Erro Fatal", "", "")
            self.finished_signal.emit()
