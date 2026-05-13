# NSG Gaming Dub 1.0

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
cargo check --workspace --no-default-features
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
```

## Modelos

O runtime não usa Python. Modelos de ML ficam fora do Git, em `models/`, e são registrados por `models/MODEL_MANIFEST.json` com hashes dos pesos críticos. Quando esse bundle existe, a configuração aponta para ele automaticamente.

A build padrão usa as features `ml` e `cuda`. `whisper-rs` roda com GPU habilitada e o port Rust/Candle do OmniVoice vendorizado em `vendor/omnivoice-rs` usa `cuda:0` com FP16 para síntese local. O snapshot oficial do OmniVoice recebe automaticamente o manifesto `omnivoice.artifacts.json` esperado pelo runtime Candle quando a pasta de modelos ainda não o contém.

No Windows, o build local requer LLVM/libclang, CUDA Toolkit e MSVC Build Tools. O projeto define `LIBCLANG_PATH`, `CUDA_PATH`, `CUDACXX`, `CUDARC_CUDA_VERSION`, `CUDA_COMPUTE_CAP`, `CUDA_EXTRA_COMPUTE_CAPS`, `CMAKE_CUDA_ARCHITECTURES`, `GGML_NATIVE` e `NVCC_CCBIN` em `.cargo/config.toml`; ajuste esses caminhos se LLVM, CUDA ou MSVC forem instalados em outro diretório. O pacote oficial é compilado para GPU, sem fallback CPU, com suporte CUDA a Turing/RTX 20 (`7.5`) ou mais nova, incluindo RTX 30 (`8.6`), RTX 40 (`8.9`) e RTX 50/Blackwell (`12.x`). O driver NVIDIA instalado precisa pertencer ao ramo R580 ou superior para executar bibliotecas CUDA 13.x.

## Distribuição portátil

Os modelos Whisper e OmniVoice excedem o limite prático dos empacotadores MSI/NSIS. A distribuição Windows suportada é uma pasta portátil com o executável e `models/` lado a lado:

```powershell
npm run dist:portable
```

O script gera a pasta `dist-portable/NSG Gaming Dub 1.0/` e o arquivo `dist-portable/NSG-Gaming-Dub-1.0-portable.zip`. Distribua o `.zip` completo ou a pasta inteira ja extraida; nunca distribua apenas `src-tauri/target/release/dublagem-master-tauri.exe`, porque as DLLs CUDA e `models/` precisam ficar ao lado do executavel. Em máquinas sem GPU NVIDIA compatível ou com driver abaixo do ramo R580, o aplicativo interrompe o processamento com erro visível antes de inicializar o runtime CUDA nativo.
