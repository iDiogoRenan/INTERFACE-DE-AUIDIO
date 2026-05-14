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
- `durationSeconds`: opcional, entre `0.25` e `30.00`. Esse valor é uma dica de duração para a síntese de uma linha, não um critério para descartar o áudio original.
- `numStep`: inteiro entre `8` e `128`.
- `guidanceScale`, `positionTemperature`, `classTemperature`: números entre `0.00` e `10.00`.
- `denoise`, `preprocessPrompt`, `postprocessOutput`: booleanos enviados diretamente para a configuração OmniVoice.
- `matchSourceLoudness`: aplica casamento local de RMS com o áudio de origem após a geração. `loudnessMatchStrength`, `sibilanceReduction` e `artifactReduction` aceitam valores entre `0.00` e `1.00`; `outputGainDb` aceita `-12.00` a `12.00`.
- Tags nativas: somente `[laughter]`, `[sigh]`, `[confirmation-en]`, `[question-en]`, `[question-ah]`, `[question-oh]`, `[question-ei]`, `[question-yi]`, `[surprise-ah]`, `[surprise-oh]`, `[surprise-wa]`, `[surprise-yo]` e `[dissatisfaction-hnn]`.

## Alinhamento temporal da dublagem

O áudio original é a autoridade de tempo da dublagem. O backend detecta janelas temporais de fala no áudio de origem, combina essas janelas com texto de origem e tradução, gera cada chunk separadamente, mede a duração real gerada e monta o resultado final posicionando cada chunk pelo `start_original` da janela correspondente.

A montagem final não concatena chunks em sequência. Os silêncios do áudio original permanecem na timeline e cada chunk carrega `start_original`, `end_original`, duração original, duração gerada, diferença percentual, ações de ajuste, tentativas, status e alertas no `alignmentReport` emitido pelos eventos de job.

Quando não há timestamps de palavra confiáveis, a segmentação textual não pode inventar fronteiras por contagem cega de palavras. O pipeline reduz janelas acústicas excedentes, prefere pontuação e pausas naturais, e só divide por palavras como último recurso sem sobrepor tokens entre chunks.

Diferenças pequenas de duração podem ser aceitas ou corrigidas por time-stretch com preservação de pitch. Diferenças maiores acionam adaptação textual e regeneração antes de qualquer ajuste agressivo. Chunks que continuam fora do limite, invadem a próxima janela, falham no TTS ou apresentam final abrupto crítico são sinalizados para revisão manual. Quando `blockExportOnCriticalChunks` está ativo, esses casos bloqueiam aprovação automática e movem o áudio para revisão.

O limite de chunks é controle de segurança e UX. Ele não descarta áudio silenciosamente. As políticas aceitas são:

- `warn_and_continue`: registra aviso e continua.
- `process_in_batches`: processa em lotes lógicos mantendo timeline e metadados contínuos.
- `require_confirmation`: registra o arquivo como aguardando confirmação.
- `resegment_first`: tenta resegmentar antes de continuar.
- `cancel_with_record`: cancela o arquivo com status explícito.

## Modelos

Whisper e OmniVoice são dependências de runtime. Os pesos não são versionados no repositório e devem ser verificados por hash antes de uso. A execução local de dublagem usa GPU por padrão; builds sem `cuda` devem falhar com erro explícito antes de iniciar ASR ou TTS local.
