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

## Relationships

- Uma **Pasta de origem** contém zero ou mais **Áudios originais**.
- Uma **Dublagem em lote** processa um ou mais **Áudios originais**.
- Um **Áudio original** pode produzir no máximo um **Áudio dublado** na **Pasta de destino** com o mesmo nome de arquivo.
- Um **Áudio dublado** pode ser encaminhado para **Revisão manual** quando sua qualidade não é confiável.

## Example Dialogue

> **Dev:** "Ao iniciar uma **Dublagem em lote**, devemos considerar todos os arquivos da **Pasta de origem**?"
> **Domain expert:** "Sim, mas somente **Áudios originais**; cada um deve gerar um **Áudio dublado** correspondente na **Pasta de destino**."

## Flagged Ambiguities

- "lista longa" deve ser tratado como **Dublagem em lote** quando estiver descrevendo a operação de processar muitos áudios.
