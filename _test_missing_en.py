import sys, os, wave
from PyQt6.QtWidgets import QApplication
from PyQt6.QtCore import Qt

# Fake the UI
os.environ["QT_QPA_PLATFORM"] = "offscreen"
ffmpeg_dir = os.path.join(os.path.dirname(__file__), ".venv", "ffmpeg")
os.environ["PATH"] = ffmpeg_dir + os.pathsep + os.environ.get("PATH", "")
sys.stdout = open(sys.stdout.fileno(), mode='w', encoding='utf-8', buffering=1)
app = QApplication(sys.argv)

import DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX as main_mod
win = main_mod.MainWindow()
val = win.tab_val
for player in (getattr(val, "_player_en", None), getattr(val, "_player_pt", None)):
    if player:
        player.load = lambda path, player=player: setattr(player, "_path", path)
        player.clear = lambda _msg="", player=player: setattr(player, "_path", "")
        player.stop = lambda: None

import tempfile, shutil
tmp_en = tempfile.mkdtemp()
tmp_pt = tempfile.mkdtemp()
tmp_out = tempfile.mkdtemp()


def write_silent_wav(path: str) -> None:
    with wave.open(path, "wb") as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(16000)
        audio.writeframes(b"\x00" * 32000)


# EN: audio1.wav, audio2.wav
write_silent_wav(os.path.join(tmp_en, "audio1.wav"))
write_silent_wav(os.path.join(tmp_en, "audio2.wav"))

# PT: audio1.wav, audio3.wav (audio3 não tem EN, audio2 não tem PT)
write_silent_wav(os.path.join(tmp_pt, "audio1.wav"))
write_silent_wav(os.path.join(tmp_pt, "audio3.wav"))

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

for player in (getattr(val, "_player_en", None), getattr(val, "_player_pt", None)):
    if player:
        player.clear()
app.processEvents()

shutil.rmtree(tmp_en, ignore_errors=True)
shutil.rmtree(tmp_pt, ignore_errors=True)
shutil.rmtree(tmp_out, ignore_errors=True)
print("TESTE PASSOU: Faltando EN esvazia o player corretamente e avisa o usuario!")
