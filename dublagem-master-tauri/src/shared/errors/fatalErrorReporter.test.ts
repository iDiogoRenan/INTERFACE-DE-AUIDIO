import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { installFrontendFatalErrorReporter } from "./fatalErrorReporter";

type DialogMessage = (
  message: string,
  options: {
    title: string;
    kind: "error";
  }
) => Promise<void>;

const dialogMocks = vi.hoisted(() => ({
  message: vi.fn<DialogMessage>(() => Promise.resolve())
}));

const runtimeMocks = vi.hoisted(() => ({
  isTauriRuntime: vi.fn<() => boolean>(() => true)
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  message: dialogMocks.message
}));

vi.mock("../tauri/client", () => ({
  isTauriRuntime: runtimeMocks.isTauriRuntime
}));

const resizeObserverLoopMessage = "ResizeObserver loop completed with undelivered notifications.";

describe("installFrontendFatalErrorReporter", () => {
  beforeAll(() => {
    installFrontendFatalErrorReporter();
  });

  beforeEach(() => {
    dialogMocks.message.mockReset();
    dialogMocks.message.mockResolvedValue(undefined);
    runtimeMocks.isTauriRuntime.mockReset();
    runtimeMocks.isTauriRuntime.mockReturnValue(true);
    vi.spyOn(console, "error").mockImplementation(() => undefined);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("does not show a fatal dialog for ResizeObserver loop notifications", async () => {
    window.dispatchEvent(
      new ErrorEvent("error", {
        message: resizeObserverLoopMessage,
        error: new Error(resizeObserverLoopMessage)
      })
    );

    await flushFatalErrorReporter();

    expect(dialogMocks.message).not.toHaveBeenCalled();
  });

  it("keeps reporting ordinary interface errors as fatal", async () => {
    const renderingError = new Error("Falha ao renderizar a timeline da dublagem.");

    window.dispatchEvent(
      new ErrorEvent("error", {
        message: renderingError.message,
        error: renderingError
      })
    );

    await flushFatalErrorReporter();

    expect(dialogMocks.message).toHaveBeenCalledTimes(1);
    expect(dialogMocks.message).toHaveBeenCalledWith(
      expect.stringContaining(renderingError.message),
      {
        title: "Erro fatal na interface",
        kind: "error"
      }
    );
  });
});

async function flushFatalErrorReporter(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}
