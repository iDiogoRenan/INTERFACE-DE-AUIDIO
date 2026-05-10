import { expect, test } from "@playwright/test";

test("loads the desktop workspace shell", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "NSG Gaming Dub 1.0" })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Dublagem/u })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Validação/u })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Ajustes/u })).toBeVisible();
});

test("keeps dubbing controls and job status reachable in a short window", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 420 });
  await page.goto("/");

  await expect(page.getByRole("button", { name: /Dublar selecionado/u })).toBeVisible();
  await expect(page.getByRole("button", { name: /Reverter transcrição/u })).toBeVisible();
  await expect(page.getByText("Fila sem arquivo ativo")).toBeVisible();
});

test("shows native OmniVoice tag and line property controls", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  await expect(page.getByRole("region", { name: "Paleta de tags OmniVoice" })).toBeVisible();
  await expect(page.getByRole("button", { name: "[sigh]" })).toBeVisible();
  await expect(page.getByRole("button", { name: "[sigh]" })).not.toHaveAttribute("title", /.+/u);
  await expect(page.getByRole("complementary", { name: "Propriedades da linha" })).toBeVisible();
  await expect(page.getByText("Personagem")).toHaveCount(0);
  await expect(page.getByRole("button", { name: /Propriedades basicas/u })).toBeVisible();
  await expect(page.getByText("Modo de voz")).toBeVisible();
  await expect(page.getByText("Instruct")).toBeVisible();
  await expect(page.getByText("Ajustes nativos")).toBeVisible();
  await expect(page.getByText("Polimento de audio")).toBeVisible();
  await expect(page.getByText("Preservar trilha de fundo")).toHaveCount(0);
  await expect(page.getByText("Nivel da trilha")).toHaveCount(0);
  await expect(page.getByText("Remover voz original")).toHaveCount(0);
  await expect(page.getByText("Reducao de sibilancia")).toBeVisible();
  await expect(page.getByRole("button", { name: "Salvar padrao global" })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Regenerar resultado" })).toBeDisabled();

  await page.getByText("Ajustes nativos").click();
  const sidebar = page.getByRole("complementary", { name: "Propriedades da linha" });
  const sidebarBox = await sidebar.boundingBox();
  const actionBox = await page.getByRole("button", { name: "Regenerar resultado" }).boundingBox();
  if (!sidebarBox || !actionBox) {
    throw new Error("Line sidebar action geometry was not available.");
  }
  expect(actionBox.y + actionBox.height).toBeLessThanOrEqual(sidebarBox.y + sidebarBox.height);
  expect(actionBox.y).toBeGreaterThan(sidebarBox.y + sidebarBox.height - 180);

  const panelHeader = sidebar.getByRole("button", { name: /Propriedades da linha/u });
  await panelHeader.click();
  await expect(panelHeader).toHaveAttribute("aria-expanded", "false");
  await expect(page.getByText("Velocidade")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Regenerar resultado" })).toBeVisible();
});

test("persists right sidebar collapsed sections", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  const basicSectionHeader = page.getByRole("button", { name: /Propriedades basicas/u });
  await expect(basicSectionHeader).toHaveAttribute("aria-expanded", "true");
  await basicSectionHeader.click();
  await expect(basicSectionHeader).toHaveAttribute("aria-expanded", "false");
  await expect(page.getByText("Modo de voz")).toHaveCount(0);

  await page.reload();

  await expect(page.getByRole("button", { name: /Propriedades basicas/u })).toHaveAttribute(
    "aria-expanded",
    "false"
  );
  await expect(page.getByText("Modo de voz")).toHaveCount(0);
});

test("keeps spaces inside line textareas without rendering duplicate text", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  const sourceEditor = page.locator("section").filter({ hasText: "Texto origem" }).first();
  const firstLine = sourceEditor.locator("textarea").first();
  await firstLine.fill("");
  await firstLine.pressSequentially("palavras com espacos");

  await expect(firstLine).toHaveValue("palavras com espacos");

  const outsideDuplicateCount = await firstLine.evaluate((textarea) => {
    const row = textarea.closest("label");
    if (!row) {
      return 0;
    }

    return Array.from(row.children).filter((child) => {
      return child !== textarea && child.textContent?.includes("palavras com espacos");
    }).length;
  });

  expect(outsideDuplicateCount).toBe(0);
});

test("grows line textareas for long text without internal scrollbars", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  const sourceEditor = page.locator("section").filter({ hasText: "Texto origem" }).first();
  const firstLine = sourceEditor.locator("textarea").first();
  const initialHeight = await firstLine.evaluate((textarea) => {
    return textarea.getBoundingClientRect().height;
  });

  await firstLine.fill("linha longa com espacos ".repeat(90).trim());

  await expect
    .poll(async () => {
      return firstLine.evaluate((textarea) => textarea.getBoundingClientRect().height);
    })
    .toBeGreaterThan(initialHeight + 20);

  await expect(firstLine).toHaveCSS("font-size", "14px");

  const overflow = await firstLine.evaluate((textarea) => {
    return textarea.scrollHeight - textarea.clientHeight;
  });
  expect(overflow).toBeLessThanOrEqual(2);
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
