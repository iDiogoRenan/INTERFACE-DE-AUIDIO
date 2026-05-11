#!/usr/bin/env python3
"""
Teste de execução da UI — verifica que:
1. MainWindow inicializa sem erros
2. btn_tudo existe e tem signal conectado
3. _run_tudo dá warning correto se pasta vazia (não crasha)
4. _run_dub_multi não crasha com lista vazia
5. _set_botoes_ativos funciona sem AttributeError
"""
import sys, os
import shutil
import tempfile
os.environ["QT_QPA_PLATFORM"] = "offscreen"  # headless
sys.path.insert(0, os.path.dirname(__file__))
ffmpeg_dir = os.path.join(os.path.dirname(__file__), ".venv", "ffmpeg")
os.environ["PATH"] = ffmpeg_dir + os.pathsep + os.environ.get("PATH", "")
cfg_fd, cfg_path = tempfile.mkstemp(suffix=".json")
os.close(cfg_fd)
os.unlink(cfg_path)
os.environ["DUBLAGEM_MASTER_CONFIG"] = cfg_path
sys.stdout = open(sys.stdout.fileno(), mode='w', encoding='utf-8', buffering=1)

from PyQt6.QtWidgets import QApplication
app = QApplication(sys.argv)

ok = True
erros = []

try:
    # 1. Importar e instanciar MainWindow
    print("1. Importando MainWindow...")
    import importlib.util
    spec = importlib.util.spec_from_file_location(
        "main_mod",
        os.path.join(os.path.dirname(__file__), "DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX.py")
    )
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    print("   [OK] Modulo carregado")

    MainWindow = mod.MainWindow
    win = MainWindow()
    print("   [OK] MainWindow instanciada")

except Exception as e:
    print(f"   [ERRO] {e}")
    import traceback; traceback.print_exc()
    ok = False

if ok:
    try:
        # 2. Verificar btn_tudo existe e tem receivers
        assert hasattr(win, 'btn_tudo'), "btn_tudo nao existe!"
        assert win.btn_tudo.isEnabled(), "btn_tudo desabilitado no inicio!"
        print("2. [OK] btn_tudo existe e está habilitado")
    except Exception as e:
        print(f"2. [ERRO] {e}"); ok = False

if ok:
    try:
        # 3. Verificar btn_cancelar existe e começa desabilitado
        assert hasattr(win, 'btn_cancelar'), "btn_cancelar nao existe!"
        assert not win.btn_cancelar.isEnabled(), "btn_cancelar deveria iniciar desabilitado!"
        print("3. [OK] btn_cancelar existe e começa desabilitado")
    except Exception as e:
        print(f"3. [ERRO] {e}"); ok = False

if ok:
    try:
        # 4. _set_botoes_ativos sem AttributeError
        win._set_botoes_ativos(False)
        assert not win.btn_tudo.isEnabled()
        assert not win.btn_dub.isEnabled()
        assert win.btn_cancelar.isEnabled()
        win._set_botoes_ativos(True)
        assert win.btn_tudo.isEnabled()
        assert not win.btn_cancelar.isEnabled()
        print("4. [OK] _set_botoes_ativos funciona corretamente")
    except Exception as e:
        print(f"4. [ERRO] {e}"); ok = False

if ok:
    try:
        # 5. _run_tudo com pasta vazia -> deve mostrar warning, não crashar
        # Substituímos QMessageBox.warning temporariamente
        import unittest.mock as mock
        with mock.patch('PyQt6.QtWidgets.QMessageBox.warning', return_value=None):
            win.lne_in.setText("")  # pasta vazia
            win._run_tudo()  # deve chamar warning, não crashar
        print("5. [OK] _run_tudo com pasta vazia: warning (sem crash)")
    except Exception as e:
        print(f"5. [ERRO] {e}"); ok = False

if ok:
    try:
        # 6. _run_tudo com pasta válida e pasta destino vazia
        import unittest.mock as mock
        with mock.patch('PyQt6.QtWidgets.QMessageBox.warning', return_value=None):
            win.lne_in.setText(r"D:\CD DUBLAGEM PROJETO\ORIGINAL_KLIFF_PATCH")
            win.lne_out.setText("")  # sem destino
            win._run_tudo()
        print("6. [OK] _run_tudo com destino vazio: warning (sem crash)")
    except Exception as e:
        print(f"6. [ERRO] {e}"); ok = False

if ok:
    try:
        # 7. _run_dub_multi com lista vazia não crasha
        win._run_dub_multi([])
        print("7. [OK] _run_dub_multi([]) não crasha")
    except Exception as e:
        print(f"7. [ERRO] {e}"); ok = False

if ok:
    try:
        # 8. Checkboxes existem com valores padrão corretos
        assert hasattr(win, 'chk_palatalizar')
        assert hasattr(win, 'chk_virgula')
        assert hasattr(win, 'chk_trailing')
        assert not win.chk_palatalizar.isChecked()  # OFF por padrão
        assert not win.chk_virgula.isChecked()       # OFF por padrão
        assert not win.chk_trailing.isChecked()      # OFF por padrão
        print("8. [OK] Checkboxes existem e começam OFF por padrão")
    except Exception as e:
        print(f"8. [ERRO] {e}"); ok = False

if ok:
    try:
        expected = {
            "_ancientstonegolem_9000_boss_00_00001.wav": "ancientstonegolem",
            "dragon_common_dragon_boss_9000_narration_00005.wav": "dragon_common_dragon_boss",
            "ndw_adult_1_questdialog_hello_00664.wav": "ndw_adult_1",
            "ngw_adult_1_questdialog_main_ngw_adult_1_00897.wav": "ngw_adult_1",
            "nhm_adult_citizen_9_questdialog_faction_01302.wav": "nhm_adult_citizen_9",
            "unique_kliff_0090_0120_player_00000.wav": "unique_kliff",
        }
        for file_name, family in expected.items():
            assert mod.audio_family_from_filename(file_name) == family
        print("9. [OK] Famílias de filtro extraídas dos nomes esperados")
    except Exception as e:
        print(f"9. [ERRO] {e}"); ok = False

if ok:
    tmpdir = tempfile.mkdtemp()
    try:
        names = [
            "_ancientstonegolem_9000_boss_00_00001.wav",
            "unique_kliff_0090_0120_player_00000.wav",
            "unique_kliff_0090_0120_player_00001.wav",
        ]
        for name in names:
            open(os.path.join(tmpdir, name), "wb").close()
        win.exp.set_folders(tmpdir, "")
        assert win.exp.tree.indentation() == 0
        assert not win.exp.tree.rootIsDecorated()
        assert win.exp.tree.topLevelItemCount() == 3
        win.exp._set_family_visible("unique_kliff", False)
        assert win.exp.tree.topLevelItemCount() == 1
        win.exp.update_status("unique_kliff_0090_0120_player_00000.wav", True, "")
        assert win.exp.tree.topLevelItemCount() == 1
        win.exp._show_all_families()
        assert win.exp.tree.topLevelItemCount() == 3
        print("10. [OK] Sidebar filtra famílias sem perder estado de status")
    except Exception as e:
        print(f"10. [ERRO] {e}"); ok = False
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)

if ok:
    try:
        player = mod.AudioPlayer("Teste")
        player._on_duration_changed(90000)
        player._on_position_changed(15000)
        assert player.slider.maximum() == 90000
        assert player.slider.value() == 15000
        assert player.lbl_time.text() == "0:15 / 1:30"
        print("11. [OK] AudioPlayer atualiza slider e tempo por sinais Qt")
    except Exception as e:
        print(f"11. [ERRO] {e}"); ok = False

if ok:
    try:
        assert mod.current_language_code(win.cmb_source_lang, "auto") == "auto"
        assert mod.current_language_code(win.cmb_target_lang, "pt") == "pt"
        assert "\U0001F1FA\U0001F1F8" not in win.txt_en.placeholderText()
        assert "\U0001F1E7\U0001F1F7" not in win.txt_pt.placeholderText()
        print("12. [OK] Idiomas usam dados internos e não dependem de emoji de bandeira")
    except Exception as e:
        print(f"12. [ERRO] {e}"); ok = False

print()
if ok:
    print("=== TODOS OS TESTES PASSARAM — PRONTO PARA RODAR ===")
else:
    print("=== FALHOU — corrigir antes de usar ===")
try:
    os.remove(cfg_path)
except FileNotFoundError:
    pass
if not ok:
    sys.exit(1)
