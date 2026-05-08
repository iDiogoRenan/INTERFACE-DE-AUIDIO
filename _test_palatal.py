#!/usr/bin/env python3
"""Teste da palatização — expectativas corretas."""
import sys, os
sys.stdout = open(sys.stdout.fileno(), mode='w', encoding='utf-8', buffering=1)
sys.path.insert(0, os.path.dirname(__file__))
ffmpeg_dir = os.path.join(os.path.dirname(__file__), ".venv", "ffmpeg")
os.environ["PATH"] = ffmpeg_dir + os.pathsep + os.environ.get("PATH", "")
from _patch_accent_fix import palatalizar_ptbr

casos = [
    # (entrada, esperado, descricao)
    ('bati',      'bachi',             'ti final → chi (t removido)'),
    ('noite',     'noiche',            'te final → che'),
    ('parti',     'parchi',            'ti precedido de r'),
    ('pedi',      'pedchi',            'di final → dchi (d mantido)'),
    ('mode',      'modche',            'de final → dche'),
    ('paredes',   'paredches',         'des final → dches'),
    ('gente',     'genche',            'gente → genche (te final)'),
    ('diferente', 'diferenche',        'diferente → diferenche (te final)'),
    ('verdade',   'verdadche',         'verdade → verdadche (de final)'),
    # Palavras isoladas NAO devem ser alteradas
    ('de',  'de',   'de isolado — NAO altera'),
    ('te',  'te',   'te isolado — NAO altera'),
    ('di',  'di',   'di isolado — NAO altera'),
    ('ti',  'ti',   'ti isolado — NAO altera'),
    # Inicio de palavras (sem char antes do di/ti/te/de) NAO altera
    ('time',      'time',              'time — ti seguido de m (nao word boundary)'),
    ('ditado',    'ditado',            'ditado — di no inicio sem lookbehind'),
    # Frases
    ('eu te amo', 'eu te amo',         'te isolado na frase — NAO altera'),
    ('ele bati no pedi', 'ele bachi no pedchi', 'bati/pedi — altera'),
    ('noite de gente', 'noiche de genche', 'noite/gente alterados, de isolado nao'),
]

all_ok = True
for entrada, esperado, desc in casos:
    resultado = palatalizar_ptbr(entrada)
    ok = resultado == esperado
    if not ok:
        all_ok = False
    status = '[OK]   ' if ok else '[ERRO] '
    print(f'{status} {desc}')
    if not ok:
        print(f'         entrada:  "{entrada}"')
        print(f'         resultado: "{resultado}"')
        print(f'         esperado:  "{esperado}"')

print()
print('TODOS OK' if all_ok else 'FALHOU — verificar')
if not all_ok:
    sys.exit(1)
