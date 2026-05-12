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

No Windows, o build local requer LLVM/libclang, CUDA Toolkit e MSVC Build Tools. O projeto define `LIBCLANG_PATH`, `CUDA_PATH`, `CUDACXX`, `CUDARC_CUDA_VERSION`, `CUDA_COMPUTE_CAP`, `CUDA_EXTRA_COMPUTE_CAPS` e `NVCC_CCBIN` em `.cargo/config.toml`; ajuste esses caminhos se LLVM, CUDA ou MSVC forem instalados em outro diretório. O alvo primário `CUDA_COMPUTE_CAP=89` mantém compatibilidade nativa com RTX 40, enquanto `CUDA_EXTRA_COMPUTE_CAPS=120a` adiciona binários nativos Blackwell para RTX 50.

## Distribuição portátil

Os modelos Whisper e OmniVoice excedem o limite prático dos empacotadores MSI/NSIS. A distribuição Windows suportada é uma pasta portátil com o executável e `models/` lado a lado:

```powershell
npm run dist:portable
```

O artefato fica em `dist-portable/NSG Gaming Dub 1.0/`. Essa pasta contém tudo que o app precisa para iniciar e carregar os modelos locais.
