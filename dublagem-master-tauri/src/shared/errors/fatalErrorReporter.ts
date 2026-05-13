import { message as showDialogMessage } from "@tauri-apps/plugin-dialog";
import { isTauriRuntime } from "../tauri/client";

let installed = false;
let dialogOpen = false;

export function installFrontendFatalErrorReporter(): void {
  if (installed || typeof window === "undefined") {
    return;
  }
  installed = true;

  window.addEventListener("error", (event) => {
    void reportFrontendFatalError("Erro fatal na interface", event.error ?? event.message);
  });
  window.addEventListener("unhandledrejection", (event) => {
    void reportFrontendFatalError("Promessa rejeitada sem tratamento", event.reason);
  });
}

async function reportFrontendFatalError(title: string, reason: unknown): Promise<void> {
  const message = errorMessage(reason);
  console.error(title, reason);

  if (!isTauriRuntime() || dialogOpen) {
    return;
  }

  dialogOpen = true;
  try {
    await showDialogMessage(message, {
      title,
      kind: "error"
    });
  } catch (dialogError) {
    console.error("Falha ao exibir diálogo de erro fatal", dialogError);
  } finally {
    dialogOpen = false;
  }
}

function errorMessage(reason: unknown): string {
  if (reason instanceof Error) {
    return reason.stack ?? reason.message;
  }
  return String(reason);
}
