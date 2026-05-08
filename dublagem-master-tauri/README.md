# Dublagem Master

Aplicativo desktop Tauri v2 para transcrição, tradução, síntese de voz e validação manual de áudios de dublagem.

## Stack

- Rust e Tauri v2 no backend.
- React, TypeScript e CSS Modules no frontend.
- Zustand para estado local da área de trabalho.
- Radix UI headless e lucide-react para controles de interface.

## Desenvolvimento

```powershell
npm install
npm run tauri dev
```

## Qualidade

```powershell
npm run lint
npm run typecheck
npm run test
npm run build
cd src-tauri
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Modelos

O runtime não usa Python. Modelos de ML ficam fora do Git e devem ser registrados por manifesto com hash. A aplicação falha de forma explícita quando Whisper ou OmniVoice ainda não foram provisionados.

Para compilar a feature opcional `ml`, instale uma distribuição LLVM/libclang e defina `LIBCLANG_PATH` quando o `clang.dll` não estiver no `PATH`.
