# Dublagem

Este contexto descreve os termos de domínio usados pelo aplicativo local de dublagem de áudio.

## Language

**Áudio original**:
Arquivo de áudio de entrada que contém a fala a ser transcrita, traduzida ou usada como referência.
_Avoid_: EN, origem, source

**Áudio dublado**:
Arquivo de áudio gerado no idioma de destino a partir de um **Áudio original**.
_Avoid_: resultado, PT, output

**Dublagem em lote**:
Operação de dublagem aplicada a uma coleção finita de **Áudios originais** selecionados ou descobertos em uma pasta.
_Avoid_: lista longa, fila grande, dublar tudo

**Pasta de origem**:
Local que contém os **Áudios originais** disponíveis para dublagem.
_Avoid_: input, pasta EN

**Pasta de destino**:
Local que recebe os **Áudios dublados** aprovados ou salvos.
_Avoid_: output, pasta PT

**Revisão manual**:
Avaliação humana de um **Áudio dublado** que não foi aprovado automaticamente ou que precisa de decisão editorial.
_Avoid_: rejeitados, falhas, pendências

**Janela temporal**:
Intervalo obrigatório de fala detectado no **Áudio original**, com início, fim e duração próprios.
_Avoid_: pedaço, corte livre, trecho aproximado

**Chunk de dublagem**:
Unidade de geração de voz associada a uma **Janela temporal**. Um **Áudio original** pode exigir múltiplos chunks, e cada chunk mantém seus próprios timestamps.
_Avoid_: frase solta, arquivo parcial sem tempo, segmento sem janela

**Relatório de sincronização**:
Registro estruturado por **Chunk de dublagem** com duração original, duração gerada, diferença percentual, ajustes aplicados, tentativas, status e alertas.
_Avoid_: log genérico, resultado bruto

**Limite de chunks**:
Controle de segurança e UX que define quando um **Áudio original** exige aviso, confirmação, resegmentação ou processamento em lotes.
_Avoid_: descarte automático, limite fatal invisível

**Corte limpo**:
Fronteira entre **Chunks de dublagem** escolhida em pausa natural ou pontuação, sem repetir palavras e sem deixar expressão dependente partida de forma artificial.
_Avoid_: corte seco, quebra por contagem de palavras, sobreposição textual

## Relationships

- Uma **Pasta de origem** contém zero ou mais **Áudios originais**.
- Uma **Dublagem em lote** processa um ou mais **Áudios originais**.
- Um **Áudio original** pode produzir no máximo um **Áudio dublado** na **Pasta de destino** com o mesmo nome de arquivo.
- Um **Áudio dublado** pode ser encaminhado para **Revisão manual** quando sua qualidade não é confiável.
- Um **Áudio original** pode conter várias **Janelas temporais** de fala separadas por pausas.
- Cada **Chunk de dublagem** pertence a uma **Janela temporal** e deve ser posicionado pelo início dessa janela, não pela concatenação simples.
- Um **Relatório de sincronização** pertence a um **Áudio original** processado e descreve todos os seus **Chunks de dublagem**.
- O **Limite de chunks** nunca autoriza descarte silencioso; ele apenas seleciona uma política explícita de processamento ou decisão humana.
- Um **Chunk de dublagem** deve usar **Corte limpo** quando a fala precisar ser dividida.

## Example Dialogue

> **Dev:** "Ao iniciar uma **Dublagem em lote**, devemos considerar todos os arquivos da **Pasta de origem**?"
> **Domain expert:** "Sim, mas somente **Áudios originais**; cada um deve gerar um **Áudio dublado** correspondente na **Pasta de destino**."

> **Dev:** "Se um **Áudio original** precisar de mais chunks que o **Limite de chunks**, ele deve ser ignorado?"
> **Domain expert:** "Não. O aplicativo deve avisar, processar em lotes, pedir confirmação, resegmentar ou registrar cancelamento explícito."

## Flagged Ambiguities

- "lista longa" deve ser tratado como **Dublagem em lote** quando estiver descrevendo a operação de processar muitos áudios.
- "chunk" deve ser tratado como **Chunk de dublagem** quando estiver associado a uma **Janela temporal** do áudio.
- "cortar melhor" deve ser tratado como **Corte limpo** quando estiver descrevendo a fronteira entre chunks.
