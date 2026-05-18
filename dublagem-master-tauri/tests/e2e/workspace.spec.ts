import { expect, test } from "@playwright/test";

test("loads the desktop workspace shell", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "NSG Gaming Dub" })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Dublagem/u })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Validação/u })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Ajustes/u })).toBeVisible();
});

test("keeps workspace header tabs inside a narrow window", async ({ page }) => {
  await page.setViewportSize({ width: 1120, height: 720 });
  await page.goto("/");

  const metrics = await page.evaluate(() => {
    const tabs = Array.from(document.querySelectorAll('[role="tab"]'));
    if (tabs.length !== 3) {
      throw new Error("Expected the three workspace tabs to be rendered.");
    }

    return {
      documentWidth: document.documentElement.scrollWidth,
      tabBoxes: tabs.map((tab) => {
        const rect = tab.getBoundingClientRect();
        return {
          left: rect.left,
          right: rect.right,
          width: rect.width
        };
      }),
      viewportWidth: window.innerWidth
    };
  });

  expect(metrics.documentWidth).toBeLessThanOrEqual(metrics.viewportWidth);
  for (const tabBox of metrics.tabBoxes) {
    expect(tabBox.left).toBeGreaterThanOrEqual(0);
    expect(tabBox.right).toBeLessThanOrEqual(metrics.viewportWidth);
    expect(tabBox.width).toBeGreaterThan(96);
  }
});

test("keeps dubbing controls and job status reachable in a short window", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 420 });
  await page.goto("/");

  await expect(page.getByRole("button", { name: /Dublar selecionado/u })).toBeVisible();
  await expect(page.getByRole("button", { name: /Reverter transcrição/u })).toBeVisible();
  await expect(page.getByText("Fila sem arquivo ativo")).toBeVisible();
});

test("keeps audio players fluid without horizontal overflow", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  const metrics = await page.evaluate(() => {
    const players = document.querySelector('[aria-label="Reprodutores de áudio"]');
    const firstPlayer = players?.firstElementChild;
    const sidebar = document.querySelector('[aria-label="Propriedades da linha"]');
    if (!players || !firstPlayer || !sidebar) {
      throw new Error("Expected audio player layout regions were not rendered.");
    }

    const playerBox = firstPlayer.getBoundingClientRect();
    const playersBox = players.getBoundingClientRect();
    const sidebarBox = sidebar.getBoundingClientRect();

    return {
      viewportWidth: window.innerWidth,
      documentWidth: document.documentElement.scrollWidth,
      firstPlayerHeight: playerBox.height,
      firstPlayerWidth: playerBox.width,
      playersRight: playersBox.right,
      sidebarLeft: sidebarBox.left
    };
  });

  expect(metrics.documentWidth).toBeLessThanOrEqual(metrics.viewportWidth);
  expect(metrics.firstPlayerWidth).toBeGreaterThan(380);
  expect(metrics.firstPlayerHeight).toBeGreaterThanOrEqual(120);
  expect(metrics.firstPlayerHeight).toBeLessThanOrEqual(180);
  expect(metrics.playersRight).toBeLessThanOrEqual(metrics.sidebarLeft);
});

test("shows native OmniVoice tag and line property controls", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  await expect(page.getByRole("region", { name: "Paleta de marcadores OmniVoice" })).toBeVisible();
  await expect(page.getByRole("button", { name: "[sigh]" })).toBeVisible();
  await expect(page.getByRole("button", { name: "[sigh]" })).not.toHaveAttribute("title", /.+/u);
  await expect(page.getByRole("complementary", { name: "Propriedades da linha" })).toBeVisible();
  await expect(page.getByText("Personagem")).toHaveCount(0);
  await expect(page.getByRole("button", { name: /Propriedades básicas/u })).toBeVisible();
  await expect(page.getByText("Modo de voz")).toBeVisible();
  await expect(page.getByText("Instrução")).toBeVisible();
  await expect(page.getByText("Ajustes nativos")).toBeVisible();
  await expect(page.getByText("Polimento de áudio")).toBeVisible();
  await expect(page.getByText("Preservar trilha de fundo")).toHaveCount(0);
  await expect(page.getByText("Nivel da trilha")).toHaveCount(0);
  await expect(page.getByText("Remover voz original")).toHaveCount(0);
  await expect(page.getByText("Redução de sibilância")).toBeVisible();
  await expect(page.getByRole("button", { name: "Salvar ajustes globais" })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Regenerar resultado" })).toBeDisabled();

  await page.getByRole("button", { name: /Ajustes nativos globais/u }).click();
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

test("places filtered-list dubbing action above the tag palette", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  const listButton = page.getByRole("button", { name: /Dublar lista/u });
  const tagPalette = page.getByRole("region", { name: "Paleta de marcadores OmniVoice" });

  await expect(listButton).toBeVisible();
  await expect(listButton).toBeDisabled();

  const listButtonBox = await listButton.boundingBox();
  const tagPaletteBox = await tagPalette.boundingBox();
  if (!listButtonBox || !tagPaletteBox) {
    throw new Error("Filtered-list action geometry was not available.");
  }

  expect(listButtonBox.y + listButtonBox.height).toBeLessThanOrEqual(tagPaletteBox.y);
});

test("persists right sidebar collapsed sections", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  const basicSectionHeader = page.getByRole("button", { name: /Propriedades básicas/u });
  await expect(basicSectionHeader).toHaveAttribute("aria-expanded", "true");
  await basicSectionHeader.click();
  await expect(basicSectionHeader).toHaveAttribute("aria-expanded", "false");
  await expect(page.getByText("Modo de voz")).toHaveCount(0);

  await page.reload();

  await expect(page.getByRole("button", { name: /Propriedades básicas/u })).toHaveAttribute(
    "aria-expanded",
    "false"
  );
  await expect(page.getByText("Modo de voz")).toHaveCount(0);
});

test("collapses tag palette downward to free the audio column", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  const tagPalette = page.getByRole("region", { name: "Paleta de marcadores OmniVoice" });
  const paletteHeader = tagPalette.getByRole("button", { name: /Paleta de marcadores/u });
  const fileColumn = page.getByRole("region", { name: "Arquivos do projeto" });

  await expect(paletteHeader).toHaveAttribute("aria-expanded", "true");
  const openFileColumnBox = await fileColumn.boundingBox();
  if (!openFileColumnBox) {
    throw new Error("Project audio column geometry was not available.");
  }

  await paletteHeader.click();

  await expect(paletteHeader).toHaveAttribute("aria-expanded", "false");
  await expect(tagPalette.getByRole("button", { name: "[sigh]" })).toHaveCount(0);
  const collapsedFileColumnBox = await fileColumn.boundingBox();
  if (!collapsedFileColumnBox) {
    throw new Error("Collapsed project audio column geometry was not available.");
  }
  expect(collapsedFileColumnBox.height).toBeGreaterThan(openFileColumnBox.height + 60);

  await page.reload();

  await expect(
    page
      .getByRole("region", { name: "Paleta de marcadores OmniVoice" })
      .getByRole("button", { name: /Paleta de marcadores/u })
  ).toHaveAttribute("aria-expanded", "false");
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

  const logBox = await page.getByRole("region", { name: "Registro de execução" }).boundingBox();
  if (!logBox) {
    throw new Error("Execution log region was not rendered.");
  }

  const viewportHeight = await page.evaluate(() => window.innerHeight);
  expect(logBox.y + logBox.height).toBeGreaterThan(viewportHeight - 30);
});

test("collapses the execution log to free the dubbing workspace", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  const logBox = page.getByRole("region", { name: "Registro de execução" });
  const editorBox = page.getByRole("region", { name: "Transcrição editável" });
  const sidebarBox = page.getByRole("complementary", { name: "Propriedades da linha" });
  const listButton = page.getByRole("button", { name: /Dublar lista/u });
  const tagPaletteHeader = page.getByRole("button", { name: /Paleta de marcadores/u });
  const dubbingAction = page.getByRole("button", { name: "Dublar selecionado" });
  const collapseButton = page.getByRole("button", { name: "Recolher logs" });
  await expect(collapseButton).toHaveAttribute("aria-expanded", "true");

  const expandedBox = await logBox.boundingBox();
  const expandedEditorBox = await editorBox.boundingBox();
  const expandedSidebarBox = await sidebarBox.boundingBox();
  const expandedListButtonBox = await listButton.boundingBox();
  const expandedTagPaletteHeaderBox = await tagPaletteHeader.boundingBox();
  const expandedActionBox = await dubbingAction.boundingBox();
  if (
    !expandedBox ||
    !expandedEditorBox ||
    !expandedSidebarBox ||
    !expandedListButtonBox ||
    !expandedTagPaletteHeaderBox ||
    !expandedActionBox
  ) {
    throw new Error("Expanded dubbing workspace geometry was not available.");
  }

  await collapseButton.click();
  await expect(page.getByRole("button", { name: "Expandir logs" })).toHaveAttribute(
    "aria-expanded",
    "false"
  );

  const collapsedBox = await logBox.boundingBox();
  const collapsedEditorBox = await editorBox.boundingBox();
  const collapsedSidebarBox = await sidebarBox.boundingBox();
  const collapsedListButtonBox = await listButton.boundingBox();
  const collapsedTagPaletteHeaderBox = await tagPaletteHeader.boundingBox();
  const collapsedActionBox = await dubbingAction.boundingBox();
  if (
    !collapsedBox ||
    !collapsedEditorBox ||
    !collapsedSidebarBox ||
    !collapsedListButtonBox ||
    !collapsedTagPaletteHeaderBox ||
    !collapsedActionBox
  ) {
    throw new Error("Collapsed dubbing workspace geometry was not available.");
  }

  const viewportHeight = await page.evaluate(() => window.innerHeight);
  expect(collapsedBox.height).toBeLessThan(expandedBox.height - 40);
  expect(collapsedBox.height).toBeLessThanOrEqual(56);
  expect(collapsedBox.y + collapsedBox.height).toBeGreaterThanOrEqual(viewportHeight - 30);
  expect(
    Math.abs(collapsedBox.y + collapsedBox.height - (expandedBox.y + expandedBox.height))
  ).toBeLessThanOrEqual(1);
  expect(collapsedEditorBox.y).toBeLessThanOrEqual(expandedEditorBox.y + 8);
  expect(collapsedActionBox.y).toBeLessThanOrEqual(expandedActionBox.y + 8);
  expect(
    Math.abs(
      collapsedSidebarBox.y + collapsedSidebarBox.height - (collapsedBox.y + collapsedBox.height)
    )
  ).toBeLessThanOrEqual(1);
  expect(Math.abs(collapsedSidebarBox.height - expandedSidebarBox.height)).toBeLessThanOrEqual(1);
  expect(Math.abs(collapsedListButtonBox.y - expandedListButtonBox.y)).toBeLessThanOrEqual(1);
  expect(
    Math.abs(collapsedTagPaletteHeaderBox.y - expandedTagPaletteHeaderBox.y)
  ).toBeLessThanOrEqual(1);

  await page.getByRole("button", { name: "Expandir logs" }).click();
  await expect(page.getByRole("button", { name: "Recolher logs" })).toHaveAttribute(
    "aria-expanded",
    "true"
  );

  const reexpandedBox = await logBox.boundingBox();
  const reexpandedListButtonBox = await listButton.boundingBox();
  const reexpandedTagPaletteHeaderBox = await tagPaletteHeader.boundingBox();
  if (!reexpandedBox || !reexpandedListButtonBox || !reexpandedTagPaletteHeaderBox) {
    throw new Error("Re-expanded dubbing workspace geometry was not available.");
  }

  expect(reexpandedBox.height).toBeGreaterThan(collapsedBox.height + 40);
  expect(reexpandedBox.y).toBeLessThan(collapsedBox.y - 40);
  expect(
    Math.abs(reexpandedBox.y + reexpandedBox.height - (collapsedBox.y + collapsedBox.height))
  ).toBeLessThanOrEqual(1);
  expect(Math.abs(reexpandedListButtonBox.y - expandedListButtonBox.y)).toBeLessThanOrEqual(1);
  expect(
    Math.abs(reexpandedTagPaletteHeaderBox.y - expandedTagPaletteHeaderBox.y)
  ).toBeLessThanOrEqual(1);
});

test("shows timestamps in execution log entries", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  await page.getByRole("button", { name: "Dublar selecionado" }).click();

  const logBox = page.getByRole("region", { name: "Registro de execução" });
  const newestEntry = logBox.locator("p").first();
  const timestamp = newestEntry.locator("time");

  await expect(newestEntry).toContainText("Selecione um arquivo e uma pasta de destino.");
  await expect(timestamp).toBeVisible();
  await expect(timestamp).toHaveAttribute(
    "datetime",
    /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u
  );
});

test("aligns sidebar and workspace panel intersections", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 960 });
  await page.goto("/");

  const metrics = await page.evaluate(() => {
    const sidebar = document.querySelector('[aria-label="Explorador do projeto"]');
    const sidebarHeader = sidebar?.firstElementChild;
    const sidebarFilter = sidebar?.querySelector("label");
    const workspace = document.querySelector("main")?.children.item(1);
    const workspaceHeader = workspace?.firstElementChild;
    const players = document.querySelector('[aria-label="Reprodutores de áudio"]');

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
