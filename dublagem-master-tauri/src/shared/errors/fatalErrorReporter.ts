import { message as showDialogMessage } from "@tauri-apps/plugin-dialog";
import { isTauriRuntime } from "../tauri/client";

let installed = false;
let dialogOpen = false;

const resizeObserverLoopMessage = "ResizeObserver loop completed with undelivered notifications.";

export function installFrontendFatalErrorReporter(): void {
  if (installed || typeof window === "undefined") {
    return;
  }
  installed = true;

  window.addEventListener("error", (event) => {
    if (isResizeObserverLoopNotification(event)) {
      event.preventDefault();
      return;
    }
    void reportFrontendFatalError("Erro fatal na interface", event.error ?? event.message);
  });
  window.addEventListener("unhandledrejection", (event) => {
    if (isRecoverableFrontendNotification(event.reason)) {
      event.preventDefault();
      return;
    }
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

function isResizeObserverLoopNotification(event: ErrorEvent): boolean {
  return (
    isResizeObserverLoopMessage(event.message) || isRecoverableFrontendNotification(event.error)
  );
}

function isRecoverableFrontendNotification(reason: unknown): boolean {
  if (reason instanceof Error) {
    return isResizeObserverLoopMessage(reason.message);
  }
  if (typeof reason === "string") {
    return isResizeObserverLoopMessage(reason);
  }
  return false;
}

function isResizeObserverLoopMessage(message: string): boolean {
  return message.trim() === resizeObserverLoopMessage;
}
