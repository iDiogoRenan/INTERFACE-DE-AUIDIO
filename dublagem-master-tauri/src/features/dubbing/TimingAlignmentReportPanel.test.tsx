import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { TimingAlignmentReport } from "../../shared/tauri/types";
import { TimingAlignmentReportPanel } from "./TimingAlignmentReportPanel";

describe("TimingAlignmentReportPanel", () => {
  it("renders chunk timing details and routes chunk actions through props", () => {
    const onRegenerate = vi.fn();
    const onEditChunk = vi.fn();
    const onAcceptChunk = vi.fn();

    render(
      <TimingAlignmentReportPanel
        report={alignmentReport}
        isBusy={false}
        onRegenerate={onRegenerate}
        onEditChunk={onEditChunk}
        onAcceptChunk={onAcceptChunk}
      />
    );

    expect(
      screen.getByRole("region", { name: "Relatório de sincronização temporal" })
    ).toBeVisible();
    expect(screen.getByText("1 chunk(s) · limite 20")).toBeVisible();
    expect(screen.getByText("3.56s")).toBeVisible();
    expect(screen.getByText("[question-en] Quem sabe?")).toBeVisible();

    expect(screen.queryByRole("button", { name: "Original" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Dublado" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "A/B" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Reprocessar" }));
    fireEvent.click(screen.getByRole("button", { name: "Editar" }));
    fireEvent.click(screen.getByRole("button", { name: "Aceitar" }));

    expect(onRegenerate).toHaveBeenCalledTimes(1);
    expect(onEditChunk).toHaveBeenCalledWith(1);
    expect(onAcceptChunk).toHaveBeenCalledWith(alignmentReport.chunks[0]);
  });
});

const alignmentReport: TimingAlignmentReport = {
  audioId: "sample",
  fileName: "sample.wav",
  modelUsed: "omnivoice",
  totalChunks: 1,
  configuredChunkLimit: 20,
  chunkLimitPolicy: "process_in_batches",
  chunkLimitExceeded: false,
  processedInBatches: false,
  hasCriticalChunks: false,
  warnings: [],
  chunks: [
    {
      segmentId: "sample:1",
      audioId: "sample",
      chunkIndex: 1,
      totalChunks: 1,
      startOriginal: 0,
      endOriginal: 4.2,
      durationOriginal: 4.2,
      textoOriginalEn: "[question-en] Who knows?",
      textoPtbr: "[question-en] Quem sabe?",
      originalSegmentPath: null,
      dubbedSegmentPath: null,
      durationGenerated: 3.56,
      durationDifferencePercent: 15.2,
      statuses: ["time_stretched"],
      actionsApplied: ["time_stretched"],
      modelUsed: "omnivoice",
      attempts: 1,
      failureReason: null,
      stretchRatio: 1.25,
      overlapSeconds: null,
      abruptEndingDetected: false
    }
  ]
};
