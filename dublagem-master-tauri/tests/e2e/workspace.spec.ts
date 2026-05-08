import { expect, test } from "@playwright/test";

test("loads the desktop workspace shell", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Dublagem Master" })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Dublagem/u })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Validação/u })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Ajustes/u })).toBeVisible();
});
