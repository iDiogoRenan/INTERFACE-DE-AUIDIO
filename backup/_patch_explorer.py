
# Patch: Explorador atualiza ao selecionar pasta + clique carrega arquivo no player/teste
import ast

with open('DUBLAGEM_MASTER_PRO_v11.py', 'r', encoding='utf-8') as f:
    src = f.read()

# ── 1. Conectar _lne_in ao explorador logo apos criar os campos de pasta ───
OLD_CONNECT = '''        self._lne_reject_folder.textChanged.connect(lambda v: self._set_cfg("reject_folder", v))
        center_layout.addWidget(grp_folders)'''

NEW_CONNECT = '''        self._lne_reject_folder.textChanged.connect(lambda v: self._set_cfg("reject_folder", v))

        # Atualizar explorador automaticamente ao mudar pasta de origem ou destino
        self._lne_in.textChanged.connect(self._on_folder_changed)
        self._lne_out.textChanged.connect(self._on_folder_changed)

        center_layout.addWidget(grp_folders)'''

if OLD_CONNECT in src:
    src = src.replace(OLD_CONNECT, NEW_CONNECT, 1)
    print("OK: conexao de pasta adicionada")
else:
    print("ERRO: OLD_CONNECT nao encontrado")

# ── 2. Melhorar o sinal file_selected do explorador para carregar player ──
# Atualmente _on_file_selected so carrega no player_en. Precisamos que tambem
# defina o arquivo como teste e habilite o botao de dublar.

OLD_FILE_SEL = '''    def _on_file_selected(self, path: str):
        self.player_en.load(path)
        info = get_audio_info(path)
        if info:
            ch = "Stereo" if info.get("channels", 1) > 1 else "Mono"
            self.status_bar.showMessage(
                f"{os.path.basename(path)}  |  {ch}  |  {info.get('sr')}Hz  |  {info.get('duration', 0):.2f}s"
            )'''

NEW_FILE_SEL = '''    def _on_folder_changed(self, _=None):
        """Atualiza explorador sempre que as pastas de origem/destino mudam."""
        p_in  = self._lne_in.text().strip()
        p_out = self._lne_out.text().strip()
        if p_in and os.path.isdir(p_in):
            self.explorer.set_folders(p_in, p_out if p_out and os.path.isdir(p_out) else p_in)

    def _on_file_selected(self, path: str):
        """Chamado ao clicar (simples) em arquivo no explorador."""
        self.player_en.load(path)
        info = get_audio_info(path)
        if info:
            ch = "Stereo" if info.get("channels", 1) > 1 else "Mono"
            self.status_bar.showMessage(
                f"{os.path.basename(path)}  |  {ch}  |  {info.get('sr')}Hz  |  {info.get('duration', 0):.2f}s"
            )
        # Adicionar automaticamente a lista de teste se ainda nao estiver
        if path not in self._test_files:
            self._test_files.append(path)
            self._test_results[path] = {"pt": "", "es": ""}
            from PyQt6.QtWidgets import QListWidgetItem
            from PyQt6.QtGui import QColor
            item = QListWidgetItem(os.path.basename(path))
            item.setToolTip(path)
            item.setForeground(QColor("#8b949e"))
            self._test_list.addItem(item)

        # Selecionar na lista de teste para ficar sincronizado
        idx = self._test_files.index(path)
        self._test_list.setCurrentRow(idx)
        self._update_nav_buttons(idx)

        # Se ja existir resultado PT para este arquivo, carregar no player
        res = self._test_results.get(path, {})
        if res.get("pt") and os.path.exists(res["pt"]):
            self.player_pt.load(res["pt"])
        if res.get("es") and os.path.exists(res["es"]):
            self.player_es.load(res["es"])

        self.lbl_test_status.setText(
            f"Arquivo: {os.path.basename(path)} — clique em 'Dublar Selecionados' para testar.")
        self.lbl_test_status.setStyleSheet("color:#f0883e; font-size:11px;")'''

if OLD_FILE_SEL in src:
    src = src.replace(OLD_FILE_SEL, NEW_FILE_SEL, 1)
    print("OK: _on_file_selected atualizado")
else:
    print("ERRO: OLD_FILE_SEL nao encontrado")
    idx = src.find("def _on_file_selected")
    print(f"  Encontrado em: {idx}")

# ── 3. Melhorar FileExplorer: duplo clique ja inicia teste rapido ──────────
OLD_DBL = '''    def _on_dbl(self, item: QTreeWidgetItem, col: int):
        path = item.data(0, Qt.ItemDataRole.UserRole)
        if path and os.path.isfile(path):
            self.file_selected.emit(path)'''

NEW_DBL = '''    def _on_dbl(self, item: QTreeWidgetItem, col: int):
        path = item.data(0, Qt.ItemDataRole.UserRole)
        if path and os.path.isfile(path):
            self.file_selected.emit(path)

    def _on_click(self, item: QTreeWidgetItem, col: int):
        """Clique simples tambem emite o sinal para carregar no player."""
        path = item.data(0, Qt.ItemDataRole.UserRole)
        if path and os.path.isfile(path):
            self.file_selected.emit(path)'''

if OLD_DBL in src:
    src = src.replace(OLD_DBL, NEW_DBL, 1)
    print("OK: _on_click adicionado ao explorador")
else:
    print("ERRO: OLD_DBL nao encontrado")

# ── 4. Conectar itemClicked (clique simples) no explorador ────────────────
OLD_TREE_CONNECT = '''        self.tree.setHeaderLabels(["Arquivo", "Status"])
        self.tree.setColumnWidth(0, 170)
        self.tree.itemDoubleClicked.connect(self._on_dbl)'''

NEW_TREE_CONNECT = '''        self.tree.setHeaderLabels(["Arquivo", "Status"])
        self.tree.setColumnWidth(0, 170)
        self.tree.itemClicked.connect(self._on_click)
        self.tree.itemDoubleClicked.connect(self._on_dbl)'''

if OLD_TREE_CONNECT in src:
    src = src.replace(OLD_TREE_CONNECT, NEW_TREE_CONNECT, 1)
    print("OK: itemClicked conectado")
else:
    print("ERRO: OLD_TREE_CONNECT nao encontrado")

# ── 5. Restaurar pasta de origem ao iniciar (carregar ultima pasta usada) ──
OLD_BUILD_END = '''        # Detectar GPU
        QTimer.singleShot(100, self._detect_gpu)'''

NEW_BUILD_END = '''        # Detectar GPU
        QTimer.singleShot(100, self._detect_gpu)

        # Restaurar ultima pasta de origem usada e ja popular explorador
        last_in  = self.cfg.get("last_input_folder", "")
        last_out = self.cfg.get("last_output_folder", "")
        if last_in and os.path.isdir(last_in):
            self._lne_in.setText(last_in)
        if last_out and os.path.isdir(last_out):
            self._lne_out.setText(last_out)
        last_guide = self.cfg.get("last_guide_file", "")
        if last_guide and os.path.exists(last_guide):
            self._lne_guide.setText(last_guide)'''

if OLD_BUILD_END in src:
    src = src.replace(OLD_BUILD_END, NEW_BUILD_END, 1)
    print("OK: restauracao de pasta adicionada")
else:
    print("ERRO: OLD_BUILD_END nao encontrado")

with open('DUBLAGEM_MASTER_PRO_v11.py', 'w', encoding='utf-8') as f:
    f.write(src)
print("Arquivo salvo!")

try:
    ast.parse(src)
    print(f"Sintaxe OK! Linhas: {src.count(chr(10))}")
except SyntaxError as e:
    print(f"ERRO SINTAXE: {e}")
