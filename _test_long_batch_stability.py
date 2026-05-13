#!/usr/bin/env python3
"""Regression coverage for long dubbing queues without invoking ML models."""
import json
import os
import shutil
import sys
import tempfile

os.environ["QT_QPA_PLATFORM"] = "offscreen"
sys.path.insert(0, os.path.dirname(__file__))

ffmpeg_dir = os.path.join(os.path.dirname(__file__), ".venv", "ffmpeg")
os.environ["PATH"] = ffmpeg_dir + os.pathsep + os.environ.get("PATH", "")
cfg_fd, cfg_path = tempfile.mkstemp(suffix=".json")
os.close(cfg_fd)
os.unlink(cfg_path)
os.environ["DUBLAGEM_MASTER_CONFIG"] = cfg_path

from PyQt6.QtWidgets import QApplication

app = QApplication.instance() or QApplication(sys.argv)

import importlib.util

spec = importlib.util.spec_from_file_location(
    "main_mod",
    os.path.join(os.path.dirname(__file__), "DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX.py"),
)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)


def _audio_names(total: int) -> list[str]:
    return [f"voice_actor_questdialog_{index:05d}.wav" for index in range(total)]


def _touch_all(folder: str, names: list[str]) -> None:
    os.makedirs(folder, exist_ok=True)
    for name in names:
        open(os.path.join(folder, name), "wb").close()


def test_file_explorer_status_updates_are_in_place() -> None:
    for total in (1000, 2000, 5000):
        tmp_in = tempfile.mkdtemp()
        tmp_out = tempfile.mkdtemp()
        try:
            names = _audio_names(total)
            _touch_all(tmp_in, names)

            explorer = mod.FileExplorer()
            explorer.set_folders(tmp_in, tmp_out)
            assert explorer.tree.topLevelItemCount() == total

            def forbidden_refresh() -> None:
                raise AssertionError("update_status must not rebuild the full tree")

            explorer.refresh = forbidden_refresh
            for name in names:
                explorer.update_status(name, True, "")

            assert explorer.tree.topLevelItemCount() == total
            assert explorer._item_by_name[names[0]].text(1) == "Pronto"
            assert explorer._item_by_name[names[-1]].text(1) == "Pronto"
        finally:
            shutil.rmtree(tmp_in, ignore_errors=True)
            shutil.rmtree(tmp_out, ignore_errors=True)


def test_transcription_cache_writer_handles_five_thousand_entries() -> None:
    total = 5000
    tmp_out = tempfile.mkdtemp()
    try:
        names = _audio_names(total)
        writer = mod.TranscriptionCacheWriter(flush_interval=1000)

        for index, name in enumerate(names):
            writer.record(
                tmp_out,
                name,
                f"source text {index}",
                f"texto traduzido {index}",
                defer_write=True,
            )

        writer.end()
        cache_file = os.path.join(tmp_out, "transcricoes_cache.json")
        with open(cache_file, "r", encoding="utf-8") as handle:
            cache = json.load(handle)

        assert len(cache) == total
        assert cache[names[0]] == {"en": "source text 0", "pt": "texto traduzido 0"}
        assert cache[names[-1]] == {
            "en": f"source text {total - 1}",
            "pt": f"texto traduzido {total - 1}",
        }
        assert not any(name.endswith(".tmp") for name in os.listdir(tmp_out))
    finally:
        shutil.rmtree(tmp_out, ignore_errors=True)


def test_batch_done_slot_keeps_ui_work_bounded() -> None:
    total = 5000
    tmp_in = tempfile.mkdtemp()
    tmp_out = tempfile.mkdtemp()
    tmp_result_dir = tempfile.mkdtemp()
    try:
        names = _audio_names(total)
        _touch_all(tmp_in, names)
        result_file = os.path.join(tmp_result_dir, "generated.wav")
        open(result_file, "wb").close()

        win = mod.MainWindow()
        win.lne_in.setText(tmp_in)
        win.lne_out.setText(tmp_out)
        win.exp.set_folders(tmp_in, tmp_out)
        win._last_dub_is_multi = True
        win._begin_transcription_cache(tmp_out)

        def forbidden_player_load(_path: str) -> None:
            raise AssertionError("batch completion must not load every file into the player")

        win.play_pt.load = forbidden_player_load
        for index, name in enumerate(names):
            win._on_dub_done(
                True,
                "",
                result_file,
                name,
                f"source text {index}",
                f"texto traduzido {index}",
            )

        win._on_dub_worker_finished()
        cache_file = os.path.join(tmp_out, "transcricoes_cache.json")
        with open(cache_file, "r", encoding="utf-8") as handle:
            cache = json.load(handle)

        assert len(cache) == total
        assert os.path.exists(os.path.join(tmp_out, names[0]))
        assert os.path.exists(os.path.join(tmp_out, names[-1]))
        assert win.exp.tree.topLevelItemCount() == total
        assert win.exp._item_by_name[names[-1]].text(1) == "Pronto"
        assert win.log_box.document().maximumBlockCount() == 20000
    finally:
        shutil.rmtree(tmp_in, ignore_errors=True)
        shutil.rmtree(tmp_out, ignore_errors=True)
        shutil.rmtree(tmp_result_dir, ignore_errors=True)


if __name__ == "__main__":
    try:
        test_file_explorer_status_updates_are_in_place()
        print("1. [OK] FileExplorer atualiza 1000/2000/5000 status sem reconstruir a arvore")
        test_transcription_cache_writer_handles_five_thousand_entries()
        print("2. [OK] Cache de transcricoes grava 5000 entradas de forma atomica")
        test_batch_done_slot_keeps_ui_work_bounded()
        print("3. [OK] Slot de conclusao processa 5000 itens sem carregar player por arquivo")
    finally:
        try:
            os.remove(cfg_path)
        except FileNotFoundError:
            pass
