# Dublador Master Pro

`DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX.py` e o aplicativo principal deste repositorio. Ele e o app dublador usado para transcrever audios de origem, gerar dublagem em portugues e revisar os resultados antes da aprovacao final.

## Execucao

Use o inicializador do projeto no Windows:

```powershell
.\INICIAR_V14_ACCENT_FIX.bat
```

O inicializador espera que o ambiente local exista em `.venv` e adiciona o FFmpeg empacotado em `.venv\ffmpeg` ao `PATH` antes de abrir o app.

## Pastas De Trabalho

- `Origem (EN)`: pasta com os audios originais que serao transcritos e dublados.
- `Destino (PT)`: pasta onde os audios dublados e o cache de transcricoes sao salvos.
- `Audio Guia`: arquivo opcional usado como referencia adicional de voz.
- `Aprovados`: pasta usada na aba de validacao manual para guardar resultados aceitos.

## Interface

A janela principal combina o explorador de projeto, os players de audio, os controles de transcricao/dublagem, os ajustes finos e a aba de validacao manual. A sidebar de projeto lista os audios da pasta de origem e permite filtrar familias de arquivos derivadas dos nomes dos audios.
