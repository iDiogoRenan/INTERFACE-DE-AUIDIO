import ast

with open('DUBLAGEM_MASTER_PRO_v13_PRATICO.py', 'r', encoding='utf-8') as f:
    src = f.read()

# 1. Atualizar SingleDubbingWorker para suportar multiplos arquivos
OLD_WORKER_INIT = '''class SingleDubbingWorker(QThread):
    """Worker focado em 1 arquivo por vez para a workflow prática."""
    log_signal = pyqtSignal(str, str)
    file_done_signal = pyqtSignal(bool, str, str) # ok, motivo, path_gerado
    finished_signal = pyqtSignal()

    def __init__(self, path_en, pasta_guia, models_ref, custom_texts, omni_temp, pad_ms, parent=None):
        super().__init__(parent)
        self.path_en = path_en
        self.pasta_guia = pasta_guia'''

NEW_WORKER_INIT = '''class SingleDubbingWorker(QThread):
    """Worker prático: dubla 1 ou vários arquivos em fila (para testes rápidos)."""
    log_signal = pyqtSignal(str, str)
    file_done_signal = pyqtSignal(bool, str, str, str) # ok, motivo, path_gerado, nome_original
    finished_signal = pyqtSignal()

    def __init__(self, paths_en, pasta_guia, models_ref, custom_texts, omni_temp, pad_ms, parent=None):
        super().__init__(parent)
        self.paths_en = paths_en if isinstance(paths_en, list) else [paths_en]
        self.pasta_guia = pasta_guia'''

src = src.replace(OLD_WORKER_INIT, NEW_WORKER_INIT, 1)

OLD_WORKER_RUN = '''            nome = os.path.basename(self.path_en)
            saida_final = os.path.join(self._temp_dir, f"teste_{nome}")
            temp_gen = os.path.join(self._temp_dir, "temp_verificacao.wav")
            temp_ref = os.path.join(self._temp_dir, "temp_ref.wav")

            # 1. Textos
            txt_en = self.custom_texts.get("en", "")
            txt_pt_final = self.custom_texts.get("pt", "")
            
            if not txt_en or not txt_pt_final:
                res = self.models["whisper"].transcribe(self.path_en, language='en', temperature=0.0)
                if not txt_en: txt_en = res["text"].strip()
                if not txt_pt_final:
                    txt_pt = GoogleTranslator(source='en', target='pt').translate(txt_en)
                    txt_pt = corrigir_pronuncia_br(txt_pt)
                    txt_pt_final = sincronizar_pontuacao(txt_pt, txt_en)

            self.log(f"📄 Texto para a IA: {txt_pt_final}", "info")

            # 2. Ref
            if not guia_valido:
                try:
                    y_orig_ref, sr_orig_ref = librosa.load(self.path_en, sr=24000)
                    y_trim, _ = librosa.effects.trim(y_orig_ref, top_db=45)
                    sf.write(temp_ref, y_trim, sr_orig_ref)
                    ref_audio_uso = temp_ref
                    ref_text_uso = txt_en
                except:
                    ref_audio_uso = self.path_en
                    ref_text_uso = txt_en
            else:
                ref_audio_uso = temp_guia_6min
                ref_text_uso = txt_guia_fixo

            # 3. Geracao
            audio_aprovado = None
            motivo_geracao = ""
            
            for tentativa in range(3):
                try:
                    t_audio = self.models["omni"].generate(
                        text=txt_pt_final, ref_audio=ref_audio_uso, ref_text=ref_text_uso, language="pt", temperature=self.omni_temp
                    )
                    if t_audio is not None:
                        y_val = np.array(t_audio).astype(np.float32).flatten()
                        valido_zcr, _ = validar_qualidade_zcr(y_val)
                        if not valido_zcr:
                            motivo_geracao = "ZCR alto (ruído metálico)"
                            continue
                        
                        sf.write(temp_gen, y_val, 24000)
                        res_temp = self.models["whisper"].transcribe(temp_gen, language='pt', temperature=0.0)
                        texto_ouvido = res_temp["text"].strip()
                        
                        similaridade = calcular_similaridade_texto(txt_pt_final, texto_ouvido)
                        if similaridade > 0.55:
                            audio_aprovado = y_val
                            break
                        else:
                            motivo_geracao = f"Alucinação (Sim: {similaridade:.2f})"
                except Exception as e:
                    motivo_geracao = str(e)
            
            if audio_aprovado is not None:
                sincronizar_master_v10_1(audio_aprovado, saida_final, self.path_en, self.pad_ms)
                self.log(f"✅ Gerado com sucesso!", "success")
                self.file_done_signal.emit(True, "", saida_final)
            else:
                self.log(f"❌ Falha: {motivo_geracao}", "error")
                self.file_done_signal.emit(False, motivo_geracao, "")'''

NEW_WORKER_RUN = '''            temp_gen = os.path.join(self._temp_dir, "temp_verificacao.wav")
            temp_ref = os.path.join(self._temp_dir, "temp_ref.wav")

            for idx_p, curr_path in enumerate(self.paths_en):
                nome = os.path.basename(curr_path)
                saida_final = os.path.join(self._temp_dir, f"teste_{nome}")
                self.log(f"🎙️ [{idx_p+1}/{len(self.paths_en)}] Processando: {nome}", "info")

                # Se for mais de 1 arquivo, ignora o texto customizado da UI
                use_custom = (len(self.paths_en) == 1)
                txt_en = self.custom_texts.get("en", "") if use_custom else ""
                txt_pt_final = self.custom_texts.get("pt", "") if use_custom else ""
                
                if not txt_en or not txt_pt_final:
                    res = self.models["whisper"].transcribe(curr_path, language='en', temperature=0.0)
                    if not txt_en: txt_en = res["text"].strip()
                    if not txt_pt_final:
                        txt_pt = GoogleTranslator(source='en', target='pt').translate(txt_en)
                        txt_pt = corrigir_pronuncia_br(txt_pt)
                        txt_pt_final = sincronizar_pontuacao(txt_pt, txt_en)

                self.log(f"📄 Texto: {txt_pt_final}", "info")

                if not guia_valido:
                    try:
                        y_orig_ref, sr_orig_ref = librosa.load(curr_path, sr=24000)
                        y_trim, _ = librosa.effects.trim(y_orig_ref, top_db=45)
                        sf.write(temp_ref, y_trim, sr_orig_ref)
                        ref_audio_uso = temp_ref
                        ref_text_uso = txt_en
                    except:
                        ref_audio_uso = curr_path
                        ref_text_uso = txt_en
                else:
                    ref_audio_uso = temp_guia_6min
                    ref_text_uso = txt_guia_fixo

                audio_aprovado = None
                motivo_geracao = ""
                
                for tentativa in range(3):
                    try:
                        t_audio = self.models["omni"].generate(
                            text=txt_pt_final, ref_audio=ref_audio_uso, ref_text=ref_text_uso, language="pt", temperature=self.omni_temp
                        )
                        if t_audio is not None:
                            y_val = np.array(t_audio).astype(np.float32).flatten()
                            valido_zcr, _ = validar_qualidade_zcr(y_val)
                            if not valido_zcr:
                                motivo_geracao = "ZCR alto (ruído metálico)"
                                continue
                            
                            sf.write(temp_gen, y_val, 24000)
                            res_temp = self.models["whisper"].transcribe(temp_gen, language='pt', temperature=0.0)
                            texto_ouvido = res_temp["text"].strip()
                            
                            similaridade = calcular_similaridade_texto(txt_pt_final, texto_ouvido)
                            if similaridade > 0.55:
                                audio_aprovado = y_val
                                break
                            else:
                                motivo_geracao = f"Alucinação (Sim: {similaridade:.2f})"
                    except Exception as e:
                        motivo_geracao = str(e)
                
                if audio_aprovado is not None:
                    sincronizar_master_v10_1(audio_aprovado, saida_final, curr_path, self.pad_ms)
                    self.log(f"✅ {nome} Gerado com sucesso!", "success")
                    self.file_done_signal.emit(True, "", saida_final, nome)
                else:
                    self.log(f"❌ {nome} Falhou: {motivo_geracao}", "error")
                    self.file_done_signal.emit(False, motivo_geracao, "", nome)'''

src = src.replace(OLD_WORKER_RUN, NEW_WORKER_RUN, 1)

OLD_FILE_EXP = '''class FileExplorer(QWidget):
    file_selected = pyqtSignal(str)
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
        self.tree.itemClicked.connect(self._on_click)
        layout.addWidget(self.tree)'''

NEW_FILE_EXP = '''class FileExplorer(QWidget):
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
                self.extract_request.emit(paths[0])
            elif action == ac_dub:
                if len(paths) == 1: self.file_selected.emit(paths[0])
                self.dub_request.emit(paths)'''

src = src.replace(OLD_FILE_EXP, NEW_FILE_EXP, 1)

OLD_EXP_REFRESH = '''    def refresh(self):
        self.tree.clear()
        if self._in and os.path.isdir(self._in):
            root = QTreeWidgetItem(self.tree, [os.path.basename(self._in), "Origem"])
            root.setForeground(0, QColor("#58a6ff"))
            for f in sorted(os.listdir(self._in)):
                if f.lower().endswith((".wav", ".mp3")):
                    st = self._status.get(f, "Pendente")
                    color = "#3fb950" if "✅" in st else "#f85149" if "❌" in st else "#8b949e"
                    it = QTreeWidgetItem(root, [f, st])
                    it.setForeground(1, QColor(color))
                    it.setData(0, Qt.ItemDataRole.UserRole, os.path.join(self._in, f))
            root.setExpanded(True)'''

NEW_EXP_REFRESH = '''    def refresh(self):
        self.tree.clear()
        if self._in and os.path.isdir(self._in):
            root = QTreeWidgetItem(self.tree, [os.path.basename(self._in), "Origem"])
            root.setForeground(0, QColor("#58a6ff"))
            try:
                for f in sorted(os.listdir(self._in)):
                    if f.lower().endswith((".wav", ".mp3")):
                        st = self._status.get(f, "Pendente")
                        color = "#3fb950" if "✅" in st else "#f85149" if "❌" in st else "#8b949e"
                        it = QTreeWidgetItem(root, [f, st])
                        it.setForeground(1, QColor(color))
                        it.setData(0, Qt.ItemDataRole.UserRole, os.path.join(self._in, f))
                root.setExpanded(True)
            except Exception as e:
                print(f"Erro ao ler diretório: {e}")'''

src = src.replace(OLD_EXP_REFRESH, NEW_EXP_REFRESH, 1)


OLD_INIT = '''        self.spn_temp.setValue(self.cfg.get("omni_temp", 0.0))
        self.spn_pad.setValue(self.cfg.get("pad_ms", 200))'''

NEW_INIT = '''        self.spn_temp.setValue(self.cfg.get("omni_temp", 0.0))
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
        self.exp.extract_request.connect(lambda p: self._extract())'''

src = src.replace(OLD_INIT, NEW_INIT, 1)

OLD_ON_DUB_DONE = '''    def _on_dub_done(self, ok, mov, path_out):
        if ok and path_out:
            self._current_result = path_out
            self.play_pt.load(path_out)
            self.play_pt.toggle_play()
            self.lbl_status.setText("✅ Dublagem de teste concluída! Ouça ou Salve.")
            self.lbl_status.setStyleSheet("color:#3fb950; font-weight:bold;")
        else:
            self.lbl_status.setText(f"❌ Falhou: {mov}")
            self.lbl_status.setStyleSheet("color:#f85149;")'''

NEW_ON_DUB_DONE = '''    def _on_dub_done(self, ok, mov, path_out, original_name):
        if ok and path_out:
            self._current_result = path_out
            self.play_pt.load(path_out)
            # Tocar automaticamente so se for arquivo unico ou o ultimo
            if getattr(self, '_last_dub_is_multi', False) == False:
                self.play_pt.toggle_play()
            self.lbl_status.setText(f"✅ {original_name} concluído! Ouça ou Salve.")
            self.lbl_status.setStyleSheet("color:#3fb950; font-weight:bold;")
        else:
            self.lbl_status.setText(f"❌ {original_name} Falhou: {mov}")
            self.lbl_status.setStyleSheet("color:#f85149;")'''

src = src.replace(OLD_ON_DUB_DONE, NEW_ON_DUB_DONE, 1)

OLD_RUN_DUB = '''    def _run_dub(self):
        if not self._current_file: return QMessageBox.warning(self, "Aviso", "Selecione um arquivo no explorador primeiro.")
        
        c_texts = {"en": self.txt_en.text(), "pt": self.txt_pt.text()}
        self.btn_dub.setEnabled(False)
        self.btn_trans.setEnabled(False)
        self.lbl_status.setText("⏳ Gerando dublagem...")
        self.lbl_status.setStyleSheet("color:#f0883e;")
        
        self.worker = SingleDubbingWorker(
            self._current_file, self.lne_guide.text(), self.models, c_texts,
            self.spn_temp.value(), self.spn_pad.value()
        )
        self.worker.log_signal.connect(self._log)
        self.worker.file_done_signal.connect(self._on_dub_done)
        self.worker.finished_signal.connect(lambda: (self.btn_dub.setEnabled(True), self.btn_trans.setEnabled(True)))
        self.worker.start()'''

NEW_RUN_DUB = '''    def _run_dub(self):
        if not self._current_file: return QMessageBox.warning(self, "Aviso", "Selecione um arquivo no explorador primeiro.")
        self._run_dub_multi([self._current_file])
        
    def _run_dub_multi(self, paths):
        if not paths: return
        self._last_dub_is_multi = (len(paths) > 1)
        
        c_texts = {"en": self.txt_en.text(), "pt": self.txt_pt.text()}
        self.btn_dub.setEnabled(False)
        self.btn_trans.setEnabled(False)
        self.lbl_status.setText(f"⏳ Processando {len(paths)} arquivo(s)...")
        self.lbl_status.setStyleSheet("color:#f0883e;")
        
        self.worker = SingleDubbingWorker(
            paths, self.lne_guide.text(), self.models, c_texts,
            self.spn_temp.value(), self.spn_pad.value()
        )
        self.worker.log_signal.connect(self._log)
        self.worker.file_done_signal.connect(self._on_dub_done)
        self.worker.finished_signal.connect(lambda: (self.btn_dub.setEnabled(True), self.btn_trans.setEnabled(True), self.lbl_status.setText("✅ Processo em fila concluído!")))
        self.worker.start()'''

src = src.replace(OLD_RUN_DUB, NEW_RUN_DUB, 1)

with open('DUBLAGEM_MASTER_PRO_v13_PRATICO.py', 'w', encoding='utf-8') as f:
    f.write(src)
print("Patcher run OK")
