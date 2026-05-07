import sys, os
path = "DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX.py"
content = open(path, encoding='utf-8').read()

old_update = """    def _update_time(self):
        if not self._is_playing: return
        curr = (pygame.time.get_ticks() - self._start_ticks) / 1000.0
        if curr >= self._dur: self.stop(); return
        self.slider.blockSignals(True)
        self.slider.setValue(int((curr / max(self._dur, 0.1)) * 1000))
        self.slider.blockSignals(False)
        self._update_lbl(curr)
        self._seek = curr"""

new_update = """    def _update_time(self):
        if not self._is_playing: return
        import pygame
        if not self._is_paused and not pygame.mixer.music.get_busy():
            self.stop()
            return
        curr = (pygame.time.get_ticks() - self._start_ticks) / 1000.0
        if curr >= self._dur and self._dur > 0: curr = self._dur
        self.slider.blockSignals(True)
        self.slider.setValue(int((curr / max(self._dur, 0.1)) * 1000))
        self.slider.blockSignals(False)
        self._update_lbl(curr)
        self._seek = curr"""

if old_update in content:
    content = content.replace(old_update, new_update)
    open(path, "w", encoding="utf-8").write(content)
    print("AudioPlayer corrigido")
else:
    print("Bloco nao encontrado")
