# v10.2.1 FIX10 - GRUNT SHIELD + CONTROLE DE QUALIDADE ESTILOSO + ANTI-ALUCINAÇÃO + SEPARAÇÃO E FIM SUAVE
# [NOVO] Interface Moderna CustomTkinter + Carregamento Animado + Limite 6 min Áudio Guia

import os
import sys
import torch
import whisper
import numpy as np
import gc
import librosa
import soundfile as sf
import datetime
import time
import re
import shutil
import difflib
import threading
import customtkinter as ctk
from tkinter import filedialog
from omnivoice import OmniVoice
from deep_translator import GoogleTranslator
from pathlib import Path
from pydub import AudioSegment
import warnings

warnings.filterwarnings("ignore")

device = "cuda" if torch.cuda.is_available() else "cpu"
# O print agora será redirecionado para a interface assim que ela abrir

models = {"whisper": None, "omni": None}

def limpar_caminho(p):
    """Remove quebras de linha, espaços e aspas que causam erros no Windows."""
    return p.strip().replace("\n", "").replace("\r", "").replace('"', '').replace("'", "")

def carregar_modelos():
    if models["whisper"] is None:
        print("📥 Carregando Whisper MEDIUM...")
        models["whisper"] = whisper.load_model("medium", device=device)
    
    if models["omni"] is None:
        print("📥 Carregando OmniVoice...")
        models["omni"] = OmniVoice.from_pretrained("k2-fsa/OmniVoice")
        models["omni"].to(device)
        print("✅ Modelos carregados com sucesso na GPU!")

def sincronizar_pontuacao(texto_base, texto_referencia):
    """Garante que o texto PT tenha a mesma pontuação final que o EN."""
    if not texto_base or not texto_referencia:
        return texto_base
    texto_base = str(texto_base).strip()
    texto_referencia = str(texto_referencia).strip()
    texto_limpo = re.sub(r'[\.\?\!\;\:\,\s\"\'\”\’]+$', '', texto_base)
    ref_final = re.sub(r'[\s\"\'\”\’]+$', '', texto_referencia)
    if ref_final.endswith('?'): return texto_limpo + '?'
    if ref_final.endswith('!'): return texto_limpo + '!'
    return texto_limpo + '.'

def validar_qualidade_zcr(audio_np):
    """Verifica se o áudio gerado não está com ruído excessivo."""
    zcr = librosa.feature.zero_crossing_rate(audio_np)[0]
    avg_zcr = np.mean(zcr)
    if avg_zcr > 0.45:
        return False, avg_zcr
    return True, avg_zcr

def calcular_similaridade_texto(texto1, texto2):
    """Calcula a porcentagem de semelhança entre dois textos."""
    t1 = re.sub(r'[^\w\s]', '', str(texto1).lower().strip())
    t2 = re.sub(r'[^\w\s]', '', str(texto2).lower().strip())
    return difflib.SequenceMatcher(None, t1, t2).ratio()

def verificar_qualidade_fala_original(resultado_whisper):
    """
    Analisa se o áudio original é fala compreensível.
    Retorna (True, "") se for bom, ou (False, "Motivo") se for ruim.
    """
    texto = resultado_whisper["text"].lower().strip('.!?,;:"\' ')
    if not texto: return False, "Áudio vazio ou sem voz detectada."

    segments = resultado_whisper.get("segments", [])
    if not segments: return False, "Whisper não encontrou segmentos de voz."

    no_speech_prob = np.mean([s.get("no_speech_prob", 0) for s in segments])
    avg_logprob = np.mean([s.get("avg_logprob", 0) for s in segments])

    if no_speech_prob > 0.65:
        return False, f"Alta probabilidade de ruído/grunhido (No Speech Prob: {no_speech_prob:.2f})."
    
    if avg_logprob < -1.2:
        return False, f"Voz muito confusa/distorcida, Whisper não tem certeza do que ouviu (LogProb: {avg_logprob:.2f})."

    grunhidos = ["ah", "oh", "uh", "hmm", "hm", "huh", "ugh", "gasp", "sigh", "ha", "eh", "whoa", "argh", "grr", "wow", "mhm"]
    if texto in grunhidos or len(texto) <= 3:
        return False, f"Detectado apenas grunhido/respiração curta ('{texto}')."

    return True, "OK"

def corrigir_pronuncia_br(texto):
    """Dicionário de correção manual para melhorar a naturalidade."""
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

def sincronizar_master_v10_1(y_gen, caminho_saida, caminho_original):
    """Sincroniza apenas o VOLUME e limpa o INÍCIO, sem mexer no final."""
    sr_gen = 24000
    y = y_gen - np.mean(y_gen)

    edges = librosa.effects.split(y, top_db=25)
    if len(edges) > 0:
        y = y[edges[0][0]:]

    try:
        y_orig, _ = sf.read(caminho_original)
        pico_orig = np.max(np.abs(y_orig)) if len(y_orig) > 0 else 0.90
        max_gen = np.max(np.abs(y))
        if max_gen > 0:
            y = (y / max_gen) * (pico_orig * 0.96)
    except:
        pass

    audio_int16 = (y * 32767).astype(np.int16)
    seg = AudioSegment(audio_int16.tobytes(), frame_rate=sr_gen, sample_width=2, channels=1)
    seg = seg.fade_in(10)
    seg = seg + AudioSegment.silent(duration=200)
    seg.export(caminho_saida, format="wav")

def registrar_log(pasta_log, nome_arquivo, motivo):
    """Grava o erro no arquivo de log para revisão."""
    caminho_log = pasta_log / "log_qualidade.txt"
    agora = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    with open(caminho_log, "a", encoding="utf-8") as f:
        f.write(f"[{agora}] ARQUIVO: {nome_arquivo} | MOTIVO: {motivo}\n")


def processar_dublagem(pasta_in, pasta_out, pasta_guia, progresso_callback, app_state):
    """Função principal que agora aceita o estado do app para pausar ou cancelar."""
    
    if progresso_callback:
        progresso_callback(0, desc="📥 Carregando Modelos de IA (Aguarde)...", modo="indeterminate")
        
    carregar_modelos()

    pasta_in = limpar_caminho(pasta_in)
    pasta_out = limpar_caminho(pasta_out)
    pasta_guia = limpar_caminho(pasta_guia)

    p_in = Path(pasta_in)
    p_out = Path(pasta_out)
    p_out.mkdir(parents=True, exist_ok=True)
    
    p_rejeitados = p_out / "rejeitados_revisao_manual"
    p_rejeitados.mkdir(parents=True, exist_ok=True)
    
    txt_guia_fixo = ""
    temp_guia_6min_path = "temp_guia_6minutos.wav"
    
    # Processa o áudio base/guia - LIMITADO AOS PRIMEIROS 6 MINUTOS (360s)
    if pasta_guia and os.path.exists(pasta_guia):
        try:
            print("🎙️ Lendo Áudio Base/Guia (Extraindo os primeiros 6 minutos)...")
            if progresso_callback:
                progresso_callback(0, desc="⏳ Lendo Áudio Guia (Processando 6 minutos de áudio)...", modo="indeterminate")
            
            # Corta o áudio em exatos 360 segundos (6 minutos)
            y_g, sr_g = librosa.load(pasta_guia, sr=24000, duration=360.0)
            sf.write(temp_guia_6min_path, y_g, sr_g)
            
            res_g = models["whisper"].transcribe(temp_guia_6min_path, language='pt', temperature=0.0)
            txt_guia_fixo = res_g["text"].strip()
            
            # Atualiza a referência de pasta_guia para a IA usar o áudio cortado
            pasta_guia = temp_guia_6min_path 
            
            print("✅ Áudio Base lido com sucesso!")
        except Exception as e:
            print(f"⚠️ Erro ao ler áudio guia, ignorando guia. Erro: {e}")
            pasta_guia = ""
            
    if progresso_callback:
        progresso_callback(0, desc="✅ Iniciando dublagem dos arquivos...", modo="determinate")
    
    arquivos = sorted(list(p_in.glob("*.wav")) + list(p_in.glob("*.mp3")))
    total = len(arquivos)
    sucesso = 0
    ignorados = 0

    temp_gen_path = "temp_verificacao.wav"
    temp_ref_path = "temp_ref_trimmed.wav"

    for i, arq in enumerate(arquivos):
        # Controle de Pausa e Cancelamento
        while app_state["pausar"]:
            if app_state["cancelar"]:
                break
            time.sleep(0.5)

        if app_state["cancelar"]:
            print("🛑 Processamento Cancelado pelo Usuário.")
            break

        saida_final = p_out / arq.name
        
        if progresso_callback:
            progresso_callback((i/total), desc=f"Processando [{i+1}/{total}]: {arq.name}", modo="determinate")
        
        try:
            res = models["whisper"].transcribe(str(arq), language='en', temperature=0.0)
            
            fala_valida, motivo_falha = verificar_qualidade_fala_original(res)
            
            if not fala_valida:
                print(f"⏩ Rejeitado (Origem Ruim): {arq.name} - {motivo_falha}")
                shutil.move(str(arq), str(p_rejeitados / arq.name))
                registrar_log(p_rejeitados, arq.name, f"ORIGINAL RUIM: {motivo_falha}")
                ignorados += 1
                continue

            txt_en = res["text"].strip()
            txt_pt = GoogleTranslator(source='en', target='pt').translate(txt_en)
            txt_pt_ia = corrigir_pronuncia_br(txt_pt)
            txt_pt_final = sincronizar_pontuacao(txt_pt_ia, txt_en)
            
            audio_final_aprovado = None
            motivo_falha_geracao = "Falha desconhecida"

            if not pasta_guia:
                try:
                    y_orig_ref, sr_orig_ref = librosa.load(str(arq), sr=24000)
                    y_trim, _ = librosa.effects.trim(y_orig_ref, top_db=45)
                    sf.write(temp_ref_path, y_trim, sr_orig_ref)
                    ref_audio_uso = temp_ref_path
                    ref_text_uso = txt_en
                except Exception as e:
                    ref_audio_uso = str(arq)
                    ref_text_uso = txt_en
            else:
                ref_audio_uso = pasta_guia
                ref_text_uso = txt_guia_fixo

            for tentativa in range(3):
                # Mais um check de segurança para cancelar entre tentativas pesadas
                if app_state["cancelar"]: break

                try:
                    temp_audio = models["omni"].generate(
                        text=txt_pt_final,
                        ref_audio=ref_audio_uso,
                        ref_text=ref_text_uso,
                        language="pt",
                        temperature=0.0
                    )
                    
                    if temp_audio is not None:
                        y_val = np.array(temp_audio).astype(np.float32).flatten()
                        
                        valido_zcr, _ = validar_qualidade_zcr(y_val)
                        if not valido_zcr: 
                            motivo_falha_geracao = "Áudio gerado com chiado/ruído metálico (ZCR alto)."
                            continue
                        
                        sf.write(temp_gen_path, y_val, 24000)
                        res_temp = models["whisper"].transcribe(temp_gen_path, language='pt', temperature=0.0)
                        texto_ouvido = res_temp["text"].strip()
                        
                        similaridade = calcular_similaridade_texto(txt_pt_final, texto_ouvido)
                        
                        if similaridade > 0.55:
                            audio_final_aprovado = y_val
                            break
                        else:
                            motivo_falha_geracao = f"Alucinação na tentativa {tentativa+1}. Esperado: '{txt_pt_final}' | Ouvido: '{texto_ouvido}' | Sim: {similaridade:.2f}"
                            print(f"⚠️ {arq.name}: {motivo_falha_geracao}")
                            
                except Exception as e:
                    motivo_falha_geracao = f"Erro no OmniVoice: {str(e)}"
                    continue

            if audio_final_aprovado is not None:
                sincronizar_master_v10_1(audio_final_aprovado, str(saida_final), str(arq))
                sucesso += 1
            else:
                if not app_state["cancelar"]:
                    print(f"❌ Rejeitado (Falha na IA): {arq.name} após 3 tentativas.")
                    shutil.move(str(arq), str(p_rejeitados / arq.name))
                    registrar_log(p_rejeitados, arq.name, f"FALHA GERAÇÃO IA: {motivo_falha_geracao}")
                    ignorados += 1

        except Exception as e:
            print(f"❌ Erro Crítico em {arq.name}: {e}")
            
        if i % 15 == 0:
            gc.collect()
            torch.cuda.empty_cache()
            
    if os.path.exists("temp_ref_stable.wav"): os.remove("temp_ref_stable.wav")
    if os.path.exists(temp_gen_path): os.remove(temp_gen_path)
    if os.path.exists(temp_ref_path): os.remove(temp_ref_path)
    if os.path.exists(temp_guia_6min_path): os.remove(temp_guia_6min_path)
    
    if app_state["cancelar"]:
        if progresso_callback:
            progresso_callback(0.0, desc="Aguardando...", modo="determinate")
        return "🛑 Operação Cancelada. Você pode iniciar novamente."

    if progresso_callback:
        progresso_callback(1.0, desc="Finalizado!", modo="determinate")
        
    return f"🏁 FINALIZADO!\n✅ Dublados com Sucesso: {sucesso}\n⚠️ Movidos p/ Revisão: {ignorados}\n📂 Total: {total}\nVerifique a pasta de rejeitados!"

# ==============================================================================
# NOVA INTERFACE GRÁFICA (CustomTkinter)
# ==============================================================================

class DubladorApp(ctk.CTk):
    def __init__(self):
        super().__init__()

        # Estado da Aplicação
        self.app_state = {"pausar": False, "cancelar": False}

        # Configurações da Janela
        self.title("🎙️ Dublador QC-PRO v10.2.1 FIX10")
        self.geometry("800x700")
        ctk.set_appearance_mode("Dark")
        ctk.set_default_color_theme("blue")

        self.grid_columnconfigure(0, weight=1)
        self.grid_rowconfigure(4, weight=1)

        # 1. Título e Descrição
        self.title_label = ctk.CTkLabel(self, text="Dublador QC-PRO", font=ctk.CTkFont(size=24, weight="bold"))
        self.title_label.grid(row=0, column=0, padx=20, pady=(20, 5), sticky="w")
        
        self.desc_label = ctk.CTkLabel(self, text="Controle de Qualidade | Anti-Alucinação | Áudio Guia (6 Minutos)", text_color="gray")
        self.desc_label.grid(row=1, column=0, padx=20, pady=(0, 20), sticky="w")

        # 2. Frame de Inputs (Pastas)
        self.inputs_frame = ctk.CTkFrame(self)
        self.inputs_frame.grid(row=2, column=0, padx=20, pady=10, sticky="ew")
        self.inputs_frame.grid_columnconfigure(1, weight=1)

        # Origem
        ctk.CTkLabel(self.inputs_frame, text="Pasta Origem (EN):").grid(row=0, column=0, padx=10, pady=10, sticky="e")
        self.entry_in = ctk.CTkEntry(self.inputs_frame, placeholder_text="Caminho da pasta original...")
        self.entry_in.grid(row=0, column=1, padx=10, pady=10, sticky="ew")
        ctk.CTkButton(self.inputs_frame, text="Procurar", width=80, command=lambda: self.selecionar_pasta(self.entry_in)).grid(row=0, column=2, padx=10, pady=10)

        # Destino
        ctk.CTkLabel(self.inputs_frame, text="Pasta Destino (PT):").grid(row=1, column=0, padx=10, pady=10, sticky="e")
        self.entry_out = ctk.CTkEntry(self.inputs_frame, placeholder_text="Caminho onde os arquivos serão salvos...")
        self.entry_out.grid(row=1, column=1, padx=10, pady=10, sticky="ew")
        ctk.CTkButton(self.inputs_frame, text="Procurar", width=80, command=lambda: self.selecionar_pasta(self.entry_out)).grid(row=1, column=2, padx=10, pady=10)

        # Guia
        ctk.CTkLabel(self.inputs_frame, text="Áudio Guia (Opcional):").grid(row=2, column=0, padx=10, pady=10, sticky="e")
        self.entry_guia = ctk.CTkEntry(self.inputs_frame, placeholder_text="Selecione um arquivo de áudio de referência...")
        self.entry_guia.grid(row=2, column=1, padx=10, pady=10, sticky="ew")
        ctk.CTkButton(self.inputs_frame, text="Procurar", width=80, command=lambda: self.selecionar_arquivo(self.entry_guia)).grid(row=2, column=2, padx=10, pady=10)

        # 3. Controles (Progresso e Botão)
        self.controls_frame = ctk.CTkFrame(self, fg_color="transparent")
        self.controls_frame.grid(row=3, column=0, padx=20, pady=10, sticky="ew")
        self.controls_frame.grid_columnconfigure(0, weight=1)

        self.status_label = ctk.CTkLabel(self.controls_frame, text="Aguardando início...", font=ctk.CTkFont(weight="bold"))
        self.status_label.grid(row=0, column=0, pady=(0, 5), sticky="w")

        self.progress_bar = ctk.CTkProgressBar(self.controls_frame)
        self.progress_bar.grid(row=1, column=0, sticky="ew", pady=5)
        self.progress_bar.set(0)

        # Frame interno para agrupar os botões
        self.buttons_frame = ctk.CTkFrame(self.controls_frame, fg_color="transparent")
        self.buttons_frame.grid(row=2, column=0, pady=(10, 0), sticky="e")

        self.btn_cancelar = ctk.CTkButton(self.buttons_frame, text="🛑 Cancelar", font=ctk.CTkFont(weight="bold"), fg_color="#C62828", hover_color="#B71C1C", width=100, state="disabled", command=self.cancelar_processo)
        self.btn_cancelar.pack(side="left", padx=5)

        self.btn_pausar = ctk.CTkButton(self.buttons_frame, text="⏸️ Pausar", font=ctk.CTkFont(weight="bold"), fg_color="#F57C00", hover_color="#E65100", width=100, state="disabled", command=self.pausar_processo)
        self.btn_pausar.pack(side="left", padx=5)

        self.btn_iniciar = ctk.CTkButton(self.buttons_frame, text="🚀 Iniciar Dublagem", font=ctk.CTkFont(weight="bold"), width=160, command=self.iniciar_processo)
        self.btn_iniciar.pack(side="left", padx=5)

        # 4. Console de Log
        self.log_box = ctk.CTkTextbox(self, state="disabled", wrap="word", font=ctk.CTkFont(family="Consolas", size=12))
        self.log_box.grid(row=4, column=0, padx=20, pady=20, sticky="nsew")

        # Redirecionar prints
        sys.stdout = PrintRedirector(self.log_box)
        sys.stderr = PrintRedirector(self.log_box)

        print(f"🚀 ENGINE v10.2.1 FIX10 ATIVA: {device.upper()}")
        print("Pronto para iniciar. Selecione as pastas e clique em Iniciar.")

    def selecionar_pasta(self, entry_widget):
        pasta = filedialog.askdirectory()
        if pasta:
            entry_widget.delete(0, "end")
            entry_widget.insert(0, pasta)

    def selecionar_arquivo(self, entry_widget):
        arquivo = filedialog.askopenfilename(filetypes=[("Arquivos de Áudio", "*.wav *.mp3")])
        if arquivo:
            entry_widget.delete(0, "end")
            entry_widget.insert(0, arquivo)

    def atualizar_progresso(self, valor, desc="", modo="determinate"):
        # Aplica o modo indeterminado (animação quicando) caso solicitado
        self.progress_bar.configure(mode=modo)
        if modo == "indeterminate":
            self.progress_bar.start()
        else:
            self.progress_bar.stop()
            self.progress_bar.set(valor)
            
        if desc:
            self.status_label.configure(text=desc)

    def pausar_processo(self):
        self.app_state["pausar"] = not self.app_state["pausar"]
        if self.app_state["pausar"]:
            self.btn_pausar.configure(text="▶️ Retomar")
            self.status_label.configure(text="Em pausa...")
            print("\n⏸️ Sistema Pausado.")
        else:
            self.btn_pausar.configure(text="⏸️ Pausar")
            self.status_label.configure(text="Retomando...")
            print("▶️ Retomando processo...\n")

    def cancelar_processo(self):
        print("\n🛑 Sinal de cancelamento enviado. Aguardando a etapa atual finalizar...")
        self.status_label.configure(text="Cancelando... Aguarde o fim da iteração.")
        self.app_state["cancelar"] = True
        self.app_state["pausar"] = False # Destrava se estiver pausado
        self.btn_pausar.configure(state="disabled")
        self.btn_cancelar.configure(state="disabled")

    def thread_processamento(self, pasta_in, pasta_out, pasta_guia):
        try:
            resultado = processar_dublagem(pasta_in, pasta_out, pasta_guia, self.atualizar_progresso, self.app_state)
            print("\n" + "="*40)
            print(resultado)
            print("="*40 + "\n")
            if not self.app_state["cancelar"]:
                self.status_label.configure(text="Concluído com sucesso!")
        except Exception as e:
            print(f"\n❌ ERRO FATAL: {e}")
            self.status_label.configure(text="Erro durante a execução.")
        finally:
            # Reseta os botões ao final ou cancelamento
            self.btn_iniciar.configure(state="normal")
            self.btn_pausar.configure(state="disabled", text="⏸️ Pausar")
            self.btn_cancelar.configure(state="disabled")
            if not self.app_state["cancelar"]:
                self.progress_bar.set(1.0)

    def iniciar_processo(self):
        pasta_in = self.entry_in.get()
        pasta_out = self.entry_out.get()
        pasta_guia = self.entry_guia.get()

        if not pasta_in or not pasta_out:
            print("⚠️ Erro: As pastas de origem e destino são obrigatórias!")
            return

        # Zera estado
        self.app_state["cancelar"] = False
        self.app_state["pausar"] = False

        # Configura interface
        self.btn_iniciar.configure(state="disabled")
        self.btn_pausar.configure(state="normal", text="⏸️ Pausar")
        self.btn_cancelar.configure(state="normal")
        self.progress_bar.set(0)
        self.status_label.configure(text="Iniciando processamento...")
        print("\n" + "="*40 + "\n🚀 Iniciando Lote de Dublagem...")
        
        threading.Thread(target=self.thread_processamento, args=(pasta_in, pasta_out, pasta_guia), daemon=True).start()

class PrintRedirector:
    def __init__(self, textbox):
        self.textbox = textbox

    def write(self, text):
        self.textbox.configure(state="normal")
        self.textbox.insert("end", text)
        self.textbox.see("end")
        self.textbox.configure(state="disabled")

    def flush(self):
        pass


if __name__ == "__main__":
    app = DubladorApp()
    app.mainloop()