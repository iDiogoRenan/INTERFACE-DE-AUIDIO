# ADR 0001: Áudio original como autoridade temporal

## Status

Aceito.

## Contexto

A dublagem de jogos precisa preservar o tempo de fala do áudio original. Uma voz gerada com boa qualidade ainda é incorreta quando começa tarde, termina tarde, elimina pausas importantes ou invade a fala seguinte.

Modelos de TTS podem aceitar dicas de duração, dividir texto internamente ou variar ritmo entre execuções. Essas características são úteis para qualidade vocal, mas não devem definir a timeline final. A timeline confiável é a do áudio original.

## Decisão

Cada fala detectada no áudio original é tratada como uma janela temporal obrigatória. O pipeline gera áudio dublado por chunk, mede a duração gerada, aplica ajuste de duração somente dentro de limites configuráveis, adapta texto e regenera quando a diferença é grande, e reconstrói a exportação posicionando cada chunk pelo início da janela original.

O limite de chunks é um controle de segurança e experiência do usuário. Ele pode gerar aviso, processamento em lotes, confirmação, resegmentação ou cancelamento registrado, mas não autoriza descarte silencioso.

## Consequências

- O áudio dublado preserva pausas e posições da fala original.
- A exportação final depende de timestamps, não da ordem simples de arquivos gerados.
- Casos críticos são explicitamente reportados por chunk e podem bloquear aprovação automática.
- O mesmo contrato temporal vale para OmniVoice, Fish S2, ElevenLabs ou outro provedor de TTS.
- A UI pode reprocessar, editar ou aceitar manualmente chunks problemáticos sem exigir regeneração completa do áudio.
