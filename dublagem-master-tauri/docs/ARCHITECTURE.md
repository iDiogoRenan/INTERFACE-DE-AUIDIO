# Arquitetura

O aplicativo separa o shell visual do pipeline de áudio. O frontend chama comandos Tauri tipados; o backend Rust concentra validações, acesso ao sistema de arquivos, jobs, tradução, ASR e TTS.

## Domínios

- `dublagem-domain`: tipos serializáveis compartilhados entre comandos e eventos.
- `audio`: descoberta de arquivos, famílias, metadados e métricas de qualidade.
- `translation`: provider oficial do Google Cloud Translation v3.
- `speech`: fronteiras para `whisper-rs` e OmniVoice/Candle.
- `jobs`: fila assíncrona com progresso por eventos Tauri e cancelamento.
- `config`: persistência em diretório de configuração da aplicação.

## Contratos Tauri

Comandos públicos:

- `load_config`, `save_config`, `scan_audio_folder`, `get_audio_metadata`, `inspect_audio_quality`
- `transcribe_audio`, `translate_text`, `start_dubbing_job`, `cancel_job`
- `approve_file`, `reject_file`, `generate_voice_pool`

Eventos públicos:

- `job:progress`, `job:log`, `job:file-complete`, `job:finished`, `job:failed`

## Modelos

Whisper e OmniVoice são dependências de runtime. Os pesos não são versionados no repositório e devem ser verificados por hash antes de uso.
