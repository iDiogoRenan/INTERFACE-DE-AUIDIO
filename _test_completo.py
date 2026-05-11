#!/usr/bin/env python3
"""Testes completos: UI principal + aba de Validação."""
import sys, os, tempfile, shutil, wave
os.environ["QT_QPA_PLATFORM"] = "offscreen"
sys.path.insert(0, os.path.dirname(__file__))
ffmpeg_dir = os.path.join(os.path.dirname(__file__), ".venv", "ffmpeg")
os.environ["PATH"] = ffmpeg_dir + os.pathsep + os.environ.get("PATH", "")
cfg_fd, cfg_path = tempfile.mkstemp(suffix=".json")
os.close(cfg_fd)
os.unlink(cfg_path)
os.environ["DUBLAGEM_MASTER_CONFIG"] = cfg_path
sys.stdout = open(sys.stdout.fileno(), mode='w', encoding='utf-8', buffering=1)

from PyQt6.QtWidgets import QApplication
from PyQt6.QtCore import Qt
import unittest.mock as mock
app = QApplication(sys.argv)

ok = True
tmpfinal = ""
blocked_final = ""


def write_silent_wav(path: str) -> None:
    with wave.open(path, "wb") as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(16000)
        audio.writeframes(b"\x00" * 3200)

# ─── Carregar MainWindow ──────────────────────────────────────────────────────
try:
    import importlib.util
    spec = importlib.util.spec_from_file_location("main_mod",
        os.path.join(os.path.dirname(__file__), "DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    win = mod.MainWindow()
    print("1. [OK] MainWindow instanciada")
except Exception as e:
    print(f"1. [ERRO] {e}")
    import traceback; traceback.print_exc()
    sys.exit(1)

# ─── Testes da UI principal ───────────────────────────────────────────────────
for attr, desc in [
    ('btn_tudo',     'btn_tudo existe'),
    ('btn_cancelar', 'btn_cancelar existe'),
    ('tabs',         'QTabWidget existe'),
    ('tab_val',      'tab_val (ValidacaoWidget) existe'),
]:
    try:
        assert hasattr(win, attr), f"Faltando: {attr}"
        print(f"2. [OK] {desc}")
    except AssertionError as e:
        print(f"2. [ERRO] {e}"); ok = False

# ─── Testes do QTabWidget ─────────────────────────────────────────────────────
try:
    assert win.tabs.count() == 2, f"Esperado 2 abas, got {win.tabs.count()}"
    assert "Dublagem" in win.tabs.tabText(0)
    assert "Validação" in win.tabs.tabText(1)
    print("3. [OK] 2 abas: Dublagem + Validação")
except AssertionError as e:
    print(f"3. [ERRO] {e}"); ok = False

# ─── Testes do ValidacaoWidget ────────────────────────────────────────────────
val = win.tab_val
val.set_quality_checker(lambda _path, _text, _lang: (True, "QC fake OK"))
for injected_player in (getattr(val, "_player_en", None), getattr(val, "_player_pt", None)):
    if injected_player:
        injected_player.load = lambda _path: None
        injected_player.clear = lambda _msg="": None
        injected_player.toggle_play = lambda: None
try:
    for attr in ['lista', 'btn_aprovar', 'btn_rejeitar', 'btn_redub', 'btn_proximo',
                 'lne_dublados', 'lne_final', 'lbl_stats', 'txt_pt',
                 'cmb_modo', 'chk_palatalizar', 'chk_virgula', 'chk_trailing']:
        assert hasattr(val, attr), f"ValidacaoWidget faltando: {attr}"
    print("4. [OK] ValidacaoWidget tem todos os atributos")
except AssertionError as e:
    print(f"4. [ERRO] {e}"); ok = False

# ─── Teste: botões de ação começam desabilitados ──────────────────────────────
try:
    assert not val.btn_aprovar.isEnabled()
    assert not val.btn_rejeitar.isEnabled()
    assert not val.btn_redub.isEnabled()
    print("5. [OK] Botões de ação desabilitados até selecionar arquivo")
except AssertionError as e:
    print(f"5. [ERRO] {e}"); ok = False

# ─── Teste: carregar lista com pasta inválida → warning ──────────────────────
try:
    with mock.patch('PyQt6.QtWidgets.QMessageBox.warning', return_value=None):
        val._carregar_lista()  # pasta vazia → warning, sem crash
    print("6. [OK] _carregar_lista com pasta vazia: warning sem crash")
except Exception as e:
    print(f"6. [ERRO] {e}"); ok = False

# ─── Teste: carregar lista com arquivos reais ─────────────────────────────────
try:
    tmpdir = tempfile.mkdtemp()
    for nome in ["audio_001.wav", "audio_002.wav", "audio_003.wav"]:
        write_silent_wav(os.path.join(tmpdir, nome))
    val.definir_pasta_dublados(tmpdir)
    val._carregar_lista()
    assert val.lista.count() == 3, f"Esperado 3 itens, got {val.lista.count()}"
    print(f"7. [OK] Lista carregada: {val.lista.count()} arquivos")
except Exception as e:
    print(f"7. [ERRO] {e}"); ok = False

# ─── Teste: aprovar sem pasta final → warning ────────────────────────────────
try:
    val.lista.setCurrentRow(0)
    val._arquivo_pt = os.path.join(tmpdir, "audio_001.wav")
    with mock.patch('PyQt6.QtWidgets.QMessageBox.warning', return_value=None):
        val._aprovar()  # sem pasta final → warning
    print("8. [OK] _aprovar sem pasta final: warning sem crash")
except Exception as e:
    print(f"8. [ERRO] {e}"); ok = False

# ─── Teste: aprovar com pasta final válida ────────────────────────────────────
try:
    tmpfinal = tempfile.mkdtemp()
    val._pasta_final = tmpfinal
    val.lne_final.setText(tmpfinal)
    val.lista.setCurrentRow(0)
    val._arquivo_pt = os.path.join(tmpdir, "audio_001.wav")
    val._aprovar()
    assert os.path.exists(os.path.join(tmpfinal, "audio_001.wav"))
    item = val.lista.item(0)
    assert item.data(Qt.ItemDataRole.UserRole + 1) == "aprovado"
    print("9. [OK] _aprovar copia arquivo e marca como aprovado")
except Exception as e:
    print(f"9. [ERRO] {e}"); ok = False

try:
    blocked_final = tempfile.mkdtemp()
    val.set_quality_checker(lambda _path, _text, _lang: (False, "Tri-checagem SEPRAR_AUDIOS reprovou: teste"))
    val.lne_final.setText(blocked_final)
    val.lista.setCurrentRow(2)
    val._arquivo_pt = os.path.join(tmpdir, "audio_003.wav")
    with mock.patch('PyQt6.QtWidgets.QMessageBox.warning', return_value=None):
        val._aprovar()
    assert not os.path.exists(os.path.join(blocked_final, "audio_003.wav"))
    assert val.lista.item(2).data(Qt.ItemDataRole.UserRole + 1) == "rejeitado"
    val.set_quality_checker(lambda _path, _text, _lang: (True, "QC fake OK"))
    print("10. [OK] _aprovar bloqueia quando o controle de qualidade reprova")
except Exception as e:
    print(f"10. [ERRO] {e}"); ok = False

# ─── Teste: rejeitar arquivo ─────────────────────────────────────────────────
try:
    val.lista.setCurrentRow(1)
    val._arquivo_pt = os.path.join(tmpdir, "audio_002.wav")
    val._rejeitar()
    item = val.lista.item(1)
    assert item.data(Qt.ItemDataRole.UserRole + 1) == "rejeitado"
    print("11. [OK] _rejeitar marca como rejeitado")
except Exception as e:
    print(f"11. [ERRO] {e}"); ok = False

# ─── Teste: estatísticas ──────────────────────────────────────────────────────
try:
    val._atualizar_stats()
    stats = val.lbl_stats.text()
    assert "1" in stats  # 1 aprovado
    print(f"12. [OK] Estatísticas atualizadas: {stats}")
except Exception as e:
    print(f"12. [ERRO] {e}"); ok = False

# ─── Teste: próximo navega corretamente ──────────────────────────────────────
try:
    val.lista.setCurrentRow(0)
    val._proximo()
    assert val.lista.currentRow() == 1
    print("13. [OK] _proximo navega para próximo item")
except Exception as e:
    print(f"13. [ERRO] {e}"); ok = False

# ─── Teste: _on_tab_mudou preenche pasta ─────────────────────────────────────
try:
    win.lne_out.setText(tmpdir)
    win._on_tab_mudou(1)
    assert val.lne_dublados.text() == tmpdir
    print("14. [OK] _on_tab_mudou preenche pasta de dublados automaticamente")
except Exception as e:
    print(f"14. [ERRO] {e}"); ok = False

# ─── Teste: _redublar_de_validacao muda para aba 0 ───────────────────────────
try:
    # Mockar SingleDubbingWorkerV14 para nao tentar rodar GPU
    import _patch_accent_fix as paf
    class FakeSignal:
        def connect(self, _slot): pass

    class FakeWorker:
        progress_signal = FakeSignal()
        file_done_signal = FakeSignal()
        log_signal = FakeSignal()
        finished = FakeSignal()
        def start(self): pass
        def deleteLater(self): pass
    with mock.patch.object(paf, 'SingleDubbingWorkerV14', return_value=FakeWorker()):
        win._redublar_de_validacao(
            os.path.join(tmpdir, "audio_003.wav"), "", "classico", False, False, False, 200
        )
    # O worker foi criado e iniciado sem crash — nao muda de aba (fica na validacao)
    assert hasattr(win, '_val_worker')
    print("15. [OK] _redublar_de_validacao: worker criado sem crash")
except Exception as e:
    import traceback
    print(f"15. [ERRO] {e}")
    traceback.print_exc()
    ok = False

try:
    from ui_language import current_language_code
    assert current_language_code(win.cmb_source_lang, "auto") == "auto"
    assert current_language_code(win.cmb_target_lang, "pt") == "pt"
    assert current_language_code(val.cmb_source_lang, "auto") == "auto"
    assert current_language_code(val.cmb_target_lang, "pt") == "pt"
    print("16. [OK] Combos de idioma usam currentData com fallback seguro")
except Exception as e:
    print(f"16. [ERRO] {e}"); ok = False

# ─── Cleanup ──────────────────────────────────────────────────────────────────
shutil.rmtree(tmpdir, ignore_errors=True)
shutil.rmtree(tmpfinal, ignore_errors=True)
shutil.rmtree(blocked_final, ignore_errors=True)
try:
    os.remove(cfg_path)
except FileNotFoundError:
    pass

print()
if ok:
    print("=== TODOS OS 16 TESTES PASSARAM ===")
else:
    print("=== FALHOU ===")
    sys.exit(1)
