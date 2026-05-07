# v10.1.4 - Edição "Fixed Guide & Absolute Zero" - RTX 5070 Ti
# Foco: Sotaque Zero Absoluto com Áudio Guia Fixo e Temperaturas em 0.0.
import os
import torch
import gradio as gr
import whisper
import numpy as np
import gc
import librosa
import soundfile as sf
import datetime
import time
import re
from omnivoice import OmniVoice
from deep_translator import GoogleTranslator
from pathlib import Path
from pydub import AudioSegment
import warnings

warnings.filterwarnings("ignore")

# --- CONFIGURAÇÃO DE HARDWARE ---
device = "cuda" if torch.cuda.is_available() else "cpu"
print(f"🚀 ENGINE v10.1.4 ATIVA: {device.upper()} (RTX 5070 Ti)")

models = {"whisper": None, "omni": None}

def carregar_modelos():
    """Carrega os motores de IA com foco em precisão industrial."""
    if models["whisper"] is None:
        print("📥 Carregando Whisper MEDIUM (Alta Consistência)...")
        models["whisper"] = whisper.load_model("medium", device=device)
    
    if models["omni"] is None:
        print("📥 Carregando OmniVoice na GPU...")
        models["omni"] = OmniVoice.from_pretrained("k2-fsa/OmniVoice")
        models["omni"].to(device)
    return "Motores prontos."

def validar_qualidade_zcr(audio_np):
    """Validação Pós-Gerada: Usa ZCR para detectar voz metálica/robótica."""
    zcr = librosa.feature.zero_crossing_rate(audio_np)[0]
    avg_zcr = np.mean(zcr)
    if avg_zcr > 0.45:
        return False, avg_zcr
    return True, avg_zcr

def verificar_esforco_puro(texto_en):
    """Detecta áudios sem fala real para manter o som original."""
    texto_limpo = texto_en.lower().strip('.!?,;:"\' ')
    grunhidos = ["ah", "oh", "uh", "hmm", "hm", "huh", "ugh", "gasp", "sigh", "ha", "eh", "whoa", "argh", "grr", "wow"]
    if not texto_en or len(texto_en) <= 2 or texto_limpo in grunhidos:
        return True
    return False

def corrigir_pronuncia_br(texto):
    """Força vogais abertas para neutralizar sotaques."""
    substituicoes = {
        r'\bolho\b': 'ólho',
        r'\bposso\b': 'pósso',
        r'\bjogo\b': 'jógo',
        r'\bgosto\b': 'gósto',
        r'\bfora\b': 'fóra',
        r'\bagora\b': 'agóra',
        r'\bpor\b': 'pór',
        r'\bmilha\b': 'mílha'
    }
    for padrao, sub in substituicoes.items():
        texto = re.sub(padrao, sub, texto, flags=re.IGNORECASE)
    return texto

def sincronizar_master_v10_1(audio_gen, duracao_original_ms, caminho_saida, caminho_original):
    """Sincronização Industrial Master."""
    sr_gen = 24000
    y = np.array(audio_gen).astype(np.float32).flatten()
    y = y - np.mean(y)
    
    y_trimmed, _ = librosa.effects.trim(y, top_db=35)
    y = y_trimmed

    try:
        y_orig, _ = sf.read(caminho_original)
        pico_orig = np.max(np.abs(y_orig)) if len(y_orig) > 0 else 0.95
        max_gen = np.max(np.abs(y))
        if max_gen > 0:
            y = (y / max_gen) * pico_orig
    except:
        y = y / (np.max(np.abs(y)) + 1e-6) * 0.95

    audio_int16 = (y * 32767).astype(np.int16)
    seg = AudioSegment(audio_int16.tobytes(), frame_rate=sr_gen, sample_width=2, channels=1)
    
    seg = seg.fade_out(15)
    
    if len(seg) > duracao_original_ms:
        seg = seg[:duracao_original_ms].fade_out(50)
    else:
        padd = min(50, duracao_original_ms - len(seg))
        seg = seg + AudioSegment.silent(duration=padd)
        if len(seg) < duracao_original_ms:
            seg = seg + AudioSegment.silent(duration=duracao_original_ms - len(seg))

    seg.export(caminho_saida, format="wav")

def processar_dublagem(pasta_in, pasta_out, pasta_guia, progresso=gr.Progress()):
    carregar_modelos()
    p_in = Path(pasta_in.strip('"').strip("'"))
    p_out = Path(pasta_out.strip('"').strip("'"))
    p_out.mkdir(parents=True, exist_ok=True)
    
    # --- CONFIGURAÇÃO DE GUIA FIXO ---
    path_guia = pasta_guia.strip('"').strip("'")
    txt_guia_fixo = ""
    if path_guia and os.path.exists(path_guia):
        print(f"🎯 Mapeando Guia Fixo: {os.path.basename(path_guia)}")
        res_g = models["whisper"].transcribe(path_guia, language='pt', temperature=0.0)
        txt_guia_fixo = res_g["text"].strip()
    
    arquivos = sorted(list(p_in.glob("*.wav")) + list(p_in.glob("*.mp3")))
    total = len(arquivos)
    sucesso = 0

    print(f"\n🎧 SESSÃO v10.1.4 | FOCO: GUIA FIXO & DETERMINÍSTICO | TOTAL: {total}\n")

    for i, arq in enumerate(arquivos):
        saida_final = p_out / arq.name
        progresso((i/total), desc=f"Dublando: {arq.name}")
        
        try:
            audio_orig_segment = AudioSegment.from_file(str(arq))
            duracao_ms = len(audio_orig_segment)

            # Transcrição ESTÉRIL (Temperature 0.0)
            res = models["whisper"].transcribe(str(arq), language='en', temperature=0.0)
            txt_en = res["text"].strip()

            if verificar_esforco_puro(txt_en):
                audio_orig_segment.export(str(saida_final), format="wav")
                sucesso += 1
                continue

            txt_pt = GoogleTranslator(source='en', target='pt').translate(txt_en)
            txt_pt_ia = corrigir_pronuncia_br(txt_pt)

            # Preparação da referência (Usa o GUIA FIXO se existir, senão usa o original)
            if path_guia and os.path.exists(path_guia):
                ref_audio_final = path_guia
                ref_text_final = txt_guia_fixo
            else:
                y_orig, sr_orig = librosa.load(str(arq), sr=24000)
                y_orig_trimmed, _ = librosa.effects.trim(y_orig, top_db=40)
                temp_ref_path = "temp_ref_stable.wav"
                sf.write(temp_ref_path, y_orig_trimmed, sr_orig)
                ref_audio_final = temp_ref_path
                ref_text_final = re.sub(r'[^a-zA-Z\s]', '', txt_en)

            audio_gen = None
            for tentativa in range(4):
                try:
                    # GERAÇÃO DETERMINÍSTICA (Temperature 0.0)
                    temp_audio = models["omni"].generate(
                        text=txt_pt_ia,
                        ref_audio=ref_audio_final, 
                        ref_text=ref_text_final,
                        language="pt",
                        temperature=0.0
                    )
                    
                    if temp_audio is not None:
                        y_val = np.array(temp_audio).astype(np.float32).flatten()
                        valido, score = validar_qualidade_zcr(y_val)
                        if not valido: continue
                        audio_gen = temp_audio
                        break
                except Exception as e:
                    print(f"Falha: {e}")

            if audio_gen is not None:
                sincronizar_master_v10_1(audio_gen, duracao_ms, str(saida_final), str(arq))
                sucesso += 1
            
        except Exception as e:
            print(f"❌ Erro em {arq.name}: {e}")
            
        if i % 30 == 0:
            gc.collect()
            torch.cuda.empty_cache()
            
    if os.path.exists("temp_ref_stable.wav"): os.remove("temp_ref_stable.wav")
            
    return f"🏁 FINALIZADO v10.1.4!\n✅ Sucesso: {sucesso} arquivos com Guia Fixo (Sotaque Zero)."

# --- INTERFACE ---
with gr.Blocks(title="OverFPS Dublador v10.1.4") as demo:
    gr.Markdown("# 🎙️ Dublador Master Industrial (v10.1.4 Fixed Guide)")
    gr.Markdown(
        "**Novidades:**\n"
        "- 🎯 **Guia Fixo:** Se você colocar um áudio em PT-BR, a IA usará esse ritmo para todos os arquivos.\n"
        "- 🗣️ **Omni Generation (Temp 0.0):** Sotaque no mínimo absoluto.\n"
        "- 🔍 **Whisper (Temp 0.0):** Máxima fidelidade.\n"
        "- 🛡️ **ZCR Guard:** Qualidade de áudio preservada."
    )
    
    with gr.Row():
        in_p = gr.Textbox(label="Pasta Origem (EN)")
        out_p = gr.Textbox(label="Pasta Destino (PT)")
    
    with gr.Row():
        guia_p = gr.Textbox(label="Áudio Guia Fixo (PT-BR)", placeholder="Ex: D:\\meu_audio_guia.wav", info="Use um áudio com o sotaque perfeito do Kliff para eliminar a cadência gringa.")
        
    btn = gr.Button("🚀 INICIAR DUBLAGEM v10.1.4", variant="primary")
    status = gr.Textbox(label="Relatório Final", lines=3)
    
    btn.click(processar_dublagem, inputs=[in_p, out_p, guia_p], outputs=status)

if __name__ == "__main__":
    demo.launch(inbrowser=True)