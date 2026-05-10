# Arquitetura

O aplicativo separa o shell visual do pipeline de áudio. O frontend chama comandos Tauri tipados; o backend Rust concentra validações, acesso ao sistema de arquivos, jobs, tradução, ASR e TTS.

## Domínios

- `dublagem-domain`: tipos serializáveis compartilhados entre comandos e eventos.
- `audio`: descoberta de arquivos, famílias, metadados e métricas de qualidade.
- `translation`: provider oficial do Google Cloud Translation v3.
- `speech`: fronteiras para `whisper-rs` e OmniVoice/Candle, ambos executados no caminho CUDA da build padrão.
- `jobs`: fila assíncrona com progresso por eventos Tauri e cancelamento.
- `config`: persistência em diretório de configuração da aplicação.

## Contratos Tauri

Comandos públicos:

- `load_config`, `save_config`, `scan_audio_folder`, `get_audio_metadata`, `inspect_audio_quality`
- `transcribe_audio`, `translate_text`, `start_dubbing_job`, `cancel_job`
- `preview_synthesis_line`, `approve_file`, `reject_file`, `generate_voice_pool`

Eventos públicos:

- `job:stage`, `job:transcription`, `job:progress`, `job:file-complete`, `job:cancelled`, `job:finished`, `job:failed`

## Controles de síntese por linha

O painel de propriedades da linha grava metadados em `nsg_dub_project.json` e envia ajustes efetivos em `lineOverrides` quando a dublagem é iniciada ou regenerada. A prévia de linha usa o mesmo contrato de validação pelo comando `preview_synthesis_line`. Os ajustes também podem ser salvos como padrão global em `config.json`; a restauração retorna aos defaults de fábrica.

Controles aceitos pelo backend:

- `voiceMode`: aceita `clone`, `design` e `auto`. Em `design`, `instruct` deve conter uma descrição de voz; em `clone`, o prompt vem da referência curta extraída do áudio.
- `speed`: opcional, entre `0.50` e `2.00`. Quando `durationSeconds` está definido, duração tem prioridade.
- `durationSeconds`: opcional, entre `0.25` e `60.00`.
- `numStep`: inteiro entre `8` e `128`.
- `guidanceScale`, `positionTemperature`, `classTemperature`: números entre `0.00` e `10.00`.
- `denoise`, `preprocessPrompt`, `postprocessOutput`: booleanos enviados diretamente para a configuração OmniVoice.
- `matchSourceLoudness`: aplica casamento local de RMS com o áudio de origem após a geração. `loudnessMatchStrength`, `sibilanceReduction` e `artifactReduction` aceitam valores entre `0.00` e `1.00`; `outputGainDb` aceita `-12.00` a `12.00`.
- Tags nativas: somente `[laughter]`, `[sigh]`, `[confirmation-en]`, `[question-en]`, `[question-ah]`, `[question-oh]`, `[question-ei]`, `[question-yi]`, `[surprise-ah]`, `[surprise-oh]`, `[surprise-wa]`, `[surprise-yo]` e `[dissatisfaction-hnn]`.

## Modelos

Whisper e OmniVoice são dependências de runtime. Os pesos não são versionados no repositório e devem ser verificados por hash antes de uso. A execução local de dublagem usa GPU por padrão; builds sem `cuda` devem falhar com erro explícito antes de iniciar ASR ou TTS local.
