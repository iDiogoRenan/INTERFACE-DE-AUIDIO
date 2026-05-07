# _patch_validacao.py — Aba de Validação Manual
import os, shutil
from PyQt6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton,
    QListWidget, QListWidgetItem, QGroupBox, QLineEdit,
    QFileDialog, QMessageBox, QCheckBox, QComboBox, QProgressBar, QTextEdit
)
from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtGui import QColor

class ValidacaoWidget(QWidget):
    """
    Aba de validação manual com transcrição dupla e players injetados.
    """
    # (src_path, texto_pt, modo, palatalizar, virgula, trailing, pad_ms, target_lang, source_lang)
    redub_request = pyqtSignal(str, str, str, bool, bool, bool, int, str, str)
    transcribe_request = pyqtSignal(str, str, str) # (src_path, target_lang, source_lang)
    batch_transcribe_request = pyqtSignal(list, str, str, str) # (paths_en, pasta_cache, target_lang, source_lang)

    def __init__(self, parent=None):
        super().__init__(parent)
        self._pasta_en       = ""
        self._pasta_dublados = ""
        self._pasta_final    = ""
        self._arquivo_pt     = ""
        self._arquivo_en     = ""
        self._player_en      = None
        self._player_pt      = None
        self._build_ui()

    def inject_players(self, player_en, player_pt):
        self._player_en = player_en
        self._player_pt = player_pt
        self._players_box_en.addWidget(player_en)
        self._players_box_pt.addWidget(player_pt)

    def _build_ui(self):
        root = QHBoxLayout(self)
        root.setContentsMargins(6, 6, 6, 6)
        root.setSpacing(6)

        # ESQUERDA: lista
        left_w = QWidget()
        left_w.setFixedWidth(290)
        left = QVBoxLayout(left_w)
        left.setContentsMargins(0, 0, 0, 0)

        grp_paths = QGroupBox("📂 Pastas de Validação")
        pl = QVBoxLayout(grp_paths)
        pl.setSpacing(3)
        self.lne_en       = self._path_row(pl, "EN Originais:")
        self.lne_dublados = self._path_row(pl, "PT Dublados: ")
        self.lne_final    = self._path_row(pl, "Aprovados:   ")
        btn_load = QPushButton("🔄  Carregar Lista de Dublados")
        btn_load.setStyleSheet("background:#1f6feb; color:white; font-weight:bold; padding:6px;")
        btn_load.clicked.connect(self._carregar_lista)
        pl.addWidget(btn_load)
        left.addWidget(grp_paths)

        grp_lista = QGroupBox("🎵 Arquivos")
        ll = QVBoxLayout(grp_lista)
        self.lbl_contagem = QLabel("0 arquivos")
        self.lbl_contagem.setStyleSheet("color:#8b949e; font-size:11px;")
        ll.addWidget(self.lbl_contagem)
        self.lista = QListWidget()
        self.lista.setStyleSheet(
            "QListWidget{background:#0d1117;border:1px solid #30363d;}"
            "QListWidget::item{padding:5px 8px;border-bottom:1px solid #21262d;font-size:11px;}"
            "QListWidget::item:selected{background:#1f6feb;color:white;}"
        )
        self.lista.currentItemChanged.connect(self._on_selecionar)
        ll.addWidget(self.lista)
        
        leg = QHBoxLayout()
        for cor, txt in [("#3fb950","✅"),("#f85149","❌"),("#8b949e","⏳")]:
            l = QLabel(txt); l.setStyleSheet(f"color:{cor}; font-size:11px;")
            leg.addWidget(l)
        self.lbl_stats = QLabel("0 | 0 | 0")
        self.lbl_stats.setStyleSheet("color:#8b949e; font-size:11px;")
        leg.addWidget(self.lbl_stats, 1)
        ll.addLayout(leg)
        left.addWidget(grp_lista, 1)
        root.addWidget(left_w)

        # DIREITA: painel principal
        right = QVBoxLayout()
        grp_play = QGroupBox("🔊 Áudios")
        players_row = QHBoxLayout(grp_play)
        left_play = QVBoxLayout()
        self._players_box_en = QVBoxLayout()
        left_play.addLayout(self._players_box_en)
        players_row.addLayout(left_play, 1)
        right_play = QVBoxLayout()
        self._players_box_pt = QVBoxLayout()
        right_play.addLayout(self._players_box_pt)
        players_row.addLayout(right_play, 1)
        right.addWidget(grp_play)

        grp_txt = QGroupBox("📝 Transcrição e Edição")
        tl = QVBoxLayout(grp_txt)
        
        self.btn_trans = QPushButton("✍️ Extrair Transcrição dos Áudios")
        self.btn_trans.setStyleSheet("background:#1f6feb; font-weight:bold; padding:5px;")
        self.btn_trans.clicked.connect(self._extrair)
        tl.addWidget(self.btn_trans)

        txt_row = QHBoxLayout()
        v_en = QVBoxLayout()
        v_en.addWidget(QLabel("Texto EN (Referência):"))
        self.txt_en = QTextEdit()
        self.txt_en.setPlaceholderText("Inglês original...")
        self.txt_en.setStyleSheet("background:#161b22; color:#c9d1d9;")
        self.txt_en.setReadOnly(True)
        v_en.addWidget(self.txt_en)
        txt_row.addLayout(v_en, 1)

        v_pt = QVBoxLayout()
        v_pt.addWidget(QLabel("Texto PT (Edite e Reduble):"))
        self.txt_pt = QTextEdit()
        self.txt_pt.setPlaceholderText("Português dublado...")
        self.txt_pt.setStyleSheet("background:#161b22; color:#e3b341;")
        v_pt.addWidget(self.txt_pt)
        txt_row.addLayout(v_pt, 1)
        
        tl.addLayout(txt_row)
        right.addWidget(grp_txt, 1)

        grp_cfg = QGroupBox("⚙️ Ajustes de Redublagem")
        cl = QHBoxLayout(grp_cfg)
        
        self.cmb_source_lang = QComboBox()
        self.cmb_source_lang.addItems(["auto", "en", "fr", "sv", "pt"])
        self.cmb_target_lang = QComboBox()
        self.cmb_target_lang.addItems(["pt", "fr", "sv", "en"])
        cl.addWidget(QLabel("Origem:"))
        cl.addWidget(self.cmb_source_lang)
        cl.addWidget(QLabel("Destino:"))
        cl.addWidget(self.cmb_target_lang)
        
        cl.addWidget(QLabel("Modo:"))
        self.cmb_modo = QComboBox()
        self.cmb_modo.addItems(["classico", "antisotaque"])
        cl.addWidget(self.cmb_modo)
        
        self.chk_palatalizar = QCheckBox("Palatização")
        self.chk_virgula     = QCheckBox("Vírgula antes de ?")
        self.chk_trailing    = QCheckBox("Ponto final extra")
        for chk in (self.chk_palatalizar, self.chk_virgula, self.chk_trailing):
            cl.addWidget(chk)
        cl.addStretch()
        right.addWidget(grp_cfg)

        grp_act = QGroupBox("🎯 Ações de Validação")
        al = QVBoxLayout(grp_act)
        self.btn_aprovar = QPushButton("✅   APROVAR  →  Pasta Aprovados")
        self.btn_aprovar.setStyleSheet("background:#238636; color:white; font-size:14px; font-weight:bold; padding:12px; border-radius:6px;")
        self.btn_aprovar.clicked.connect(self._aprovar)
        al.addWidget(self.btn_aprovar)

        self.btn_redub = QPushButton("🎙️  Redublar baseando-se no original EN")
        self.btn_redub.setStyleSheet("background:#5a3e00; color:#f0883e; font-weight:bold; padding:9px;")
        self.btn_redub.clicked.connect(self._redublar_en)
        al.addWidget(self.btn_redub)

        self.btn_redub_pt = QPushButton("🎙️  Redublar baseando-se no PT (Muda o texto, mantém voz PT)")
        self.btn_redub_pt.setStyleSheet("background:#5a004b; color:#f03edb; font-weight:bold; padding:9px;")
        self.btn_redub_pt.clicked.connect(self._redublar_pt)
        al.addWidget(self.btn_redub_pt)

        row_bot = QHBoxLayout()
        self.btn_rejeitar = QPushButton("❌  Rejeitar")
        self.btn_rejeitar.setStyleSheet("background:#6e1a1a; color:#ff9090; font-weight:bold; padding:8px;")
        self.btn_rejeitar.clicked.connect(self._rejeitar)
        row_bot.addWidget(self.btn_rejeitar)

        self.btn_proximo = QPushButton("⏭  Próximo")
        self.btn_proximo.setStyleSheet("background:#21262d; color:#c9d1d9; font-weight:bold; padding:8px;")
        self.btn_proximo.clicked.connect(self._proximo)
        row_bot.addWidget(self.btn_proximo)
        al.addLayout(row_bot)

        self.lbl_status = QLabel("Selecione um arquivo na lista à esquerda.")
        self.lbl_status.setAlignment(Qt.AlignmentFlag.AlignCenter)
        al.addWidget(self.lbl_status)

        self.prog_bar = QProgressBar()
        self.prog_bar.setRange(0, 100); self.prog_bar.setValue(0)
        self.prog_bar.setFixedHeight(10)
        al.addWidget(self.prog_bar)
        
        right.addWidget(grp_act)
        right_w = QWidget()
        right_w.setLayout(right)
        root.addWidget(right_w, 1)
        self._set_acoes_ativas(False)

    def _path_row(self, layout, label):
        row = QHBoxLayout()
        lbl = QLabel(label); lbl.setFixedWidth(80)
        row.addWidget(lbl)
        lne = QLineEdit(); lne.setReadOnly(True); lne.setPlaceholderText("Selecionar...")
        row.addWidget(lne, 1)
        btn = QPushButton("📁"); btn.setFixedWidth(30)
        btn.clicked.connect(lambda _, l=lne: self._escolher_pasta(l))
        row.addWidget(btn)
        layout.addLayout(row)
        return lne

    def _escolher_pasta(self, lne):
        p = QFileDialog.getExistingDirectory(self, "Selecionar pasta")
        if p:
            lne.setText(p)
            if lne is self.lne_en: self._pasta_en = p
            elif lne is self.lne_dublados: self._pasta_dublados = p
            elif lne is self.lne_final: self._pasta_final = p

    def _carregar_lista(self):
        pasta = self.lne_dublados.text().strip()
        if not pasta or not os.path.isdir(pasta):
            return QMessageBox.warning(self, "Aviso", "Defina a pasta PT Dublados primeiro.")
        self._pasta_dublados = pasta
        self.lista.clear()
        exts = (".wav", ".mp3", ".ogg", ".flac", ".wem")
        for nome in sorted(f for f in os.listdir(pasta) if f.lower().endswith(exts)):
            item = QListWidgetItem(f"⏳  {nome}")
            item.setData(Qt.ItemDataRole.UserRole, os.path.join(pasta, nome))
            item.setData(Qt.ItemDataRole.UserRole + 1, "pendente")
            item.setForeground(QColor("#8b949e"))
            self.lista.addItem(item)
        self.lbl_contagem.setText(f"{self.lista.count()} arquivo(s)")
        self._atualizar_stats()
        
        # Disparar extração em lote para todos os arquivos da lista
        en_paths = []
        for i in range(self.lista.count()):
            nome = os.path.basename(self.lista.item(i).data(Qt.ItemDataRole.UserRole))
            if self._pasta_en:
                p = os.path.join(self._pasta_en, nome)
                if os.path.exists(p):
                    en_paths.append(p)
        if en_paths:
            self.batch_transcribe_request.emit(en_paths, self._pasta_dublados, self.cmb_target_lang.currentText(), self.cmb_source_lang.currentText())
            
        if self.lista.count() > 0: self.lista.setCurrentRow(0)

    def _on_selecionar(self, item, _=None):
        if not item: return
        self._arquivo_pt = item.data(Qt.ItemDataRole.UserRole)
        nome = os.path.basename(self._arquivo_pt)
        en_path = os.path.join(self._pasta_en, nome) if self._pasta_en else ""
        self._arquivo_en = en_path if os.path.exists(en_path) else ""
        
        status_text = f"📄 {nome}"
        if not self._arquivo_en:
            status_text += " ⚠️ (Original EN não encontrado na pasta!)"
        self.lbl_status.setText(status_text)
        self.lbl_status.setStyleSheet("color:#f85149;" if not self._arquivo_en else "")
        
        self._set_acoes_ativas(True)
        self.txt_en.clear(); self.txt_pt.clear()
        
        # Tentar carregar do cache se já existir
        cache_file = os.path.join(self._pasta_dublados, "transcricoes_cache.json")
        if os.path.exists(cache_file):
            try:
                import json
                with open(cache_file, "r", encoding="utf-8") as f:
                    cache_data = json.load(f)
                if nome in cache_data:
                    self.txt_en.setText(cache_data[nome].get("en", ""))
                    self.txt_pt.setText(cache_data[nome].get("pt", ""))
            except: pass
        
        if self._player_en:
            if self._arquivo_en: self._player_en.load(self._arquivo_en)
            else: getattr(self._player_en, 'clear', lambda m: None)("(EN não encontrado)")
        if self._player_pt:
            self._player_pt.load(self._arquivo_pt)
            self._player_pt.toggle_play()

    def _extrair(self):
        src = self._arquivo_en
        if not src or not os.path.exists(src):
            return QMessageBox.warning(self, "Aviso", "Áudio EN original não encontrado para transcrição.")
        self.transcribe_request.emit(src, self.cmb_target_lang.currentText(), self.cmb_source_lang.currentText())
        self.lbl_status.setText("⏳ Extraindo transcrição...")
        self.lbl_status.setStyleSheet("color:#f0883e;")
        self._set_acoes_ativas(False)

    def on_transcribe_pronto(self, en: str, pt: str):
        self._set_acoes_ativas(True)
        self.txt_en.setText(en)
        self.txt_pt.setText(pt)
        self.lbl_status.setText("✅ Transcrição concluída.")
        self.lbl_status.setStyleSheet("color:#3fb950;")

    def _aprovar(self):
        item = self.lista.currentItem()
        if not item or not self._arquivo_pt: return
        pasta_final = self.lne_final.text().strip()
        if not pasta_final: return QMessageBox.warning(self, "Aviso", "Defina a pasta Aprovados primeiro.")
        os.makedirs(pasta_final, exist_ok=True)
        dest = os.path.join(pasta_final, os.path.basename(self._arquivo_pt))
        try:
            shutil.copy2(self._arquivo_pt, dest)
            item.setText(f"✅  {os.path.basename(self._arquivo_pt)}")
            item.setForeground(QColor("#3fb950"))
            item.setData(Qt.ItemDataRole.UserRole + 1, "aprovado")
            self.lbl_status.setText("✅ Aprovado!"); self.lbl_status.setStyleSheet("color:#3fb950;")
            self._atualizar_stats(); self._proximo()
        except Exception as e: QMessageBox.critical(self, "Erro", f"Falha ao copiar:\n{e}")

    def _rejeitar(self):
        item = self.lista.currentItem()
        if not item: return
        item.setText(f"❌  {os.path.basename(self._arquivo_pt)}")
        item.setForeground(QColor("#f85149"))
        item.setData(Qt.ItemDataRole.UserRole + 1, "rejeitado")
        self.lbl_status.setText("❌ Rejeitado."); self.lbl_status.setStyleSheet("color:#f85149;")
        self._atualizar_stats(); self._proximo()

    def _redublar_en(self):
        src = self._arquivo_en
        if not src: return QMessageBox.warning(self, "Aviso", "Selecione um arquivo original EN primeiro.")
        self._iniciar_redublagem(src)

    def _redublar_pt(self):
        src = self._arquivo_pt
        if not src: return QMessageBox.warning(self, "Aviso", "Selecione um arquivo PT primeiro.")
        self._iniciar_redublagem(src)

    def _iniciar_redublagem(self, src):
        modo = "antisotaque" if "antisotaque" in self.cmb_modo.currentText() else "classico"
        self.redub_request.emit(
            src, self.txt_pt.toPlainText().strip(), modo,
            self.chk_palatalizar.isChecked(), self.chk_virgula.isChecked(),
            self.chk_trailing.isChecked(), 200,
            self.cmb_target_lang.currentText(), self.cmb_source_lang.currentText()
        )
        self.lbl_status.setText("⏳ Redublando..."); self.lbl_status.setStyleSheet("color:#f0883e;")
        self.prog_bar.setValue(0)
        self._set_acoes_ativas(False)

    def _proximo(self):
        row = self.lista.currentRow()
        if row < self.lista.count() - 1: self.lista.setCurrentRow(row + 1)

    def _set_acoes_ativas(self, ativo: bool):
        for b in (self.btn_aprovar, self.btn_rejeitar, self.btn_redub, self.btn_redub_pt, self.btn_proximo, self.btn_trans):
            b.setEnabled(ativo)

    def _atualizar_stats(self):
        ap = rej = pen = 0
        for i in range(self.lista.count()):
            s = self.lista.item(i).data(Qt.ItemDataRole.UserRole + 1)
            if s == "aprovado": ap += 1
            elif s == "rejeitado": rej += 1
            else: pen += 1
        self.lbl_stats.setText(f"✅{ap} ❌{rej} ⏳{pen}")

    def on_redub_pronto(self, ok: bool, path_out: str):
        self._set_acoes_ativas(True)
        if ok and path_out:
            try: shutil.copy2(path_out, self._arquivo_pt)
            except Exception: self._arquivo_pt = path_out
            if self._player_pt:
                self._player_pt.load(self._arquivo_pt)
                self._player_pt.toggle_play()
            self.lbl_status.setText("✅ Redublado! Ouça e aprove ou rejeite.")
            self.lbl_status.setStyleSheet("color:#3fb950;")
            self.prog_bar.setValue(100)
        else:
            self.lbl_status.setText("❌ Redublagem falhou.")
            self.lbl_status.setStyleSheet("color:#f85149;")

    def on_redub_progress(self, val: int): self.prog_bar.setValue(val)
    def definir_pastas(self, pasta_en: str = "", pasta_dublados: str = ""):
        if pasta_en and os.path.isdir(pasta_en): self._pasta_en = pasta_en; self.lne_en.setText(pasta_en)
        if pasta_dublados and os.path.isdir(pasta_dublados): self._pasta_dublados = pasta_dublados; self.lne_dublados.setText(pasta_dublados)
    def definir_pasta_dublados(self, pasta: str): self.definir_pastas(pasta_dublados=pasta)
    def on_batch_progress(self, current, total, nome, en, pt):
        self.lbl_status.setText(f"⏳ Extraindo transcrições em lote ({current}/{total})...")
        self.lbl_status.setStyleSheet("color:#8b949e;")
        # Se for o arquivo atualmente selecionado, já atualiza a UI
        if self._arquivo_en and os.path.basename(self._arquivo_en) == nome:
            if not self.txt_en.toPlainText().strip():
                self.txt_en.setText(en)
                self.txt_pt.setText(pt)
    def on_batch_done(self):
        self.lbl_status.setText("✅ Transcrições em lote concluídas e salvas no cache!")
        self.lbl_status.setStyleSheet("color:#3fb950;")
