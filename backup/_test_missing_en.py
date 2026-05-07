import sys, os
from PyQt6.QtWidgets import QApplication
from PyQt6.QtCore import Qt

# Fake the UI
os.environ["QT_QPA_PLATFORM"] = "offscreen"
app = QApplication(sys.argv)

import DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX as main_mod
win = main_mod.MainWindow()
val = win.tab_val

import tempfile, shutil
tmp_en = tempfile.mkdtemp()
tmp_pt = tempfile.mkdtemp()
tmp_out = tempfile.mkdtemp()

# EN: audio1.wav, audio2.wav
open(os.path.join(tmp_en, "audio1.wav"), "wb").write(b"RIFF" + b"\x00"*40)
open(os.path.join(tmp_en, "audio2.wav"), "wb").write(b"RIFF" + b"\x00"*40)

# PT: audio1.wav, audio3.wav (audio3 não tem EN, audio2 não tem PT)
open(os.path.join(tmp_pt, "audio1.wav"), "wb").write(b"RIFF" + b"\x00"*40)
open(os.path.join(tmp_pt, "audio3.wav"), "wb").write(b"RIFF" + b"\x00"*40)

val.definir_pastas(tmp_en, tmp_pt)
val.lne_final.setText(tmp_out)

val._carregar_lista()
print(f"Arquivos na lista: {val.lista.count()} (deve ser 2: audio1, audio3)")
assert val.lista.count() == 2

# Selecionar audio1 (Tem EN e PT)
val.lista.setCurrentRow(0)
print(f"Selecionado audio1: EN={val._arquivo_en}, PT={val._arquivo_pt}")
assert "audio1" in val._arquivo_en
assert "audio1" in val._arquivo_pt
assert val._player_en._path != ""

# Selecionar audio3 (NÃO TEM EN)
val.lista.setCurrentRow(1)
print(f"Selecionado audio3: EN={val._arquivo_en}, PT={val._arquivo_pt}")
assert val._arquivo_en == ""
assert "audio3" in val._arquivo_pt
assert val._player_en._path == "" # Deve ter sido limpo
print(f"Label Status: {val.lbl_status.text()}")
assert "não encontrado" in val.lbl_status.text()

shutil.rmtree(tmp_en)
shutil.rmtree(tmp_pt)
shutil.rmtree(tmp_out)
print("TESTE PASSOU: Faltando EN esvazia o player corretamente e avisa o usuario!")
