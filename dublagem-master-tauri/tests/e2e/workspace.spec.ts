import { expect, test } from "@playwright/test";

test("loads the desktop workspace shell", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Dublagem Master" })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Dublagem/u })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Validação/u })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Ajustes/u })).toBeVisible();
});

test("keeps dubbing controls and job status reachable in a short window", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 420 });
  await page.goto("/");

  await expect(page.getByRole("button", { name: /Dublar selecionado/u })).toBeVisible();
  await expect(page.getByText("Fila sem arquivo ativo")).toBeVisible();
});

test("lets the execution log use the remaining desktop height", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  const logBox = await page.getByRole("region", { name: "Log de execução" }).boundingBox();
  if (!logBox) {
    throw new Error("Execution log region was not rendered.");
  }

  const viewportHeight = await page.evaluate(() => window.innerHeight);
  expect(logBox.y + logBox.height).toBeGreaterThan(viewportHeight - 30);
});

test("aligns sidebar and workspace panel intersections", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  const metrics = await page.evaluate(() => {
    const sidebar = document.querySelector("aside");
    const sidebarHeader = sidebar?.firstElementChild;
    const sidebarFilter = sidebar?.querySelector("label");
    const workspace = document.querySelector("main")?.children.item(1);
    const workspaceHeader = workspace?.firstElementChild;
    const players = document.querySelector('[aria-label="Players de áudio"]');

    if (!sidebarHeader || !sidebarFilter || !workspaceHeader || !players) {
      throw new Error("Expected workspace layout regions were not rendered.");
    }

    return {
      sidebarHeaderBottom: sidebarHeader.getBoundingClientRect().bottom,
      workspaceHeaderBottom: workspaceHeader.getBoundingClientRect().bottom,
      sidebarFilterTop: sidebarFilter.getBoundingClientRect().top,
      playersTop: players.getBoundingClientRect().top
    };
  });

  expect(Math.abs(metrics.sidebarHeaderBottom - metrics.workspaceHeaderBottom)).toBeLessThanOrEqual(
    1
  );
  expect(Math.abs(metrics.sidebarFilterTop - metrics.playersTop)).toBeLessThanOrEqual(1);
});
