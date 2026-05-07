
# Patch: AudioPlayer com Pygame + Extracao de Transcricao + Custom Texts
import ast, re

with open('DUBLAGEM_MASTER_PRO_v11.py', 'r', encoding='utf-8') as f:
    src = f.read()

# ── 1. Substituir AudioPlayer para usar pygame.mixer.music ──────────────────
# Vamos localizar class AudioPlayer
idx_start = src.find('class AudioPlayer(QWidget):')
idx_end = src.find('class ControlPanel(QScrollArea):')
OLD_PLAYER = src[idx_start:idx_end]

NEW_PLAYER = '''import pygame
if not pygame.mixer.get_init():
    pygame.mixer.init()

class AudioPlayer(QWidget):
    """Player de áudio integrado para arquivos WAV usando Pygame (music)."""
    active_player = None

    def __init__(self, label_text: str = "Áudio", parent=None):
        super().__init__(parent)
        self._current_path = ""
        self._is_playing = False
        self._is_paused = False
        self._duration_sec = 0.0
        self._seek_pos = 0.0
        self._start_ticks = 0
        self._vol = 0.8
        
        self._timer = QTimer(self)
        self._timer.setInterval(100)
        self._timer.timeout.connect(self._update_time)

        self._build_ui(label_text)

    def _build_ui(self, label_text: str):
        main = QVBoxLayout(self)
        main.setSpacing(4)
        main.setContentsMargins(0, 0, 0, 0)

        # Header
        hdr = QHBoxLayout()
        self.lbl_header = QLabel(label_text)
        self.lbl_header.setStyleSheet("color: #58a6ff; font-weight: bold; font-size: 12px;")
        hdr.addWidget(self.lbl_header)
        self.lbl_info = QLabel("Nenhum arquivo")
        self.lbl_info.setStyleSheet("color: #8b949e; font-size: 11px;")
        hdr.addWidget(self.lbl_info, 1)
        self.lbl_channel = QLabel("")
        self.lbl_channel.setStyleSheet("color: #3fb950; font-size: 11px; font-weight: bold;")
        hdr.addWidget(self.lbl_channel)
        main.addLayout(hdr)

        # Controls
        ctrl = QHBoxLayout()
        self.btn_play = QPushButton("▶")
        self.btn_play.setObjectName("btn_play")
        self.btn_play.clicked.connect(self.toggle_play)
        self.btn_play.setEnabled(False)
        ctrl.addWidget(self.btn_play)

        self.slider = QSlider(Qt.Orientation.Horizontal)
        self.slider.setRange(0, 1000)
        self.slider.sliderReleased.connect(self._seek)
        self.slider.sliderPressed.connect(self._on_slider_press)
        ctrl.addWidget(self.slider, 1)

        self.lbl_time = QLabel("0:00 / 0:00")
        self.lbl_time.setStyleSheet("color: #8b949e; font-size: 11px; min-width: 90px;")
        ctrl.addWidget(self.lbl_time)

        vol_lbl = QLabel("🔊")
        ctrl.addWidget(vol_lbl)
        self.vol_slider = QSlider(Qt.Orientation.Horizontal)
        self.vol_slider.setRange(0, 100)
        self.vol_slider.setValue(80)
        self.vol_slider.setMaximumWidth(80)
        self.vol_slider.valueChanged.connect(self._set_volume)
        ctrl.addWidget(self.vol_slider)

        main.addLayout(ctrl)

    def load(self, path: str):
        if not path or not os.path.exists(path):
            return
        if AudioPlayer.active_player is self:
            self.stop()
            
        self._current_path = path
        self.btn_play.setEnabled(True)
        self._seek_pos = 0.0
        self.slider.setValue(0)

        # Info
        info = get_audio_info(path)
        if info:
            self._duration_sec = info.get("duration", 0)
            ch  = "Stereo" if info.get("channels", 1) > 1 else "Mono"
            sr  = info.get("sr", 0)
            self.lbl_info.setText(f"{os.path.basename(path)}  |  {self._duration_sec:.1f}s  |  {sr}Hz")
            color = "#58a6ff" if ch == "Stereo" else "#3fb950"
            self.lbl_channel.setStyleSheet(f"color: {color}; font-size: 11px; font-weight: bold;")
            self.lbl_channel.setText(f"[{ch}]")
        else:
            self.lbl_info.setText(os.path.basename(path))
            self._duration_sec = 0.0
        self._update_labels(0.0)

    def toggle_play(self):
        if not self._current_path: return

        if self._is_playing:
            self.pause()
        else:
            self.play()

    def play(self):
        if AudioPlayer.active_player and AudioPlayer.active_player is not self:
            AudioPlayer.active_player.stop()

        AudioPlayer.active_player = self
        pygame.mixer.music.set_volume(self._vol)

        if self._is_paused:
            pygame.mixer.music.unpause()
        else:
            try:
                pygame.mixer.music.load(self._current_path)
                pygame.mixer.music.play(start=self._seek_pos)
            except Exception as e:
                print(f"Erro ao tocar {self._current_path}: {e}")
                return
            
        self._start_ticks = pygame.time.get_ticks() - int(self._seek_pos * 1000)
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
            AudioPlayer.active_player = None
        self._is_playing = False
        self._is_paused = False
        self.btn_play.setText("▶")
        self._timer.stop()
        self._seek_pos = 0.0
        self.slider.blockSignals(True)
        self.slider.setValue(0)
        self.slider.blockSignals(False)
        self._update_labels(0.0)

    def _set_volume(self, val):
        self._vol = val / 100.0
        if AudioPlayer.active_player is self:
            pygame.mixer.music.set_volume(self._vol)

    def _on_slider_press(self):
        if self._is_playing:
            self._was_playing = True
            self.pause()
        else:
            self._was_playing = False

    def _seek(self):
        val = self.slider.value()
        self._seek_pos = (val / 1000.0) * self._duration_sec
        self._is_paused = False # force reload
        self._update_labels(self._seek_pos)
        if getattr(self, '_was_playing', False):
            self.play()

    def _update_time(self):
        if not self._is_playing: return
        # pygame.mixer.music.get_pos() isn't always reliable with starts, calculate manually
        current = (pygame.time.get_ticks() - self._start_ticks) / 1000.0
        if current >= self._duration_sec:
            self.stop()
            return
        
        self.slider.blockSignals(True)
        self.slider.setValue(int((current / max(self._duration_sec, 0.1)) * 1000))
        self.slider.blockSignals(False)
        self._update_labels(current)
        self._seek_pos = current

    def _update_labels(self, pos_sec: float):
        m, s = int(pos_sec // 60), int(pos_sec % 60)
        dm, ds = int(self._duration_sec // 60), int(self._duration_sec % 60)
        self.lbl_time.setText(f"{m}:{s:02d} / {dm}:{ds:02d}")

    def get_current_path(self) -> str:
        return self._current_path

# ─────────────────────────────────────────────────────────────────────────────
# PAINEL DE CONTROLES (filtros / parâmetros em tempo real)
# ─────────────────────────────────────────────────────────────────────────────
'''
src = src.replace(OLD_PLAYER, NEW_PLAYER, 1)

# ── 2. Adicionar TranscribeWorker ──────────────────────────────────────────
OLD_SINGLE_WORKER = 'class SingleFileWorker(QThread):'

TRANSCRIBE_WORKER = '''class TranscribeWorker(QThread):
    """Extrai apenas a transcrição EN e traduz para ES e PT, sem gerar dublagem."""
    log_signal    = pyqtSignal(str, str)
    result_signal = pyqtSignal(str, str, str)  # (en, es, pt)
    error_signal  = pyqtSignal(str)

    def __init__(self, path_en: str, cfg: dict, models_ref: dict, parent=None):
        super().__init__(parent)
        self.path_en = path_en
        self.cfg = cfg
        self.models = models_ref

    def log(self, msg: str, level: str = "info"):
        self.log_signal.emit(msg, level)

    def run(self):
        import torch
        from deep_translator import GoogleTranslator
        import whisper

        try:
            cfg = self.cfg
            dev = "cuda" if torch.cuda.is_available() else "cpu"
            wtemp = cfg.get("whisper_temperature", 0.0)

            if self.models["whisper"] is None:
                self.log("📥 Carregando Whisper para transcrição...", "info")
                self.models["whisper"] = whisper.load_model(cfg.get("whisper_model", "medium"), device=dev)

            self.log(f"🔍 Extraindo transcrição de: {os.path.basename(self.path_en)}", "info")
            res_en = self.models["whisper"].transcribe(self.path_en, language='en', temperature=wtemp)
            txt_en = res_en["text"].strip()
            self.log(f"📄 EN (Original): {txt_en}", "info")

            txt_es = ""
            if cfg.get("use_spanish_bridge", False):
                txt_es = GoogleTranslator(source='en', target='es').translate(txt_en)
                self.log(f"📄 ES (Traduzido): {txt_es}", "info")

            txt_pt_raw = GoogleTranslator(source='en', target='pt').translate(txt_en)
            txt_pt = corrigir_pronuncia_br(txt_pt_raw, cfg)
            txt_pt = corrigir_r_forte(txt_pt, cfg)
            txt_pt = sincronizar_pontuacao(txt_pt, txt_en)
            self.log(f"📄 PT (Corrigido): {txt_pt}", "info")

            self.result_signal.emit(txt_en, txt_es, txt_pt)
        except Exception as e:
            self.error_signal.emit(str(e))

class SingleFileWorker(QThread):'''

src = src.replace(OLD_SINGLE_WORKER, TRANSCRIBE_WORKER, 1)

# ── 3. Modificar SingleFileWorker para aceitar custom_texts ────────────────
# Na assinatura do init:
src = src.replace('def __init__(self, path_en: str, guide_path: str, cfg: dict, models_ref: dict, parent=None):',
                  'def __init__(self, path_en: str, guide_path: str, cfg: dict, models_ref: dict, custom_texts: dict = None, parent=None):')
src = src.replace('self.models    = models_ref', 'self.models    = models_ref\n        self.custom_texts = custom_texts or {}')

# No metodo _run, substituir a parte de transcrição e tradução:
OLD_TRANS = '''        # ── 1. Transcrever EN ──────────────────────────────────────────────
        self.log(f"🔍 Transcrevendo: {os.path.basename(self.path_en)}", "info")
        res_en   = models["whisper"].transcribe(self.path_en, language='en', temperature=wtemp)
        txt_en   = res_en["text"].strip()
        self.log(f"📄 EN: {txt_en}", "info")

        # ── 2. Traduzir ────────────────────────────────────────────────────
        txt_pt_raw = GoogleTranslator(source='en', target='pt').translate(txt_en)
        txt_es     = ""
        use_es     = cfg.get("use_spanish_bridge", False)
        if use_es:
            txt_es = GoogleTranslator(source='en', target='es').translate(txt_en)
            self.log(f"📄 ES: {txt_es}", "info")

        # ── 3. Correções fonéticas ─────────────────────────────────────────
        txt_pt = corrigir_pronuncia_br(txt_pt_raw, cfg)
        txt_pt = corrigir_r_forte(txt_pt, cfg)
        txt_pt = sincronizar_pontuacao(txt_pt, txt_en)
        self.log(f"📄 PT (ajustado): {txt_pt}", "info")'''

NEW_TRANS = '''        use_es = cfg.get("use_spanish_bridge", False)
        
        if self.custom_texts.get("pt"):
            # Usar textos customizados da interface (se preenchidos)
            txt_en = self.custom_texts.get("en", "")
            txt_es = self.custom_texts.get("es", "")
            txt_pt = self.custom_texts.get("pt", "")
            self.log("✏️ Usando transcrições editadas manualmente da interface.", "info")
            if not txt_en and models["whisper"] is not None:
                # Pegar texto EN para referencia se faltar
                res_en = models["whisper"].transcribe(self.path_en, language='en', temperature=wtemp)
                txt_en = res_en["text"].strip()
        else:
            # ── 1. Transcrever EN ──────────────────────────────────────────────
            self.log(f"🔍 Transcrevendo: {os.path.basename(self.path_en)}", "info")
            res_en   = models["whisper"].transcribe(self.path_en, language='en', temperature=wtemp)
            txt_en   = res_en["text"].strip()
            self.log(f"📄 EN: {txt_en}", "info")

            # ── 2. Traduzir ────────────────────────────────────────────────────
            txt_pt_raw = GoogleTranslator(source='en', target='pt').translate(txt_en)
            txt_es     = ""
            if use_es:
                txt_es = GoogleTranslator(source='en', target='es').translate(txt_en)
                self.log(f"📄 ES: {txt_es}", "info")

            # ── 3. Correções fonéticas ─────────────────────────────────────────
            txt_pt = corrigir_pronuncia_br(txt_pt_raw, cfg)
            txt_pt = corrigir_r_forte(txt_pt, cfg)
            txt_pt = sincronizar_pontuacao(txt_pt, txt_en)
            self.log(f"📄 PT (ajustado): {txt_pt}", "info")'''

src = src.replace(OLD_TRANS, NEW_TRANS, 1)

# ── 4. Adicionar Botão e Método no UI ──────────────────────────────────────
# Vamos procurar "# Transcrições" no _build_ui
OLD_TRANS_UI = '''        grp_trans = QGroupBox("📝 Transcrições (editável - altera o que a IA fala)")
        t_layout = QVBoxLayout(grp_trans)

        r_en = QHBoxLayout()
        r_en.addWidget(QLabel("🇺🇸 EN:"))
        self.txt_en = QLineEdit()
        self.txt_en.setPlaceholderText("Transcrição em inglês aparecerá aqui...")
        r_en.addWidget(self.txt_en, 1)
        t_layout.addLayout(r_en)'''

NEW_TRANS_UI = '''        grp_trans = QGroupBox("📝 Transcrições (editável - altera o que a IA fala)")
        t_layout = QVBoxLayout(grp_trans)
        
        btn_tr = QHBoxLayout()
        self.btn_extract_trans = QPushButton("📝  Extrair Transcrição Apenas (Arquivo Selecionado)")
        self.btn_extract_trans.setStyleSheet("QPushButton { background:#1f6feb; font-weight:bold; }")
        self.btn_extract_trans.clicked.connect(self._extract_transcription)
        btn_tr.addWidget(self.btn_extract_trans)
        btn_tr.addStretch()
        t_layout.addLayout(btn_tr)

        r_en = QHBoxLayout()
        r_en.addWidget(QLabel("🇺🇸 EN:"))
        self.txt_en = QLineEdit()
        self.txt_en.setPlaceholderText("Transcrição em inglês aparecerá aqui...")
        r_en.addWidget(self.txt_en, 1)
        t_layout.addLayout(r_en)'''

src = src.replace(OLD_TRANS_UI, NEW_TRANS_UI, 1)

# Adicionar metodos na MainWindow:
OLD_PAUSE = '''    def _toggle_pause(self):'''

NEW_TRANS_METH = '''    # ── Extrair transcrição apenas ──────────────────────────────────────────

    def _extract_transcription(self):
        row = self._test_list.currentRow()
        if row < 0 or row >= len(self._test_files):
            QMessageBox.warning(self, "Atenção", "Selecione um arquivo na lista de Teste primeiro.")
            return
        path = self._test_files[row]
        
        self.btn_extract_trans.setEnabled(False)
        self.lbl_test_status.setText("⏳ Extraindo transcrição...")
        self.txt_en.clear()
        self.txt_es.clear()
        self.txt_pt.clear()
        
        self.worker_trans = TranscribeWorker(path, self.cfg, self.models)
        self.worker_trans.log_signal.connect(self._log)
        self.worker_trans.result_signal.connect(self._on_trans_extracted)
        self.worker_trans.error_signal.connect(self._on_trans_error)
        self.worker_trans.finished.connect(lambda: self.btn_extract_trans.setEnabled(True))
        self.worker_trans.start()
        
    def _on_trans_extracted(self, en, es, pt):
        self.txt_en.setText(en)
        self.txt_es.setText(es)
        self.txt_pt.setText(pt)
        self.lbl_test_status.setText("✅ Transcrição extraída. Você pode editar o texto e depois clicar em Dublar Selecionados.")
        self.lbl_test_status.setStyleSheet("color:#3fb950; font-weight:bold;")

    def _on_trans_error(self, err):
        self._log(f"❌ Erro na transcrição: {err}", "error")
        self.lbl_test_status.setText(f"❌ Erro: {err[:80]}")

    def _toggle_pause(self):'''

src = src.replace(OLD_PAUSE, NEW_TRANS_METH, 1)

# Atualizar `_process_next_test` para passar os custom_texts
OLD_WORKER_CALL = '''        self.worker_single = SingleFileWorker(
            path,
            self._lne_guide.text().strip(),
            self.cfg,
            self.models
        )'''

NEW_WORKER_CALL = '''        # Passar os textos customizados caso o usuário tenha preenchido ou editado
        custom = {
            "en": self.txt_en.text().strip(),
            "es": self.txt_es.text().strip(),
            "pt": self.txt_pt.text().strip(),
        }

        self.worker_single = SingleFileWorker(
            path,
            self._lne_guide.text().strip(),
            self.cfg,
            self.models,
            custom_texts=custom
        )'''

src = src.replace(OLD_WORKER_CALL, NEW_WORKER_CALL, 1)


with open('DUBLAGEM_MASTER_PRO_v11.py', 'w', encoding='utf-8') as f:
    f.write(src)
print("Patch aplicado.")
