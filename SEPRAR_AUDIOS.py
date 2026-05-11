# v1.5.4 - Audio Purge: Root Scanner (Anti-Loop & PT-BR Force) - RTX 5070 Ti
# FIX: Detecção de Glitches de Loop (Palavras repetidas infinitamente).
# FIX: Motor de Expressões Regulares atualizado (adicionado 'eca').
import os
import torch
import whisper
import shutil
import gc
import threading
import tkinter as tk
from tkinter import ttk, filedialog, scrolledtext, messagebox
from pathlib import Path
import warnings

from _audio_quality_gate import (
    OUTROS_RUIDOS,
    PADROES_EXPRESSOES,
    eh_ruido,
    realizar_tri_checagem,
)

__all__ = [
    "AppScanner",
    "OUTROS_RUIDOS",
    "PADROES_EXPRESSOES",
    "eh_ruido",
    "realizar_tri_checagem",
]

warnings.filterwarnings("ignore")

# --- LÓGICA DE DETECÇÃO DE HARDWARE ---
def get_device_info():
    if torch.cuda.is_available():
        vram = torch.cuda.get_device_properties(0).total_memory / (1024**3)
        nome_gpu = torch.cuda.get_device_name(0)
        return "cuda", f"🚀 GPU ATIVA: {nome_gpu} ({vram:.1f}GB VRAM)"
    else:
        return "cpu", "⚠️ CPU ATIVA (Lento - Sem aceleração de GPU)"

device, device_msg = get_device_info()
models = {"whisper": None, "current_type": None}

class AppScanner:
    def __init__(self, root):
        self.root = root
        self.root.title("Audio Purge v1.5.4 - Anti-Loop PT-BR")
        self.root.geometry("850x680")
        
        self.pasta_in = tk.StringVar()
        self.pasta_out = tk.StringVar()
        self.metodo = tk.StringVar(value="Copiar")
        self.motor = tk.StringVar(value="base")
        self.idioma = tk.StringVar(value="pt")
        self.processando = False

        self.setup_ui()

    def setup_ui(self):
        main_frame = tk.Frame(self.root, padx=20, pady=20)
        main_frame.pack(fill="both", expand=True)

        lbl_hardware = tk.Label(main_frame, text=device_msg, font=("Arial", 10, "bold"), 
                                fg="#2ecc71" if device == "cuda" else "#e74c3c")
        lbl_hardware.grid(row=0, column=0, columnspan=2, pady=(0,10))

        # Pastas
        tk.Label(main_frame, text="Pasta de Origem:", font=("Arial", 9, "bold")).grid(row=1, column=0, sticky="w")
        tk.Entry(main_frame, textvariable=self.pasta_in, width=85).grid(row=2, column=0, padx=5)
        tk.Button(main_frame, text="Buscar", command=lambda: self.pasta_in.set(filedialog.askdirectory())).grid(row=2, column=1)

        tk.Label(main_frame, text="Pasta de Rejeitados (Grunhidos/Expressões):", font=("Arial", 9, "bold")).grid(row=3, column=0, sticky="w", pady=(10,0))
        tk.Entry(main_frame, textvariable=self.pasta_out, width=85).grid(row=4, column=0, padx=5)
        tk.Button(main_frame, text="Buscar", command=lambda: self.pasta_out.set(filedialog.askdirectory())).grid(row=4, column=1)

        # Configurações
        cfg_frame = tk.LabelFrame(main_frame, text=" Ajustes de Precisão ", pady=10)
        cfg_frame.grid(row=5, column=0, columnspan=2, sticky="ew", pady=15)

        tk.Label(cfg_frame, text="Motor:").pack(side="left", padx=5)
        for m in ["tiny", "base", "medium"]:
            tk.Radiobutton(cfg_frame, text=m.capitalize(), variable=self.motor, value=m).pack(side="left")

        tk.Label(cfg_frame, text=" | Idioma:").pack(side="left", padx=(10,0))
        ttk.Combobox(cfg_frame, textvariable=self.idioma, values=["pt", "en", "auto"], width=7, state="readonly").pack(side="left", padx=5)

        tk.Label(cfg_frame, text=" | Ação:").pack(side="left", padx=(10,0))
        tk.Radiobutton(cfg_frame, text="Copiar", variable=self.metodo, value="Copiar").pack(side="left")
        tk.Radiobutton(cfg_frame, text="Mover", variable=self.metodo, value="Mover").pack(side="left")

        self.progress = ttk.Progressbar(main_frame, orient="horizontal", length=750, mode="determinate")
        self.progress.grid(row=6, column=0, columnspan=2, pady=10)
        
        self.lbl_stats = tk.Label(main_frame, text="Pronto para iniciar.", font=("Arial", 10))
        self.lbl_stats.grid(row=7, column=0, columnspan=2)

        self.log_area = scrolledtext.ScrolledText(main_frame, width=110, height=14, font=("Consolas", 8), bg="#1e1e1e", fg="#d4d4d4")
        self.log_area.grid(row=8, column=0, columnspan=2, pady=10)

        self.btn_start = tk.Button(main_frame, text="🚀 INICIAR VARREDURA", bg="#2c3e50", fg="white", 
                                   font=("Arial", 11, "bold"), command=self.iniciar_thread, height=2)
        self.btn_start.grid(row=9, column=0, columnspan=2, sticky="ew")

    def log(self, mensagem):
        self.log_area.insert(tk.END, mensagem + "\n")
        self.log_area.see(tk.END)

    def iniciar_thread(self):
        if not self.pasta_in.get() or not self.pasta_out.get():
            messagebox.showwarning("Aviso", "Selecione as pastas!")
            return
        if self.processando: return
        self.processando = True
        self.btn_start.config(state="disabled", text="⚡ PROCESSANDO...")
        threading.Thread(target=self.processar, daemon=True).start()

    def processar(self):
        try:
            m_type = self.motor.get()
            self.log(f"📥 Carregando Whisper {m_type.upper()}...")
            if models["whisper"] is None or models["current_type"] != m_type:
                models["whisper"] = whisper.load_model(m_type, device=device)
                models["current_type"] = m_type
            
            p_in, p_out = Path(self.pasta_in.get()), Path(self.pasta_out.get())
            p_out.mkdir(parents=True, exist_ok=True)
            
            arquivos = []
            for ext in ['*.wav', '*.mp3', '*.ogg']:
                arquivos.extend(list(p_in.glob(ext.lower())) + list(p_in.glob(ext.upper())))
            
            arquivos = sorted(list(set(arquivos)))
            self.progress["maximum"] = len(arquivos)
            
            removidos, mantidos = 0, 0
            for i, arq in enumerate(arquivos):
                # Configuração para evitar "surdez"
                args = {"temperature": 0, "condition_on_previous_text": False}
                if self.idioma.get() != "auto": args["language"] = self.idioma.get()

                res = models["whisper"].transcribe(str(arq), **args)
                texto = res["text"].strip()
                valido, motivo = realizar_tri_checagem(texto)
                
                self.progress["value"] = i + 1
                txt_preview = (texto[:50] + "...") if len(texto) > 50 else texto
                if not txt_preview: txt_preview = "[Vazio]"

                if not valido:
                    removidos += 1
                    dest = p_out / arq.name
                    if self.metodo.get() == "Mover":
                        try: shutil.move(str(arq), str(dest))
                        except: shutil.copy2(str(arq), str(dest))
                    else:
                        shutil.copy2(str(arq), str(dest))
                    self.log(f"🚫 {arq.name} -> {motivo} | Ouviu: '{txt_preview}'")
                else:
                    mantidos += 1
                    if i % 5 == 0: self.log(f"🗣️ {arq.name} -> OK | Ouviu: '{txt_preview}'")

                self.lbl_stats.config(text=f"Fila: {i+1}/{len(arquivos)} | ✅ {mantidos} | 🚫 {removidos}")
                if (i+1) % 50 == 0:
                    gc.collect()
                    if device == "cuda": torch.cuda.empty_cache()

            self.log("\n🏁 FINALIZADO!")
            messagebox.showinfo("Sucesso", f"Concluído!\nMantidos: {mantidos}\nRejeitados: {removidos}")

        except Exception as e:
            self.log(f"❌ ERRO: {e}")
        finally:
            self.processando = False
            self.btn_start.config(state="normal", text="🚀 INICIAR VARREDURA")

if __name__ == "__main__":
    root = tk.Tk()
    app = AppScanner(root)
    root.mainloop()
