#!/usr/bin/env python3
# patch_add_btn_tudo.py
import sys, os
sys.stdout = open(sys.stdout.fileno(), mode='w', encoding='utf-8', buffering=1)

path = "DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX.py"
content = open(path, encoding="utf-8").read()

# ─── 1. Substituir o bloco de botões na UI ───────────────────────────────────
OLD_UI = (
    '        # 4. Ações\n'
    '        grp_act = QGroupBox("🚀 Ações de Dublagem")\n'
    '        al = QVBoxLayout(grp_act)\n'
    '        \n'
    '        row_b = QHBoxLayout()\n'
    '        self.btn_dub = QPushButton("🎙️ Dublar Arquivo Atual")\n'
    '        self.btn_dub.setStyleSheet("background:#5a3e00; color:#f0883e; font-size:14px; font-weight:bold; padding:10px;")\n'
    '        self.btn_dub.clicked.connect(self._run_dub)\n'
    '        row_b.addWidget(self.btn_dub)\n'
    '        \n'
    '        self.btn_save = QPushButton("💾 Salvar na Pasta Destino")\n'
    '        self.btn_save.setStyleSheet("background:#238636; font-size:14px; font-weight:bold; padding:10px;")\n'
    '        self.btn_save.clicked.connect(self._save_result)\n'
    '        row_b.addWidget(self.btn_save)\n'
    '        al.addLayout(row_b)\n'
    '        \n'
    '        self.lbl_status = QLabel("Selecione um arquivo no explorador à esquerda.")\n'
    '        self.lbl_status.setAlignment(Qt.AlignmentFlag.AlignCenter)\n'
    '        al.addWidget(self.lbl_status)\n'
    '        \n'
    '        self.prog_bar = QProgressBar()\n'
    '        self.prog_bar.setRange(0, 100)\n'
    '        self.prog_bar.setValue(0)\n'
    '        self.prog_bar.setTextVisible(True)\n'
    '        self.prog_bar.setFixedHeight(12)\n'
    '        self.prog_bar.setStyleSheet("QProgressBar { font-size: 8px; font-weight: bold; }")\n'
    '        al.addWidget(self.prog_bar)\n'
    '        \n'
    '        mid.addWidget(grp_act)'
)

NEW_UI = (
    '        # 4. Ações\n'
    '        grp_act = QGroupBox("🚀 Ações de Dublagem")\n'
    '        al = QVBoxLayout(grp_act)\n'
    '\n'
    '        # Botão principal DUBLAR TUDO\n'
    '        self.btn_tudo = QPushButton("🚀  DUBLAR TUDO  (Auto-Salvar na Pasta Destino)")\n'
    '        self.btn_tudo.setStyleSheet(\n'
    '            "background:#1f6feb; color:white; font-size:15px; font-weight:bold;"\n'
    '            " padding:14px; border-radius:6px;"\n'
    '        )\n'
    '        self.btn_tudo.setToolTip("Dubla TODOS os arquivos da pasta Origem e salva automaticamente na pasta Destino.")\n'
    '        self.btn_tudo.clicked.connect(self._run_tudo)\n'
    '        al.addWidget(self.btn_tudo)\n'
    '\n'
    '        row_b = QHBoxLayout()\n'
    '        self.btn_dub = QPushButton("🎙️ Dublar Arquivo Atual")\n'
    '        self.btn_dub.setStyleSheet("background:#5a3e00; color:#f0883e; font-size:13px; font-weight:bold; padding:8px;")\n'
    '        self.btn_dub.clicked.connect(self._run_dub)\n'
    '        row_b.addWidget(self.btn_dub)\n'
    '\n'
    '        self.btn_save = QPushButton("💾 Salvar na Pasta Destino")\n'
    '        self.btn_save.setStyleSheet("background:#238636; font-size:13px; font-weight:bold; padding:8px;")\n'
    '        self.btn_save.clicked.connect(self._save_result)\n'
    '        row_b.addWidget(self.btn_save)\n'
    '\n'
    '        self.btn_cancelar = QPushButton("⏹ Cancelar")\n'
    '        self.btn_cancelar.setStyleSheet("background:#6e1a1a; color:#ff9090; font-size:13px; font-weight:bold; padding:8px;")\n'
    '        self.btn_cancelar.setEnabled(False)\n'
    '        self.btn_cancelar.clicked.connect(self._cancelar)\n'
    '        row_b.addWidget(self.btn_cancelar)\n'
    '        al.addLayout(row_b)\n'
    '\n'
    '        self.lbl_status = QLabel("Selecione um arquivo ou clique em DUBLAR TUDO.")\n'
    '        self.lbl_status.setAlignment(Qt.AlignmentFlag.AlignCenter)\n'
    '        al.addWidget(self.lbl_status)\n'
    '\n'
    '        self.prog_bar = QProgressBar()\n'
    '        self.prog_bar.setRange(0, 100)\n'
    '        self.prog_bar.setValue(0)\n'
    '        self.prog_bar.setTextVisible(True)\n'
    '        self.prog_bar.setFixedHeight(14)\n'
    '        self.prog_bar.setStyleSheet("QProgressBar { font-size: 9px; font-weight: bold; }")\n'
    '        al.addWidget(self.prog_bar)\n'
    '\n'
    '        mid.addWidget(grp_act)'
)

if OLD_UI in content:
    content = content.replace(OLD_UI, NEW_UI, 1)
    print("✅ Bloco UI substituído.")
else:
    print("❌ Bloco OLD_UI não encontrado — verificar manualmente.")
    # mostra trecho para debug
    idx = content.find('# 4. Ações')
    print(repr(content[idx:idx+300]))

# ─── 2. Adicionar métodos se não existirem ───────────────────────────────────
METHODS = '''
    def _run_tudo(self):
        """Dubla TODOS os arquivos da pasta de entrada e salva na saída automaticamente."""
        pasta_in  = self.lne_in.text().strip()
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
        self._log(f"🚀 Iniciando batch: {len(arquivos)} arquivo(s) → {pasta_out}", "info")
        self._run_dub_multi(arquivos)

    def _cancelar(self):
        if self.worker and self.worker.isRunning():
            self.worker.terminate()
            self.worker.wait(2000)
            self._log("⏹ Cancelado pelo usuário.", "warning")
            self.lbl_status.setText("⏹ Cancelado.")
            self.lbl_status.setStyleSheet("color:#d29922;")
            self._set_botoes_ativos(True)

    def _set_botoes_ativos(self, ativo: bool):
        self.btn_tudo.setEnabled(ativo)
        self.btn_dub.setEnabled(ativo)
        self.btn_trans.setEnabled(ativo)
        self.btn_redub.setEnabled(ativo)
        self.btn_cancelar.setEnabled(not ativo)

'''

if '_run_tudo' not in content:
    # Inserir antes de _run_dub
    content = content.replace('    def _run_dub(self):', METHODS + '    def _run_dub(self):', 1)
    print("✅ Métodos _run_tudo/_cancelar/_set_botoes_ativos adicionados.")
else:
    print("✅ Métodos já existem.")

# ─── 3. Atualizar _run_dub_multi para usar _set_botoes_ativos ────────────────
OLD_DISABLE = (
    '        self.btn_dub.setEnabled(False)\n'
    '        self.btn_trans.setEnabled(False)\n'
    '        self.btn_redub.setEnabled(False)\n'
)
NEW_DISABLE = '        self._set_botoes_ativos(False)\n'
if OLD_DISABLE in content:
    content = content.replace(OLD_DISABLE, NEW_DISABLE, 1)
    print("✅ Disable substituído por _set_botoes_ativos.")

OLD_ENABLE = (
    '            self.btn_dub.setEnabled(True),\n'
    '            self.btn_trans.setEnabled(True),\n'
    '            self.btn_redub.setEnabled(True),\n'
)
NEW_ENABLE = '            self._set_botoes_ativos(True),\n'
if OLD_ENABLE in content:
    content = content.replace(OLD_ENABLE, NEW_ENABLE, 1)
    print("✅ Enable substituído por _set_botoes_ativos.")

# ─── 4. Salvar ────────────────────────────────────────────────────────────────
open(path, "w", encoding="utf-8").write(content)
print("💾 Arquivo salvo.")

# ─── 5. Verificar sintaxe ────────────────────────────────────────────────────
import ast
try:
    ast.parse(open(path, encoding="utf-8").read())
    print("✅ Sintaxe OK!")
except SyntaxError as e:
    print(f"❌ Erro de sintaxe: {e}")
