#!/usr/bin/env python3
"""Integra a aba de Validação no MainWindow."""
import sys, os, ast
sys.stdout = open(sys.stdout.fileno(), mode='w', encoding='utf-8', buffering=1)

path = "DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX.py"
content = open(path, encoding='utf-8').read()

# ─── 1. Adicionar import do ValidacaoWidget ───────────────────────────────────
OLD_IMP = 'from _patch_accent_fix import ('
NEW_IMP = 'from _patch_validacao import ValidacaoWidget\nfrom _patch_accent_fix import ('

if 'from _patch_validacao import' not in content:
    content = content.replace(OLD_IMP, NEW_IMP, 1)
    print('OK import ValidacaoWidget')
else:
    print('OK import ja existe')

# ─── 2. Adicionar QTabWidget ao import PyQt6 ─────────────────────────────────
if 'QTabWidget' not in content:
    content = content.replace(
        'QApplication, QMainWindow, QWidget, QVBoxLayout, QHBoxLayout,',
        'QApplication, QMainWindow, QWidget, QVBoxLayout, QHBoxLayout, QTabWidget,',
        1
    )
    print('OK QTabWidget adicionado ao import')
else:
    print('OK QTabWidget ja importado')

# ─── 3. Modificar _build_ui para usar QTabWidget ─────────────────────────────
OLD_BUILD = (
    '    def _build_ui(self):\n'
    '        cen = QWidget()\n'
    '        self.setCentralWidget(cen)\n'
    '        layout = QHBoxLayout(cen)\n'
    '\n'
    '        # ESQUERDA: Explorador\n'
    '        self.exp = FileExplorer()\n'
    '        layout.addWidget(self.exp, 1)\n'
    '\n'
    '        # CENTRO: Processamento\n'
    '        mid = QVBoxLayout()'
)

NEW_BUILD = (
    '    def _build_ui(self):\n'
    '        cen = QWidget()\n'
    '        self.setCentralWidget(cen)\n'
    '        root_layout = QHBoxLayout(cen)\n'
    '\n'
    '        # ESQUERDA: Explorador (sempre visível)\n'
    '        self.exp = FileExplorer()\n'
    '        root_layout.addWidget(self.exp, 1)\n'
    '\n'
    '        # ABAS: Dublagem | Validação\n'
    '        self.tabs = QTabWidget()\n'
    '        self.tabs.setStyleSheet(\n'
    '            "QTabWidget::pane { border:1px solid #30363d; }"\n'
    '            "QTabBar::tab { background:#161b22; color:#8b949e; padding:8px 18px; }"\n'
    '            "QTabBar::tab:selected { background:#0d1117; color:#c9d1d9; font-weight:bold; border-bottom:2px solid #1f6feb; }"\n'
    '        )\n'
    '\n'
    '        # Tab 1: Dublagem\n'
    '        tab_dub = QWidget()\n'
    '        layout = QHBoxLayout(tab_dub)\n'
    '        self.tabs.addTab(tab_dub, "🎙️  Dublagem")\n'
    '\n'
    '        # Tab 2: Validação\n'
    '        self.tab_val = ValidacaoWidget()\n'
    '        self.tab_val.redublar_signal.connect(self._redublar_de_validacao)\n'
    '        self.tabs.addTab(self.tab_val, "✅  Validação Manual")\n'
    '\n'
    '        root_layout.addWidget(self.tabs, 5)\n'
    '\n'
    '        # CENTRO: Processamento\n'
    '        mid = QVBoxLayout()'
)

if OLD_BUILD in content:
    content = content.replace(OLD_BUILD, NEW_BUILD, 1)
    print('OK _build_ui modificado com QTabWidget')
else:
    print('ERRO: bloco OLD_BUILD nao encontrado!')
    idx = content.find('def _build_ui(self):')
    print(repr(content[idx:idx+300]))

# ─── 4. Substituir layout.addWidget(self.exp, 1) que agora é do tab_dub ──────
# Já tratado acima — o explorador vai para root_layout

# ─── 5. Corrigir a linha que conecta ao layout right ─────────────────────────
# O layout.addLayout(right, 1) deve estar dentro do tab_dub — verificar se está OK
# A referência a `layout` agora aponta para o HBoxLayout do tab_dub

# ─── 6. Conectar quando muda de aba para atualizar pasta_dublados ────────────
OLD_CONNECT = '        self.exp.file_selected.connect(self._select_file)\n'
NEW_CONNECT = (
    '        self.exp.file_selected.connect(self._select_file)\n'
    '        self.tabs.currentChanged.connect(self._on_tab_mudou)\n'
)
if 'self._on_tab_mudou' not in content:
    content = content.replace(OLD_CONNECT, NEW_CONNECT, 1)
    print('OK tab changed conectado')
else:
    print('OK tab changed ja conectado')

# ─── 7. Adicionar métodos _redublar_de_validacao e _on_tab_mudou ─────────────
METHODS = '''
    def _on_tab_mudou(self, idx):
        """Quando entra na aba Validação, pré-preenche a pasta de dublados com a Destino."""
        if idx == 1:
            pasta_out = self.lne_out.text().strip()
            if pasta_out:
                self.tab_val.definir_pasta_dublados(pasta_out)

    def _redublar_de_validacao(self, caminho: str):
        """Recebe sinal da aba Validação para redublar um arquivo específico."""
        self.tabs.setCurrentIndex(0)  # volta para aba de dublagem
        self._current_file = caminho
        self._run_dub_multi([caminho])

'''

if '_redublar_de_validacao' not in content:
    content = content.replace('    def _on_tab_mudou', '##PLACEHOLDER##', 1) if '_on_tab_mudou' in content else content
    content = content.replace('    def _on_tab_mudou', '', 1)
    # Inserir antes de _run_tudo
    content = content.replace('    def _run_tudo(self):', METHODS + '    def _run_tudo(self):', 1)
    print('OK métodos _on_tab_mudou e _redublar_de_validacao adicionados')
else:
    print('OK métodos já existem')

# ─── 8. Salvar e validar ─────────────────────────────────────────────────────
open(path, 'w', encoding='utf-8').write(content)
print('Arquivo salvo')

try:
    ast.parse(open(path, encoding='utf-8').read())
    print('Sintaxe OK')
except SyntaxError as e:
    print(f'ERRO sintaxe: {e}')
