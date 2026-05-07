#!/usr/bin/env python3
import sys, ast
sys.stdout = open(sys.stdout.fileno(), mode='w', encoding='utf-8', buffering=1)

path = "DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX.py"
content = open(path, encoding='utf-8').read()

# Corrigir AudioPlayerWidget -> AudioPlayer no bloco de injecao
content = content.replace(
    'self._val_play_en = AudioPlayerWidget(',
    'self._val_play_en = AudioPlayer(',
    1
)
content = content.replace(
    'self._val_play_pt = AudioPlayerWidget(',
    'self._val_play_pt = AudioPlayer(',
    1
)

open(path, 'w', encoding='utf-8').write(content)

try:
    ast.parse(open(path, encoding='utf-8').read())
    print('OK - AudioPlayer corrigido, Sintaxe OK')
except SyntaxError as e:
    print(f'ERRO: {e}')
