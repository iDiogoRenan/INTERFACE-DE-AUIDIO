import os
import sys
import gc
import json
import time
import shutil
import datetime
import threading
import numpy as np
import librosa
import soundfile as sf
import re
import difflib
from pathlib import Path
from pydub import AudioSegment
import traceback

# ── PATCH ANTI-SOTAQUE PT-BR ─────────────────────────────────────────────────
from _patch_validacao import ValidacaoWidget
from _patch_accent_fix import (
    SingleDubbingWorkerV14,
    GeradorPoolWorker,
    VOICE_PROFILES_PTBR,
    get_duracao_exata,
)

import pygame
from PyQt6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QVBoxLayout, QHBoxLayout, QTabWidget,
    QPushButton, QLabel, QFileDialog, QProgressBar, QMessageBox,
    QLineEdit, QGroupBox, QSplitter, QTreeWidget, QTreeWidgetItem,
    QTextEdit, QSlider, QListWidget, QListWidgetItem, QSpinBox, QDoubleSpinBox
)
from PyQt6.QtCore import Qt, QThread, pyqtSignal, QTimer
from PyQt6.QtGui import QFont, QColor, QTextCursor, QTextCharFormat

APP_VERSION = "14.1 (Anti-Sotaque + Anti-Corte Final)"

# ─────────────────────────────────────────────────────────────────────────────
# OMNI11 - LÓGICA BASE IMUTÁVEL
# ─────────────────────────────────────────────────────────────────────────────

def limpar_caminho(p):
    return p.strip().replace("\n", "").replace("\r", "").replace('"', '').replace("'", "")

def sincronizar_pontuacao(texto_base, texto_referencia):
    if not texto_base or not texto_referencia: return texto_base
    texto_base = str(texto_base).strip()
    texto_referencia = str(texto_referencia).strip()
    texto_limpo = re.sub(r'[\.\?\!\;\:\,\s\"\'\”\’]+$', '', texto_base)
    ref_final = re.sub(r'[\s\"\'\”\’]+$', '', texto_referencia)
    if ref_final.endswith('?'): return texto_limpo + '?'
    if ref_final.endswith('!'): return texto_limpo + '!'
    return texto_limpo + '.'

def validar_qualidade_zcr(audio_np):
    zcr = librosa.feature.zero_crossing_rate(audio_np)[0]
    avg_zcr = np.mean(zcr)
    if avg_zcr > 0.45: return False, avg_zcr
    return True, avg_zcr

def calcular_similaridade_texto(texto1, texto2):
    t1 = re.sub(r'[^\w\s]', '', str(texto1).lower().strip())
    t2 = re.sub(r'[^\w\s]', '', str(texto2).lower().strip())
    return difflib.SequenceMatcher(None, t1, t2).ratio()

def verificar_qualidade_fala_original(resultado_whisper):
    texto = resultado_whisper["text"].lower().strip('.!?,;:"\' ')
    if not texto: return False, "Áudio vazio ou sem voz detectada."
    segments = resultado_whisper.get("segments", [])
    if not segments: return False, "Whisper não encontrou segmentos de voz."
    no_speech_prob = np.mean([s.get("no_speech_prob", 0) for s in segments])
    avg_logprob = np.mean([s.get("avg_logprob", 0) for s in segments])
    if no_speech_prob > 0.65:
        return False, f"Alta probabilidade de ruído/grunhido (No Speech Prob: {no_speech_prob:.2f})."
    if avg_logprob < -1.2:
        return False, f"Voz confusa/distorcida (LogProb: {avg_logprob:.2f})."
    grunhidos = ["ah", "oh", "uh", "hmm", "hm", "huh", "ugh", "gasp", "sigh", "ha", "eh", "whoa", "argh", "grr", "wow", "mhm"]
    if texto in grunhidos or len(texto) <= 3:
        return False, f"Detectado apenas grunhido ('{texto}')."
    return True, "OK"

def corrigir_pronuncia_br(texto):
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

def sincronizar_master_v10_1(y_gen, caminho_saida, caminho_original, silence_pad_ms=200):
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
    if silence_pad_ms > 0:
        seg = seg + AudioSegment.silent(duration=int(silence_pad_ms))
    seg.export(caminho_saida, format="wav")

def get_audio_info(caminho: str) -> dict:
    try:
        y, sr = sf.read(caminho)
        return {"duration": len(y) / sr, "sr": sr, "channels": 1 if len(y.shape) == 1 else y.shape[1]}
    except:
        try:
            dur = librosa.get_duration(path=caminho)
            return {"duration": dur, "sr": librosa.get_samplerate(caminho), "channels": 1}
        except: return None

# ─────────────────────────────────────────────────────────────────────────────
# WORKERS
# ─────────────────────────────────────────────────────────────────────────────

class TranscribeWorker(QThread):
    log_signal = pyqtSignal(str, str)
    result_signal = pyqtSignal(str, str)
    error_signal = pyqtSignal(str)

    def __init__(self, path_en, models_ref, target_lang="pt", source_lang="auto", parent=None):
        super().__init__(parent)
        self.path_en = path_en
        self.models = models_ref
        self.target_lang = target_lang
        self.source_lang = source_lang

    def log(self, msg, level="info"): self.log_signal.emit(msg, level)

    def run(self):
        import torch
        from deep_translator import GoogleTranslator
        import whisper
        try:
            dev = "cuda" if torch.cuda.is_available() else "cpu"
            if self.models["whisper"] is None:
                self.log("📥 Carregando Whisper MEDIUM...", "info")
                self.models["whisper"] = whisper.load_model("medium", device=dev)
            
            self.log(f"🔍 Transcrevendo: {os.path.basename(self.path_en)}", "info")
            w_lang = self.source_lang if self.source_lang in ['en', 'pt', 'fr', 'sv'] else None
            res = self.models["whisper"].transcribe(self.path_en, language=w_lang, temperature=0.0)
            txt_en = res["text"].strip()
            self.log(f"📄 Origem: {txt_en}", "info")
            
            if self.source_lang == self.target_lang and txt_en:
                txt_target = txt_en
            else:
                txt_target = GoogleTranslator(source='auto', target=self.target_lang).translate(txt_en)
                
            if self.target_lang == "pt":
                txt_target_ia = corrigir_pronuncia_br(txt_target)
                txt_pt = sincronizar_pontuacao(txt_target_ia, txt_en)
            else:
                txt_pt = sincronizar_pontuacao(txt_target, txt_en)
                
            self.log(f"📄 Destino: {txt_pt}", "info")
            
            self.result_signal.emit(txt_en, txt_pt)
        except Exception as e:
            self.error_signal.emit(str(e))

class BatchTranscribeWorker(QThread):
    progress_signal = pyqtSignal(int, int, str, str, str) # current, total, nome, en_text, pt_text
    done_signal = pyqtSignal()
    
    def __init__(self, paths_en, pasta_cache, models_ref, target_lang="pt", source_lang="auto", parent=None):
        super().__init__(parent)
        self.paths_en = paths_en
        self.pasta_cache = pasta_cache
        self.models = models_ref
        self.target_lang = target_lang
        self.source_lang = source_lang
        
    def run(self):
        import torch
        from deep_translator import GoogleTranslator
        import whisper
        import json
        
        cache_file = os.path.join(self.pasta_cache, "transcricoes_cache.json")
        cache_data = {}
        if os.path.exists(cache_file):
            try:
                with open(cache_file, "r", encoding="utf-8") as f:
                    cache_data = json.load(f)
            except: pass
            
        dev = "cuda" if torch.cuda.is_available() else "cpu"
        total = len(self.paths_en)
        
        for i, p in enumerate(self.paths_en):
            nome = os.path.basename(p)
            if nome in cache_data:
                en = cache_data[nome].get("en", "")
                pt = cache_data[nome].get("pt", "")
                self.progress_signal.emit(i+1, total, nome, en, pt)
                continue
                
            if self.models["whisper"] is None:
                self.models["whisper"] = whisper.load_model("medium", device=dev)
                
            try:
                w_lang = self.source_lang if self.source_lang in ['en', 'pt', 'fr', 'sv'] else None
                res = self.models["whisper"].transcribe(p, language=w_lang, temperature=0.0)
                txt_en = res["text"].strip()
                
                if self.source_lang == self.target_lang and txt_en:
                    txt_target = txt_en
                else:
                    txt_target = GoogleTranslator(source='auto', target=self.target_lang).translate(txt_en)
                    
                if self.target_lang == "pt":
                    txt_target_ia = corrigir_pronuncia_br(txt_target)
                    txt_pt = sincronizar_pontuacao(txt_target_ia, txt_en)
                else:
                    txt_pt = sincronizar_pontuacao(txt_target, txt_en)
                
                cache_data[nome] = {"en": txt_en, "pt": txt_pt}
                self.progress_signal.emit(i+1, total, nome, txt_en, txt_pt)
                
                # Salvar a cada item para garantir persistência em caso de queda
                with open(cache_file, "w", encoding="utf-8") as f:
                    json.dump(cache_data, f, ensure_ascii=False, indent=2)
            except Exception as e:
                print(f"Erro em lote ({nome}): {e}")
                self.progress_signal.emit(i+1, total, nome, "", "")
                
        self.done_signal.emit()


# SingleDubbingWorker substituído pelo patch anti-sotaque
SingleDubbingWorker = SingleDubbingWorkerV14


# ─────────────────────────────────────────────────────────────────────────────
# PYGAME AUDIO PLAYER
# ─────────────────────────────────────────────────────────────────────────────
if not pygame.mixer.get_init(): pygame.mixer.init()

class AudioPlayer(QWidget):
    active_player = None

    def __init__(self, label_text="Áudio", parent=None):
        super().__init__(parent)
        self._path = ""
        self._is_playing = False
        self._is_paused = False
        self._dur = 0.0
        self._seek = 0.0
        self._start_ticks = 0
        self._vol = 0.8
        
        self._timer = QTimer(self)
        self._timer.setInterval(100)
        self._timer.timeout.connect(self._update_time)

        self._build_ui(label_text)

    def _build_ui(self, label_text):
        main = QVBoxLayout(self)
        main.setSpacing(4)
        main.setContentsMargins(0, 0, 0, 0)
        
        hdr = QHBoxLayout()
        lbl = QLabel(label_text)
        lbl.setStyleSheet("color: #58a6ff; font-weight: bold; font-size: 12px;")
        hdr.addWidget(lbl)
        self.lbl_info = QLabel("Nenhum arquivo")
        self.lbl_info.setStyleSheet("color: #8b949e; font-size: 11px;")
        hdr.addWidget(self.lbl_info, 1)
        main.addLayout(hdr)

        ctrl = QHBoxLayout()
        self.btn_play = QPushButton("▶")
        self.btn_play.clicked.connect(self.toggle_play)
        self.btn_play.setEnabled(False)
        ctrl.addWidget(self.btn_play)

        self.slider = QSlider(Qt.Orientation.Horizontal)
        self.slider.setRange(0, 1000)
        self.slider.sliderReleased.connect(self._do_seek)
        self.slider.sliderPressed.connect(self._press_seek)
        ctrl.addWidget(self.slider, 1)

        self.lbl_time = QLabel("0:00 / 0:00")
        self.lbl_time.setStyleSheet("color: #8b949e; font-size: 11px; min-width: 90px;")
        ctrl.addWidget(self.lbl_time)
        main.addLayout(ctrl)

    def load(self, path):
        if not path or not os.path.exists(path): return
        if AudioPlayer.active_player is self: self.stop()
        self._path = path
        self.btn_play.setEnabled(True)
        self._seek = 0.0
        self.slider.setValue(0)
        info = get_audio_info(path)
        self._dur = info.get("duration", 0) if info else 0.0
        self.lbl_info.setText(f"{os.path.basename(path)} | {self._dur:.1f}s")
        self._update_lbl(0)

    def clear(self, msg="Nenhum arquivo"):
        if AudioPlayer.active_player is self: self.stop()
        self._path = ""
        self.btn_play.setEnabled(False)
        self._seek = 0.0
        self.slider.setValue(0)
        self._dur = 0.0
        self.lbl_info.setText(msg)
        self._update_lbl(0)

    def toggle_play(self):
        if not self._path: return
        self.pause() if self._is_playing else self.play()

    def play(self):
        if AudioPlayer.active_player and AudioPlayer.active_player is not self:
            AudioPlayer.active_player.stop()
        AudioPlayer.active_player = self
        pygame.mixer.music.set_volume(self._vol)
        if self._is_paused:
            pygame.mixer.music.unpause()
        else:
            try:
                pygame.mixer.music.load(self._path)
                pygame.mixer.music.play(start=self._seek)
            except Exception as e: print(e); return
            
        self._start_ticks = pygame.time.get_ticks() - int(self._seek * 1000)
        self._is_playing = True
        self._is_paused = False
        self.btn_play.setText("⏸")
        self._timer.start()

    def pause(self):
        if self._is_playing:
            pygame.mixer.music.pause()
            self._is_playing = False
            self._is_paused = True
            self.btn_play.setText("▶")
            self._timer.stop()

    def stop(self):
        if AudioPlayer.active_player is self:
            pygame.mixer.music.stop()
            pygame.mixer.music.unload()
            AudioPlayer.active_player = None
        self._is_playing = self._is_paused = False
        self.btn_play.setText("▶")
        self._timer.stop()
        self._seek = 0.0
        self.slider.setValue(0)
        self._update_lbl(0)

    def _press_seek(self):
        self._was_playing = self._is_playing
        if self._is_playing: self.pause()

    def _do_seek(self):
        self._seek = (self.slider.value() / 1000.0) * self._dur
        self._is_paused = False
        self._update_lbl(self._seek)
        if getattr(self, '_was_playing', False): self.play()

    def _update_time(self):
        if not self._is_playing: return
        import pygame
        if not self._is_paused and not pygame.mixer.music.get_busy():
            self.stop()
            return
        curr = (pygame.time.get_ticks() - self._start_ticks) / 1000.0
        if curr >= self._dur and self._dur > 0: curr = self._dur
        self.slider.blockSignals(True)
        self.slider.setValue(int((curr / max(self._dur, 0.1)) * 1000))
        self.slider.blockSignals(False)
        self._update_lbl(curr)
        self._seek = curr

    def _update_lbl(self, pos):
        m, s = int(pos//60), int(pos%60)
        dm, ds = int(self._dur//60), int(self._dur%60)
        self.lbl_time.setText(f"{m}:{s:02d} / {dm}:{ds:02d}")

# ─────────────────────────────────────────────────────────────────────────────
# UI COMPONENTS
# ─────────────────────────────────────────────────────────────────────────────
class FileExplorer(QWidget):
    file_selected = pyqtSignal(str)
    dub_request = pyqtSignal(list)
    extract_request = pyqtSignal(str)
    
    def __init__(self, parent=None):
        super().__init__(parent)
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0,0,0,0)
        lbl = QLabel("📁 Explorador do Projeto")
        lbl.setStyleSheet("color:#58a6ff; font-weight:bold;")
        layout.addWidget(lbl)
        self.tree = QTreeWidget()
        self.tree.setHeaderLabels(["Arquivo", "Status"])
        self.tree.setColumnWidth(0, 160)
        
        # Permitir arrastar e selecionar multiplos
        self.tree.setSelectionMode(QTreeWidget.SelectionMode.ExtendedSelection)
        self.tree.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self.tree.customContextMenuRequested.connect(self._show_menu)
        
        self.tree.itemClicked.connect(self._on_click)
        layout.addWidget(self.tree)
        self._in = ""
        self._out = ""
        self._status = {}
        
    def _show_menu(self, pos):
        items = self.tree.selectedItems()
        if not items: return
        from PyQt6.QtWidgets import QMenu
        menu = QMenu(self)
        menu.setStyleSheet("QMenu { background-color: #161b22; color: #c9d1d9; border: 1px solid #30363d; } QMenu::item:selected { background-color: #1f6feb; }")
        
        paths = [it.data(0, Qt.ItemDataRole.UserRole) for it in items if it.data(0, Qt.ItemDataRole.UserRole)]
        if not paths: return
        
        if len(paths) == 1:
            ac_play = menu.addAction("▶ Ouvir Original")
            ac_ext = menu.addAction("📝 Extrair Transcrição (Para Edição)")
            ac_dub = menu.addAction("🎙️ Dublar Só Este (Teste)")
        else:
            ac_play = None
            ac_ext = None
            ac_dub = menu.addAction(f"🎙️ Dublar {len(paths)} Selecionados (Fila de Teste)")
            
        action = menu.exec(self.tree.viewport().mapToGlobal(pos))
        if action:
            if action == ac_play: self.file_selected.emit(paths[0])
            elif action == ac_ext:
                self.file_selected.emit(paths[0])
            elif action == ac_dub:
                self.dub_request.emit(paths)

    def set_folders(self, f_in, f_out):
        self._in, self._out = f_in, f_out
        self.refresh()

    def _has_dubbed_output(self, name):
        if not self._out or not os.path.isdir(self._out):
            return False
        return os.path.isfile(os.path.join(self._out, name))

    def refresh(self):
        self.tree.clear()
        if self._in and os.path.isdir(self._in):
            root = QTreeWidgetItem(self.tree, [os.path.basename(self._in), "Origem"])
            root.setForeground(0, QColor("#58a6ff"))
            try:
                for f in sorted(os.listdir(self._in)):
                    if f.lower().endswith((".wav", ".mp3", ".wem", ".ogg", ".flac")):
                        st = self._status.get(f)
                        if not st and self._has_dubbed_output(f):
                            st = "✅ Pronto"
                        if not st:
                            st = "Pendente"
                        color = "#3fb950" if "✅" in st else "#f85149" if "❌" in st else "#8b949e"
                        it = QTreeWidgetItem(root, [f, st])
                        it.setForeground(1, QColor(color))
                        it.setData(0, Qt.ItemDataRole.UserRole, os.path.join(self._in, f))
                        if "✅" in st:
                            it.setToolTip(1, "Já existe dublagem na pasta Destino.")
                root.setExpanded(True)
            except Exception as e:
                print(f"Erro ao ler diretório: {e}")

    def update_status(self, name, ok, motivo):
        self._status[name] = "✅ Pronto" if ok else f"❌ {motivo[:25]}"
        self.refresh()

    def _on_click(self, item):
        path = item.data(0, Qt.ItemDataRole.UserRole)
        if path and os.path.isfile(path): self.file_selected.emit(path)

# ─────────────────────────────────────────────────────────────────────────────
# MAIN
# ─────────────────────────────────────────────────────────────────────────────
DARK_STYLE = """
QMainWindow, QDialog, QWidget { background-color: #0d1117; color: #c9d1d9; font-family: 'Segoe UI', Arial; }
QGroupBox { font-weight: bold; border: 1px solid #30363d; border-radius: 6px; margin-top: 10px; padding-top: 15px; }
QGroupBox::title { subcontrol-origin: margin; left: 10px; padding: 0 3px 0 3px; color: #8b949e; }
QPushButton { background-color: #21262d; border: 1px solid #30363d; border-radius: 6px; color: #c9d1d9; padding: 6px 12px; }
QPushButton:hover { background-color: #30363d; border-color: #8b949e; }
QPushButton:disabled { color: #484f58; border-color: #21262d; background-color: #0d1117; }
QLineEdit, QTextEdit { background-color: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 6px; color: #c9d1d9; }
QLineEdit:focus, QTextEdit:focus { border: 1px solid #58a6ff; }
QProgressBar { border: 1px solid #30363d; border-radius: 4px; background-color: #161b22; text-align: center; color: white; }
QProgressBar::chunk { background-color: #1f6feb; border-radius: 3px; }
QTreeWidget, QListWidget { background: #161b22; border: 1px solid #30363d; border-radius: 6px; }
QTreeWidget::item:selected, QListWidget::item:selected { background: #1f6feb; }
QSpinBox, QDoubleSpinBox { background: #161b22; border: 1px solid #30363d; border-radius: 4px; padding: 4px; color: white; }
"""

class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self._pool_dir = os.path.join(os.path.dirname(__file__), "voice_pool_ptbr")
        self.models = {"whisper": None, "omni": None}
        self.worker = None
        self._trans_worker = None
        self._val_trans_worker = None
        self._batch_trans_worker = None
        self._val_worker = None
        self.pool_worker = None
        self._qt_workers = set()
        self._pending_dub_paths = None
        self._cfg_file = "config_pratico.json"
        self._load_cfg()
        
        self.setWindowTitle(f"🎙️ Dublador Master Pro v{APP_VERSION} (Focado e Ágil)")
        self.resize(1300, 750)
        self.setStyleSheet(DARK_STYLE)
        
        self._current_file = ""
        self._current_result = ""
        self._session_dubbed_paths = {}
        
        # Limpar temporários antigos ao abrir
        temp_dir = os.path.join(os.path.dirname(__file__), "_temp_dublagem")
        if os.path.exists(temp_dir):
            try: shutil.rmtree(temp_dir)
            except: pass
        os.makedirs(temp_dir, exist_ok=True)

        self._build_ui()
        
        # Restaurar ultimas pastas
        if os.path.isdir(self.cfg.get("in", "")): self.lne_in.setText(self.cfg["in"])
        if os.path.isdir(self.cfg.get("out", "")): self.lne_out.setText(self.cfg["out"])
        if os.path.exists(self.cfg.get("guide", "")): self.lne_guide.setText(self.cfg["guide"])
        self.spn_temp.setValue(self.cfg.get("omni_temp", 0.0))
        self.spn_pad.setValue(self.cfg.get("pad_ms", 200))
        
        # Conectar mudancas para salvar config instantaneamente
        self.lne_in.textChanged.connect(self._save_cfg)
        self.lne_out.textChanged.connect(self._save_cfg)
        self.spn_temp.valueChanged.connect(self._save_cfg)
        self.spn_pad.valueChanged.connect(self._save_cfg)
        
        # Forcar atualizacao do explorador
        self.exp.set_folders(self.lne_in.text(), self.lne_out.text())
        
        # Conectar sinais novos do explorador
        self.exp.dub_request.connect(self._run_dub_multi)
        self.exp.extract_request.connect(lambda p: self._extract())

    def _load_cfg(self):
        self.cfg = {"in": "", "out": "", "guide": "", "omni_temp": 0.0, "pad_ms": 200}
        if os.path.exists(self._cfg_file):
            try: self.cfg = json.load(open(self._cfg_file, "r"))
            except: pass

    def _save_cfg(self):
        self.cfg["in"] = self.lne_in.text()
        self.cfg["out"] = self.lne_out.text()
        self.cfg["guide"] = self.lne_guide.text()
        self.cfg["omni_temp"] = self.spn_temp.value()
        self.cfg["pad_ms"] = self.spn_pad.value()
        json.dump(self.cfg, open(self._cfg_file, "w"))

    def _track_worker(self, worker, attr_name=None):
        if attr_name:
            setattr(self, attr_name, worker)
        self._qt_workers.add(worker)

        def _cleanup():
            self._qt_workers.discard(worker)
            if attr_name and getattr(self, attr_name, None) is worker:
                setattr(self, attr_name, None)
            worker.deleteLater()

        worker.finished.connect(_cleanup)
        return worker

    def _is_worker_running(self, worker):
        return worker is not None and worker.isRunning()

    def _stop_worker(self, worker, nome="processo", timeout_ms=3000):
        if not self._is_worker_running(worker):
            return
        worker.requestInterruption()
        if not worker.wait(timeout_ms):
            try:
                self._log(f"⚠️ {nome} não respondeu ao cancelamento; forçando parada.", "warning")
            except Exception:
                pass
            worker.terminate()
            worker.wait(2000)

    def _build_ui(self):
        cen = QWidget()
        self.setCentralWidget(cen)
        root_layout = QHBoxLayout(cen)

        # ESQUERDA: Explorador (sempre visível)
        self.exp = FileExplorer()
        root_layout.addWidget(self.exp, 1)

        # ABAS: Dublagem | Validação
        self.tabs = QTabWidget()
        self.tabs.setStyleSheet(
            "QTabWidget::pane { border:1px solid #30363d; }"
            "QTabBar::tab { background:#161b22; color:#8b949e; padding:8px 18px; }"
            "QTabBar::tab:selected { background:#0d1117; color:#c9d1d9; font-weight:bold; border-bottom:2px solid #1f6feb; }"
        )

        # Tab 1: Dublagem
        tab_dub = QWidget()
        layout = QHBoxLayout(tab_dub)
        self.tabs.addTab(tab_dub, "🎙️  Dublagem")

        # Tab 2: Validação
        self.tab_val = ValidacaoWidget()
        self.tab_val.redub_request.connect(self._redublar_de_validacao)
        self.tab_val.transcribe_request.connect(self._transcrever_de_validacao)
        self.tab_val.batch_transcribe_request.connect(self._transcrever_lote_validacao)
        # Injetar players dedicados (AudioPlayerWidget ja definido neste modulo)
        self._val_play_en = AudioPlayer("🎵 Original EN")
        self._val_play_pt = AudioPlayer("🔊 Dublado PT")
        self.tab_val.inject_players(self._val_play_en, self._val_play_pt)
        self.tabs.addTab(self.tab_val, "✅  Validação Manual")

        root_layout.addWidget(self.tabs, 5)

        # CENTRO: Processamento
        mid = QVBoxLayout()
        
        # 1. Pastas
        grp_f = QGroupBox("📂 Pastas de Trabalho")
        fl = QVBoxLayout(grp_f)
        self.lne_in = self._add_path_row(fl, "Origem (EN):", False)
        self.lne_out = self._add_path_row(fl, "Destino (PT):", False)
        self.lne_guide = self._add_path_row(fl, "Áudio Guia (Opcional):", True)
        
        self.lne_in.textChanged.connect(lambda: self.exp.set_folders(self.lne_in.text(), self.lne_out.text()))
        self.lne_out.textChanged.connect(lambda: self.exp.set_folders(self.lne_in.text(), self.lne_out.text()))
        mid.addWidget(grp_f)

        # 2. Players
        grp_p = QGroupBox("🎵 Áudios")
        pl = QVBoxLayout(grp_p)
        self.play_en = AudioPlayer("🇺🇸 Original")
        self.play_pt = AudioPlayer("🇧🇷 Resultado")
        pl.addWidget(self.play_en)
        pl.addWidget(self.play_pt)
        mid.addWidget(grp_p)

        # 3. Textos
        grp_t = QGroupBox("📝 Transcrição & Edição")
        tl = QVBoxLayout(grp_t)
        
        self.btn_trans = QPushButton("📝 Extrair Transcrição Apenas (Para Edição)")
        self.btn_trans.setStyleSheet("QPushButton{background:#1f6feb; font-weight:bold;}")
        self.btn_trans.clicked.connect(self._extract)
        tl.addWidget(self.btn_trans)
        
        self.txt_en = QLineEdit(); self.txt_en.setPlaceholderText("🇺🇸 EN Original...")
        self.txt_pt = QLineEdit(); self.txt_pt.setPlaceholderText("🇧🇷 PT Editável...")
        tl.addWidget(self.txt_en)
        tl.addWidget(self.txt_pt)
        
        self.btn_redub = QPushButton("🔄 Redublar Forçando Texto Acima")
        self.btn_redub.setStyleSheet("background:#2ea043; color:white; font-size:13px; font-weight:bold; padding:8px;")
        self.btn_redub.clicked.connect(self._run_dub)
        tl.addWidget(self.btn_redub)
        mid.addWidget(grp_t)

        # 4. Ações
        grp_act = QGroupBox("🚀 Ações de Dublagem")
        al = QVBoxLayout(grp_act)

        # Botão principal DUBLAR TUDO
        self.btn_tudo = QPushButton("🚀  DUBLAR TUDO  (Auto-Salvar na Pasta Destino)")
        self.btn_tudo.setStyleSheet(
            "background:#1f6feb; color:white; font-size:15px; font-weight:bold;"
            " padding:14px; border-radius:6px;"
        )
        self.btn_tudo.setToolTip("Dubla TODOS os arquivos da pasta Origem e salva automaticamente na pasta Destino.")
        self.btn_tudo.clicked.connect(self._run_tudo)
        al.addWidget(self.btn_tudo)

        row_b = QHBoxLayout()
        self.btn_dub = QPushButton("🎙️ Dublar Arquivo Atual")
        self.btn_dub.setStyleSheet("background:#5a3e00; color:#f0883e; font-size:13px; font-weight:bold; padding:8px;")
        self.btn_dub.clicked.connect(self._run_dub)
        row_b.addWidget(self.btn_dub)

        self.btn_save = QPushButton("💾 Salvar na Pasta Destino")
        self.btn_save.setStyleSheet("background:#238636; font-size:13px; font-weight:bold; padding:8px;")
        self.btn_save.clicked.connect(self._save_result)
        row_b.addWidget(self.btn_save)

        self.btn_cancelar = QPushButton("⏹ Cancelar")
        self.btn_cancelar.setStyleSheet("background:#6e1a1a; color:#ff9090; font-size:13px; font-weight:bold; padding:8px;")
        self.btn_cancelar.setEnabled(False)
        self.btn_cancelar.clicked.connect(self._cancelar)
        row_b.addWidget(self.btn_cancelar)
        al.addLayout(row_b)

        self.lbl_status = QLabel("Selecione um arquivo ou clique em DUBLAR TUDO.")
        self.lbl_status.setAlignment(Qt.AlignmentFlag.AlignCenter)
        al.addWidget(self.lbl_status)

        self.prog_bar = QProgressBar()
        self.prog_bar.setRange(0, 100)
        self.prog_bar.setValue(0)
        self.prog_bar.setTextVisible(True)
        self.prog_bar.setFixedHeight(14)
        self.prog_bar.setStyleSheet("QProgressBar { font-size: 9px; font-weight: bold; }")
        al.addWidget(self.prog_bar)

        mid.addWidget(grp_act)

        layout.addLayout(mid, 2)

        # DIREITA: Ajustes Finos e Log
        right = QVBoxLayout()
        
        grp_cfg = QGroupBox("⚙️ Ajustes Finos")
        cl = QVBoxLayout(grp_cfg)

        # ── Idioma Origem/Destino ─────────────────────────────────────────────
        from PyQt6.QtWidgets import QComboBox, QCheckBox
        row_lang = QHBoxLayout()
        self.cmb_source_lang = QComboBox()
        self.cmb_source_lang.addItems(["auto (Detectar)", "en (Inglês)", "fr (Francês)", "sv (Sueco)", "pt (Português)"])
        self.cmb_source_lang.setStyleSheet("QComboBox { background:#161b22; color:#c9d1d9; border:1px solid #30363d; padding:4px; border-radius:4px; }")
        
        self.cmb_target_lang = QComboBox()
        self.cmb_target_lang.addItems(["pt (Português)", "fr (Francês)", "sv (Sueco)", "en (Inglês)"])
        self.cmb_target_lang.setStyleSheet("QComboBox { background:#161b22; color:#c9d1d9; border:1px solid #30363d; padding:4px; border-radius:4px; }")
        
        row_lang.addWidget(QLabel("De:"))
        row_lang.addWidget(self.cmb_source_lang, 1)
        row_lang.addWidget(QLabel("Para:"))
        row_lang.addWidget(self.cmb_target_lang, 1)
        cl.addLayout(row_lang)

        # ── Modo de Voz ───────────────────────────────────────────────────────
        lbl_modo = QLabel("🎙️ Modo de Voz:")
        lbl_modo.setToolTip(
            "instruct: Voice Design puro (sem sotaque, mais rápido)\n"
            "pool: Clona vozes pré-geradas (mais natural, requer gerar pool 1x)"
        )
        cl.addWidget(lbl_modo)
        self.cmb_modo = QComboBox()
        self.cmb_modo.addItems([
            "classico (Voz Perfeita — igual ao original)",
            "antisotaque (Voz Original + Fonética)",
        ])
        self.cmb_modo.setEnabled(True)
        self.cmb_modo.setStyleSheet("QComboBox { background:#161b22; color:#c9d1d9; border:1px solid #30363d; padding:4px; border-radius:4px; }")
        cl.addWidget(self.cmb_modo)

        from PyQt6.QtWidgets import QCheckBox
        self.chk_palatalizar = QCheckBox("Palatização PT-BR (ti→tchi, de→dche)")
        self.chk_palatalizar.setChecked(False)
        self.chk_palatalizar.setToolTip("Quando ativo, palavras terminadas em ti/te/di/de viram tchi/tche/dchi/dche.")
        self.chk_palatalizar.setStyleSheet("QCheckBox { color:#c9d1d9; } QCheckBox::indicator { width:16px; height:16px; }")
        cl.addWidget(self.chk_palatalizar)

        self.chk_virgula = QCheckBox("Vírgula antes de ? (pausa em perguntas)")
        self.chk_virgula.setChecked(False)
        self.chk_virgula.setToolTip("Adiciona ', ?' antes de interrogações para pausar.")
        self.chk_virgula.setStyleSheet("QCheckBox { color:#c9d1d9; } QCheckBox::indicator { width:16px; height:16px; }")
        cl.addWidget(self.chk_virgula)

        self.chk_trailing = QCheckBox("Ponto final extra — evita corte (antisotaque)")
        self.chk_trailing.setChecked(False)
        self.chk_trailing.setToolTip("Adiciona ' .' ao final do texto no modo antisotaque.\nEvita que o modelo corte a última palavra. Deixe desligado se aparecerem pontos duplos.")
        self.chk_trailing.setStyleSheet("QCheckBox { color:#c9d1d9; } QCheckBox::indicator { width:16px; height:16px; }")
        cl.addWidget(self.chk_trailing)

        self.btn_pool = QPushButton("⚡ Gerar Pool de Vozes PT-BR (1x)")
        self.btn_pool.setStyleSheet("background:#0d419d; color:white; font-weight:bold; padding:6px;")
        self.btn_pool.setToolTip("Gera 6 vozes PT-BR nativas. Necessário apenas para o modo 'pool'. Demora ~2 min.")
        self.btn_pool.clicked.connect(self._gerar_pool)
        cl.addWidget(self.btn_pool)

        cl.addSpacing(8)

        lbl_pad = QLabel("Tempo de Silêncio Final (ms):")
        lbl_pad.setToolTip("Aumente se o áudio original tiver espaço vazio no final.")
        cl.addWidget(lbl_pad)

        row_pad = QHBoxLayout()
        self.sld_pad = QSlider(Qt.Orientation.Horizontal)
        self.sld_pad.setRange(0, 2000)
        self.spn_pad = QSpinBox()
        self.spn_pad.setRange(0, 2000); self.spn_pad.setSingleStep(50); self.spn_pad.setValue(200)
        self.sld_pad.valueChanged.connect(self.spn_pad.setValue)
        self.spn_pad.valueChanged.connect(self.sld_pad.setValue)
        row_pad.addWidget(self.sld_pad, 1)
        row_pad.addWidget(self.spn_pad)
        cl.addLayout(row_pad)

        # Manter spn_temp para compatibilidade (oculto mas usado internamente)
        self.spn_temp = QDoubleSpinBox()
        self.spn_temp.setRange(0.0, 1.0); self.spn_temp.setValue(0.0)
        self.spn_temp.setVisible(False)
        cl.addWidget(self.spn_temp)

        cl.addStretch()
        right.addWidget(grp_cfg, 1)

        grp_log = QGroupBox("📋 Log")
        log_l = QVBoxLayout(grp_log)
        self.log_box = QTextEdit()
        self.log_box.setReadOnly(True)
        self.log_box.setFont(QFont("Consolas", 10))
        log_l.addWidget(self.log_box)
        right.addWidget(grp_log, 2)

        layout.addLayout(right, 1)

        self.exp.file_selected.connect(self._select_file)
        self.tabs.currentChanged.connect(self._on_tab_mudou)

    def _add_path_row(self, layout, text, is_file):
        r = QHBoxLayout()
        r.addWidget(QLabel(text))
        l = QLineEdit()
        r.addWidget(l, 1)
        b = QPushButton("📁")
        b.setMaximumWidth(35)
        if is_file: b.clicked.connect(lambda: self._br_file(l))
        else: b.clicked.connect(lambda: self._br_folder(l))
        r.addWidget(b)
        layout.addLayout(r)
        return l

    def _br_folder(self, l):
        f = QFileDialog.getExistingDirectory(self)
        if f: l.setText(f)
    def _br_file(self, l):
        f, _ = QFileDialog.getOpenFileName(self, filter="Áudio (*.wav *.mp3)")
        if f: l.setText(f)

    def _log(self, msg, level="info"):
        c = {"info": "#e6edf3", "success": "#3fb950", "warning": "#d29922", "error": "#f85149"}.get(level, "#fff")
        cur = self.log_box.textCursor()
        cur.movePosition(QTextCursor.MoveOperation.End)
        fmt = QTextCharFormat(); fmt.setForeground(QColor(c))
        cur.setCharFormat(fmt)
        cur.insertText(f"[{datetime.datetime.now().strftime('%H:%M:%S')}] {msg}\n")
        self.log_box.setTextCursor(cur)
        self.log_box.ensureCursorVisible()

    # --- Core Logic ---
    def _select_file(self, path, auto_extract=True):
        self._current_file = path
        self._current_result = ""
        self.play_en.load(path)
        
        nome = os.path.basename(path)
        out_path = os.path.join(self.lne_out.text().strip(), nome)
        if os.path.exists(out_path):
            self.play_pt.load(out_path)
        elif nome in self._session_dubbed_paths and os.path.exists(self._session_dubbed_paths[nome]):
            self._current_result = self._session_dubbed_paths[nome]
            self.play_pt.load(self._current_result)
        else:
            self.play_pt.clear("Nenhum resultado")
            self.play_pt.stop()
            
        self.txt_en.clear()
        self.txt_pt.clear()
        self.lbl_status.setText(f"Selecionado: {nome}")
        self.lbl_status.setStyleSheet("color:#58a6ff;")
        
        # Tentar carregar do cache se já existir
        cache_file = os.path.join(self.lne_out.text().strip(), "transcricoes_cache.json")
        carregado_do_cache = False
        if os.path.exists(cache_file):
            try:
                import json
                with open(cache_file, "r", encoding="utf-8") as f:
                    cache_data = json.load(f)
                if nome in cache_data:
                    self.txt_en.setText(cache_data[nome].get("en", ""))
                    self.txt_pt.setText(cache_data[nome].get("pt", ""))
                    carregado_do_cache = True
            except: pass
            
        if auto_extract and not carregado_do_cache:
            self._extract()

    def _extract(self):
        if not self._current_file: return QMessageBox.warning(self, "Aviso", "Selecione um arquivo no explorador primeiro.")
        if self._is_worker_running(self._trans_worker):
            self._log("Transcrição anterior ainda em andamento; aguarde ela terminar.", "warning")
            return
        self.btn_trans.setEnabled(False)
        self.btn_redub.setEnabled(False)
        self.lbl_status.setText("⏳ Extraindo transcrição...")
        
        s_lang = self.cmb_source_lang.currentText().split(' ')[0]
        t_lang = self.cmb_target_lang.currentText().split(' ')[0]
        
        worker = self._track_worker(
            TranscribeWorker(self._current_file, self.models, target_lang=t_lang, source_lang=s_lang),
            "_trans_worker",
        )
        worker.log_signal.connect(self._log)
        worker.result_signal.connect(self._on_extracted)
        worker.finished.connect(self._on_extract_finished)
        worker.start()

    def _on_extracted(self, en, pt):
        self.txt_en.setText(en)
        self.txt_pt.setText(pt)
        self.lbl_status.setText("✅ Transcrição extraída. Edite e clique em Dublar.")

    def _on_extract_finished(self):
        self.btn_trans.setEnabled(True)
        self.btn_redub.setEnabled(True)
        pending = self._pending_dub_paths
        self._pending_dub_paths = None
        if pending:
            self._log("Transcrição concluída; iniciando dublagem solicitada.", "info")
            self.lbl_status.setText("Transcrição concluída. Iniciando dublagem...")
            QTimer.singleShot(0, lambda paths=pending: self._run_dub_multi(paths))

    def _on_tab_mudou(self, idx):
        """Ao entrar na aba Validação, pré-preenche EN+PT automaticamente."""
        if idx == 1:
            self.tab_val.definir_pastas(
                pasta_en=self.lne_in.text().strip(),
                pasta_dublados=self.lne_out.text().strip()
            )

    def _transcrever_de_validacao(self, src: str, target_lang: str, source_lang: str):
        if self._is_worker_running(self._val_trans_worker):
            self._log("[VAL] Transcrição anterior ainda em andamento.", "warning")
            return
        worker = self._track_worker(
            TranscribeWorker(src, self.models, target_lang=target_lang, source_lang=source_lang),
            "_val_trans_worker",
        )
        worker.log_signal.connect(lambda msg, lv: self._log(f"[VAL] {msg}", lv))
        worker.result_signal.connect(self.tab_val.on_transcribe_pronto)
        worker.start()

    def _transcrever_lote_validacao(self, paths_en: list, pasta_cache: str, target_lang: str, source_lang: str):
        if self._is_worker_running(self._batch_trans_worker):
            self._log("[VAL] Transcrição em lote já está rodando.", "warning")
            return
        worker = self._track_worker(
            BatchTranscribeWorker(paths_en, pasta_cache, self.models, target_lang=target_lang, source_lang=source_lang),
            "_batch_trans_worker",
        )
        worker.progress_signal.connect(self.tab_val.on_batch_progress)
        worker.done_signal.connect(self.tab_val.on_batch_done)
        worker.start()

    def _redublar_de_validacao(self, src: str, texto_pt: str, modo: str,
                               palatalizar: bool, virgula: bool, trailing: bool, pad_ms: int,
                               target_lang: str, source_lang: str):
        """Recebe sinal da aba Validação e cria worker dedicado de redublagem."""
        from _patch_accent_fix import SingleDubbingWorkerV14
        if self._is_worker_running(self._val_worker):
            self._log("[VAL] Redublagem anterior ainda em andamento.", "warning")
            return
        worker = self._track_worker(SingleDubbingWorkerV14(
            paths_en=[src],
            pasta_guia="",
            models_ref=self.models,
            custom_texts={"pt": texto_pt} if texto_pt else {},
            omni_temp=0.05,
            pad_ms=pad_ms,
            modo_voz=modo,
            palatalizar=palatalizar,
            virgula_interrogacao=virgula,
            trailing_ponto=trailing,
            target_lang=target_lang,
            source_lang=source_lang,
        ), "_val_worker")
        worker.progress_signal.connect(self.tab_val.on_redub_progress)
        worker.file_done_signal.connect(
            lambda ok, _msg, path_out, _nome, *_: self.tab_val.on_redub_pronto(ok, path_out)
        )
        worker.log_signal.connect(lambda msg, lv: self._log(f"[VAL] {msg}", lv))
        worker.start()

    def _run_tudo(self):
        """Dubla TODOS os arquivos da pasta de entrada e salva automaticamente na saída."""
        pasta_in = self.lne_in.text().strip()
        pasta_out = self.lne_out.text().strip()
        if not pasta_in or not os.path.isdir(pasta_in):
            return QMessageBox.warning(self, "Aviso", "Defina a pasta Origem (EN) primeiro.")
        if not pasta_out:
            return QMessageBox.warning(self, "Aviso", "Defina a pasta Destino (PT) primeiro.")

        exts = (".wav", ".mp3", ".wem", ".ogg", ".flac")
        arquivos = sorted([
            os.path.join(pasta_in, f) for f in os.listdir(pasta_in)
            if f.lower().endswith(exts)
        ])
        if not arquivos:
            return QMessageBox.warning(self, "Aviso", "Nenhum arquivo de áudio encontrado na pasta Origem.")

        self._log(f"🚀 Iniciando batch: {len(arquivos)} arquivos → {pasta_out}", "info")
        self._run_dub_multi(arquivos)

    def _cancelar(self):
        if self.worker and self.worker.isRunning():
            self._stop_worker(self.worker, "Dublagem", timeout_ms=5000)
            self._log("⏹ Cancelado pelo usuário.", "warning")
            self.lbl_status.setText("⏹ Cancelado.")
            self.lbl_status.setStyleSheet("color:#d29922;")
            self._set_botoes_ativos(True)

    def _run_dub(self):
        if not self._current_file: return QMessageBox.warning(self, "Aviso", "Selecione um arquivo no explorador primeiro.")
        self._run_dub_multi([self._current_file])
        
    def _run_dub_multi(self, paths):
        if not paths: return
        if self._is_worker_running(self.worker):
            self._log("Dublagem já está em andamento; aguarde ou cancele antes de iniciar outra.", "warning")
            return
        if self._is_worker_running(self._trans_worker):
            self._pending_dub_paths = list(paths)
            self._log("Transcrição em andamento; a dublagem vai começar automaticamente em seguida.", "warning")
            self.lbl_status.setText("Transcrição em andamento. Dublagem entra em seguida...")
            return
        if len(paths) == 1 and self._current_file != paths[0]:
            self._select_file(paths[0], auto_extract=False)
        self._last_dub_is_multi = (len(paths) > 1)
        modo = "antisotaque" if "antisotaque" in self.cmb_modo.currentText() else "classico"
        c_texts = {"en": self.txt_en.text(), "pt": self.txt_pt.text()}
        self._set_botoes_ativos(False)
        self.lbl_status.setText(f"⏳ [{modo.upper()}] Processando {len(paths)} arquivo(s)...")
        self.lbl_status.setStyleSheet("color:#f0883e;")
        self.prog_bar.setValue(0)

        s_lang = self.cmb_source_lang.currentText().split(' ')[0]
        t_lang = self.cmb_target_lang.currentText().split(' ')[0]

        worker = self._track_worker(SingleDubbingWorker(
            paths, self.lne_guide.text(), self.models, c_texts,
            self.spn_temp.value(), self.spn_pad.value(),
            modo_voz=modo,
            pasta_pool=self._pool_dir,
            palatalizar=self.chk_palatalizar.isChecked(),
            virgula_interrogacao=self.chk_virgula.isChecked(),
            trailing_ponto=self.chk_trailing.isChecked(),
            target_lang=t_lang,
            source_lang=s_lang,
        ), "worker")
        worker.log_signal.connect(self._on_dub_log)
        worker.progress_signal.connect(self.prog_bar.setValue)
        worker.file_done_signal.connect(self._on_dub_done)
        worker.transcription_ready_signal.connect(self._on_dub_transcription_ready)
        worker.finished_signal.connect(lambda: (
            self._set_botoes_ativos(True),
            self.lbl_status.setText("✅ Processo concluído!"),
            self.prog_bar.setValue(100),
        ))
        worker.start()

    def _on_dub_log(self, msg, level="info"):
        self._log(msg, level)
        texto = str(msg).lower()
        if "carregando whisper" in texto or "transcrevendo" in texto:
            self.lbl_status.setText("Extraindo transcrição do original...")
        elif "carregando omnivoice" in texto:
            self.lbl_status.setText("Carregando OmniVoice...")
        elif "gerando" in texto:
            self.lbl_status.setText("Gerando áudio dublado...")
        elif "aprovado" in texto:
            self.lbl_status.setText("Validando áudio gerado...")
        elif "falhou" in texto or "erro" in texto:
            self.lbl_status.setText(str(msg))

    def _on_dub_transcription_ready(self, en, pt):
        self.txt_en.setText(en)
        self.txt_pt.setText(pt)
        self.lbl_status.setText("Transcrição pronta. Gerando áudio dublado...")

    def _set_botoes_ativos(self, ativo: bool):
        self.btn_tudo.setEnabled(ativo)
        self.btn_dub.setEnabled(ativo)
        self.btn_trans.setEnabled(ativo)
        self.btn_redub.setEnabled(ativo)
        self.btn_cancelar.setEnabled(not ativo)

    def _gerar_pool(self):
        """Gera o pool de vozes PT-BR nativas (roda 1x, ~2 min)."""
        if self._is_worker_running(self.pool_worker):
            self._log("Pool de vozes já está sendo gerado.", "warning")
            return
        self.btn_pool.setEnabled(False)
        self.btn_pool.setText("⏳ Gerando vozes PT-BR...")
        self._log("🎙️ Iniciando geração do Pool de Vozes PT-BR...", "info")
        worker = self._track_worker(GeradorPoolWorker(self._pool_dir, self.models), "pool_worker")
        worker.log_signal.connect(self._log)
        worker.finished_signal.connect(lambda: (
            self.btn_pool.setEnabled(True),
            self.btn_pool.setText("✅ Pool PT-BR Gerado! (Clique p/ regerar)"),
        ))
        worker.start()

    def _resolver_original_para_revisao(self, original_name: str) -> str:
        current = getattr(self, "_current_file", "") or ""
        if current and os.path.basename(current) == original_name and os.path.exists(current):
            return current
        worker_paths = getattr(getattr(self, "worker", None), "paths_en", []) or []
        for path in worker_paths:
            if os.path.basename(path) == original_name and os.path.exists(path):
                return path
        pasta_in = self.lne_in.text().strip()
        if pasta_in:
            path = os.path.join(pasta_in, original_name)
            if os.path.exists(path):
                return path
        return ""

    def _salvar_para_revisao_manual(self, original_name: str, motivo: str):
        out_folder = self.lne_out.text().strip()
        if out_folder:
            pasta_revisao = os.path.join(out_folder, "rejeitados_revisao_manual")
        else:
            pasta_revisao = os.path.join(os.path.dirname(__file__), "rejeitados_revisao_manual")
        os.makedirs(pasta_revisao, exist_ok=True)

        src = self._resolver_original_para_revisao(original_name)
        if src:
            dest = os.path.join(pasta_revisao, original_name)
            try:
                if os.path.abspath(src) != os.path.abspath(dest):
                    shutil.copy2(src, dest)
            except Exception as e:
                self._log(f"⚠️ Não consegui copiar para revisão: {e}", "warning")

        log_path = os.path.join(pasta_revisao, "log_qualidade.txt")
        try:
            agora = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            with open(log_path, "a", encoding="utf-8") as f:
                f.write(f"[{agora}] ARQUIVO: {original_name} | MOTIVO: {motivo}\n")
        except Exception as e:
            self._log(f"⚠️ Não consegui gravar log de revisão: {e}", "warning")

        self._log(f"📂 Enviado para revisão: {pasta_revisao}", "warning")

    def _on_dub_done(self, ok, mov, path_out, original_name, txt_en="", txt_pt_final=""):
        if ok and path_out:
            self._current_result = path_out
            self._session_dubbed_paths[original_name] = path_out
            self.exp.update_status(original_name, True, "")
            self.play_pt.load(path_out)

            is_multi = getattr(self, '_last_dub_is_multi', False)
            out_folder = self.lne_out.text().strip()

            if is_multi:
                # ── AUTO-SAVE no batch: salva direto na pasta destino ──
                if out_folder:
                    os.makedirs(out_folder, exist_ok=True)
                    dest = os.path.join(out_folder, original_name)
                    try:
                        shutil.copy2(path_out, dest)
                        self._log(f"💾 Auto-salvo: {original_name}", "success")
                        self.exp.update_status(original_name, True, "")
                    except Exception as e:
                        self._log(f"❌ Erro ao salvar {original_name}: {e}", "error")
                else:
                    self._log("⚠️ Pasta destino não definida — arquivo não salvo!", "warning")
            else:
                # Arquivo único: toca automaticamente, usuário salva quando quiser
                self.play_pt.toggle_play()

            # Atualiza o cache de transcrição com o texto final se o usuário quiser ver depois
            if out_folder and txt_en and txt_pt_final:
                cache_file = os.path.join(out_folder, "transcricoes_cache.json")
                cache_data = {}
                if os.path.exists(cache_file):
                    try:
                        import json
                        with open(cache_file, "r", encoding="utf-8") as f:
                            cache_data = json.load(f)
                    except: pass
                cache_data[original_name] = {"en": txt_en, "pt": txt_pt_final}
                try:
                    import json
                    with open(cache_file, "w", encoding="utf-8") as f:
                        json.dump(cache_data, f, ensure_ascii=False, indent=2)
                except: pass

            self.lbl_status.setText(f"✅ {original_name} concluído!")
            self.lbl_status.setStyleSheet("color:#3fb950; font-weight:bold;")
        else:
            self._salvar_para_revisao_manual(original_name, mov)
            self.exp.update_status(original_name, False, mov)
            self.lbl_status.setText(f"❌ {original_name} Falhou: {mov}")
            self.lbl_status.setStyleSheet("color:#f85149;")

    def _save_result(self):
        if not self._current_result: return QMessageBox.warning(self, "Aviso", "Nenhum resultado gerado ainda.")
        out_folder = self.lne_out.text().strip()
        if not out_folder: return QMessageBox.warning(self, "Aviso", "Defina a pasta de destino (PT).")
        os.makedirs(out_folder, exist_ok=True)
        
        dest = os.path.join(out_folder, os.path.basename(self._current_file))
        shutil.copy2(self._current_result, dest)
        self._log(f"Salvo: {dest}", "success")
        self.exp.update_status(os.path.basename(self._current_file), True, "")
        self.lbl_status.setText(f"💾 Salvo com sucesso em {dest}!")

    def closeEvent(self, event):
        for worker in list(getattr(self, "_qt_workers", ())):
            self._stop_worker(worker, "Thread", timeout_ms=3000)
        if AudioPlayer.active_player:
            AudioPlayer.active_player.stop()
        try:
            pygame.mixer.quit()
        except Exception:
            pass
        self._save_cfg()
        event.accept()

# ─────────────────────────────────────────────────────────────────────────────
# GLOBAL EXCEPTION HANDLER — nunca deixa o programa fechar sem aviso
# ─────────────────────────────────────────────────────────────────────────────
_LOG_CRASH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "crash_log.txt")

def _escrever_log_crash(tipo, valor, tb):
    """Grava qualquer exceção não tratada em arquivo e tenta mostrar na UI."""
    import traceback as _tb
    txt = "".join(_tb.format_exception(tipo, valor, tb))
    ts  = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    linha = f"\n{'='*60}\n[{ts}] CRASH NÃO TRATADO:\n{txt}\n"
    try:
        with open(_LOG_CRASH, "a", encoding="utf-8") as f:
            f.write(linha)
    except Exception:
        pass
    # Tentar mostrar na janela principal se ela existir
    try:
        app = QApplication.instance()
        if app:
            for widget in app.topLevelWidgets():
                if hasattr(widget, '_log'):
                    widget._log(f"💥 CRASH: {valor}\n{txt}", "error")
                    widget._log(f"📝 Log gravado em: {_LOG_CRASH}", "warning")
                    break
    except Exception:
        pass

def _exception_hook(tipo, valor, tb):
    _escrever_log_crash(tipo, valor, tb)
    # NÃO chama sys.__excepthook__ para não fechar o programa
    import traceback as _tb
    print("".join(_tb.format_exception(tipo, valor, tb)), file=sys.stderr)

sys.excepthook = _exception_hook


if __name__ == "__main__":
    # Garantir encoding correto no console Windows
    try:
        sys.stdout.reconfigure(encoding='utf-8')
        sys.stderr.reconfigure(encoding='utf-8')
    except Exception:
        pass

    try:
        app = QApplication(sys.argv)
        app.setStyle("Fusion")
        # Handler de exceção também para threads Qt
        w = MainWindow()
        w.show()
        # Sobrescrever excepthook dentro da thread Qt principal
        sys.excepthook = _exception_hook
        ret = app.exec()
        sys.exit(ret)
    except Exception as _e:
        import traceback as _tb
        txt = _tb.format_exc()
        ts  = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        msg = f"\n{'='*60}\n[{ts}] ERRO NA INICIALIZAÇÃO:\n{txt}\n"
        print(msg, file=sys.stderr)
        try:
            with open(_LOG_CRASH, "a", encoding="utf-8") as f:
                f.write(msg)
        except Exception:
            pass
        # Mostrar erro em QMessageBox para não fechar silenciosamente
        try:
            _app2 = QApplication.instance() or QApplication(sys.argv)
            QMessageBox.critical(
                None,
                "Erro Fatal na Inicialização",
                f"O programa encontrou um erro ao iniciar:\n\n{_e}\n\nDetalhes salvos em:\n{_LOG_CRASH}",
            )
        except Exception:
            pass
        input("\nPressione ENTER para fechar...")
        sys.exit(1)
