#!/usr/bin/env python3
"""Injeta os players no tab_val usando o mesmo AudioPlayerWidget da interface principal."""
import sys, ast
sys.stdout = open(sys.stdout.fileno(), mode='w', encoding='utf-8', buffering=1)

path = "DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX.py"
content = open(path, encoding='utf-8').read()

# Localizar a criação do tab_val e adicionar inject_players logo após
OLD = (
    "        self.tab_val = ValidacaoWidget()\n"
    "        self.tab_val.redub_request.connect(self._redublar_de_validacao)\n"
)
NEW = (
    "        self.tab_val = ValidacaoWidget()\n"
    "        self.tab_val.redub_request.connect(self._redublar_de_validacao)\n"
    "        # Injetar players dedicados (AudioPlayerWidget ja definido neste modulo)\n"
    "        self._val_play_en = AudioPlayerWidget(\"🎵 Original EN\")\n"
    "        self._val_play_pt = AudioPlayerWidget(\"🔊 Dublado PT\")\n"
    "        self.tab_val.inject_players(self._val_play_en, self._val_play_pt)\n"
)

if OLD in content:
    content = content.replace(OLD, NEW, 1)
    print("OK players injetados")
else:
    print("BLOCO NAO ENCONTRADO")
    idx = content.find("tab_val = ValidacaoWidget")
    print(repr(content[idx:idx+200]))

open(path, 'w', encoding='utf-8').write(content)

try:
    ast.parse(open(path, encoding='utf-8').read())
    print("Sintaxe OK")
except SyntaxError as e:
    print(f"ERRO: {e}")
