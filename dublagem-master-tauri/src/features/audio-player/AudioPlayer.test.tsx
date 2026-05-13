import { render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AudioPlayer } from "./AudioPlayer";

const clientMocks = vi.hoisted(() => ({
  prepareAudioPreview: vi.fn<(source: string) => Promise<string>>()
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`
}));

vi.mock("../../shared/tauri/client", () => ({
  isTauriRuntime: () => true,
  tauriClient: {
    prepareAudioPreview: clientMocks.prepareAudioPreview
  }
}));

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason?: unknown) => void;
}

class FakeResizeObserver {
  observe(): void {
    return undefined;
  }

  disconnect(): void {
    return undefined;
  }
}

const decodedAudioBuffer: AudioBuffer = {
  duration: 0.5,
  length: 4,
  numberOfChannels: 1,
  sampleRate: 48_000,
  copyFromChannel(destination, channelNumber, bufferOffset): void {
    void destination;
    void channelNumber;
    void bufferOffset;
  },
  copyToChannel(source, channelNumber, bufferOffset): void {
    void source;
    void channelNumber;
    void bufferOffset;
  },
  getChannelData: () => new Float32Array([0, 0.2, -0.4, 0.8])
};

const audioContexts: FakeAudioContext[] = [];

class FakeAudioContext {
  state: AudioContextState = "running";
  closeCalls = 0;
  readonly decodeAudioData = vi.fn<(audioData: ArrayBuffer) => Promise<AudioBuffer>>(
    (audioData) => {
      void audioData;
      return Promise.resolve(decodedAudioBuffer);
    }
  );

  constructor() {
    audioContexts.push(this);
  }

  close(): Promise<void> {
    this.closeCalls += 1;

    if (this.state === "closed") {
      return Promise.reject(
        new DOMException("Cannot close a closed AudioContext.", "InvalidStateError")
      );
    }

    this.state = "closed";
    return Promise.resolve();
  }
}

describe("AudioPlayer", () => {
  beforeEach(() => {
    audioContexts.length = 0;
    clientMocks.prepareAudioPreview.mockReset();
    vi.stubGlobal("AudioContext", FakeAudioContext);
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
    vi.spyOn(window.HTMLCanvasElement.prototype, "getContext").mockImplementation(() => null);
    vi.spyOn(window.HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("closes the waveform AudioContext once when unmount races with decoding", async () => {
    const previewPath = "E:\\audio\\preview.wav";
    const arrayBuffer = createDeferred<ArrayBuffer>();
    const response = {
      ok: true,
      status: 200,
      arrayBuffer: () => arrayBuffer.promise
    } as Response;
    const fetchMock = vi.fn<typeof fetch>(() => Promise.resolve(response));

    clientMocks.prepareAudioPreview.mockResolvedValue(previewPath);
    vi.stubGlobal("fetch", fetchMock);

    const { unmount } = render(<AudioPlayer title="Origem" path="E:\\audio\\source.wav" />);

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });

    const audioContext = createdAudioContext();

    unmount();

    expect(audioContext.closeCalls).toBe(1);

    arrayBuffer.resolve(new ArrayBuffer(8));

    await waitFor(() => {
      expect(audioContext.decodeAudioData).toHaveBeenCalledTimes(1);
    });
    await Promise.resolve();

    expect(audioContext.closeCalls).toBe(1);
  });
});

function createDeferred<T>(): Deferred<T> {
  let resolve: ((value: T) => void) | undefined;
  let reject: ((reason?: unknown) => void) | undefined;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });

  if (resolve === undefined || reject === undefined) {
    throw new Error("Falha ao inicializar Deferred.");
  }

  return { promise, resolve, reject };
}

function createdAudioContext(): FakeAudioContext {
  if (audioContexts.length !== 1) {
    throw new Error("AudioContext de teste nao foi criado.");
  }

  return audioContexts[0];
}
