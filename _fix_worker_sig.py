#!/usr/bin/env python3
import sys, ast
sys.stdout = open(sys.stdout.fileno(), mode='w', encoding='utf-8', buffering=1)

path = "DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX.py"
lines = open(path, encoding='utf-8').readlines()

# Encontrar o bloco errado e substituir
old_block = (
    '        from _patch_accent_fix import SingleDubbingWorkerV14\n'
    '        self._val_worker = SingleDubbingWorkerV14(\n'
    '            paths_en=[src],\n'
    '            models=getattr(self, \'_shared_models\', {"whisper": None, "omni": None}),\n'
    '            pasta_guia="",\n'
    '            modo_voz=modo,\n'
    '            custom_texts={"pt": texto_pt} if texto_pt else {},\n'
    '            palatalizar=palatalizar,\n'
    '            virgula_interrogacao=virgula,\n'
    '            trailing_ponto=trailing,\n'
    '            pad_ms=pad_ms,\n'
    '        )\n'
)

new_block = (
    '        from _patch_accent_fix import SingleDubbingWorkerV14\n'
    '        shared = getattr(self, \'_shared_models\', {"whisper": None, "omni": None})\n'
    '        self._val_worker = SingleDubbingWorkerV14(\n'
    '            paths_en=[src],\n'
    '            pasta_guia="",\n'
    '            models_ref=shared,\n'
    '            custom_texts={"pt": texto_pt} if texto_pt else {},\n'
    '            omni_temp=0.05,\n'
    '            pad_ms=pad_ms,\n'
    '            modo_voz=modo,\n'
    '            palatalizar=palatalizar,\n'
    '            virgula_interrogacao=virgula,\n'
    '            trailing_ponto=trailing,\n'
    '        )\n'
)

content = open(path, encoding='utf-8').read()
if old_block in content:
    content = content.replace(old_block, new_block, 1)
    open(path, 'w', encoding='utf-8').write(content)
    print('OK substituído')
else:
    print('BLOCO NAO ENCONTRADO — verificando contexto...')
    idx = content.find('_val_worker = SingleDubbingWorkerV14')
    print(repr(content[idx-30:idx+400]))

try:
    ast.parse(open(path, encoding='utf-8').read())
    print('Sintaxe OK')
except SyntaxError as e:
    print(f'Erro: {e}')
