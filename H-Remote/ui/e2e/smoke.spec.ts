import { expect, test } from "@playwright/test";

async function resetProfile(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.evaluate(() => {
    window.localStorage.clear();
  });
  await page.reload();
}

async function completeWizard(page: import("@playwright/test").Page) {
  await expect(
    page.getByRole("heading", { name: "Your sessions, one place" }),
  ).toBeVisible();
  for (let step = 0; step < 12; step += 1) {
    const finish = page.getByRole("button", { name: "Finish" });
    if (await finish.isVisible().catch(() => false)) {
      await finish.click();
      break;
    }
    const next = page.getByRole("button", { name: /next/i });
    await expect(next).toBeVisible();
    await next.click();
  }
  await expect(page.locator(".wizard-layer")).toHaveCount(0);
  await expect(page.getByTestId("nav-servers")).toBeVisible();
}

test.describe("H-Remote shell", () => {
  test("completes first-run guidance and opens the catalog", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await expect(page.getByRole("heading", { name: "Servers", exact: true })).toBeVisible();
    await expect(page.getByText("Production RDP")).toBeVisible();
  });

  test("searches, filters tags and switches views", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);

    const search = page.getByPlaceholder(/Search servers/);
    await search.fill("Build");
    await expect(page.getByText("Build Server")).toBeVisible();
    await expect(page.getByText("Production RDP")).toHaveCount(0);

    await search.fill("#prod");
    await expect(page.getByText("Production RDP")).toBeVisible();
    await expect(page.getByText("Lab VNC")).toHaveCount(0);

    await search.fill("");
    await page.getByRole("button", { name: "card view" }).click();
    await expect(page.getByText("File Server")).toBeVisible();
    await page.getByRole("button", { name: "tree view" }).click();
    await expect(page.getByText("Prod").first()).toBeVisible();
    await page.getByRole("button", { name: "list view" }).click();

    await page.getByTestId("folder-filter-Prod").click();
    await expect(page.locator(".row")).toHaveCount(3);
    await expect(page.getByText("Lab VNC")).toHaveCount(0);
    await page.getByTestId("folder-filter-Prod").click();
    await expect(page.locator(".row")).toHaveCount(7);
  });

  test("collapses and expands the left navigation", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.getByRole("button", { name: "Collapse sidebar" }).click();
    await expect(page.getByRole("button", { name: "Expand sidebar" })).toBeVisible();
    await expect(page.getByTestId("nav-settings")).toBeVisible();
    await page.getByRole("button", { name: "Expand sidebar" }).click();
    await expect(page.getByRole("button", { name: "Collapse sidebar" })).toBeVisible();
  });

  test("renames a connection inline with F2", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    const row = page.locator(".row", { hasText: "Production RDP" });
    await row.getByRole("button", { name: /Production RDP/ }).click();
    await page.keyboard.press("F2");
    const input = page.getByLabel("Rename Production RDP");
    await expect(input).toBeVisible();
    await input.fill("Renamed RDP");
    await input.press("Enter");
    await expect(page.getByText("Renamed RDP")).toBeVisible();
  });

  test("moves a connection into a folder and back to the root", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    const folder = page.getByTestId("folder-filter-Lab");
    const root = page.getByRole("button", { name: /All connections/ });
    await expect(folder).toContainText("1");
    await page.locator(".row", { hasText: "FTP Drop" }).dragTo(folder);
    await expect(folder).toContainText("2");
    await page.locator(".row", { hasText: "FTP Drop" }).dragTo(root);
    await expect(folder).toContainText("1");
  });

  test("colours an individual folder and keeps it after reload", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    const row = page.locator(".folder-row", { has: page.getByTestId("folder-filter-Lab") });
    const dot = row.locator(".dot");
    await page.getByRole("button", { name: "Colour for Lab" }).click();
    const picker = page.getByRole("dialog", { name: "Folder colour" });
    await expect(picker).toBeVisible();
    await picker.getByRole("button", { name: "Use colour #FF3EA66B" }).click();
    await expect(picker).toHaveCount(0);
    await expect(dot).toHaveCSS("background-color", "rgb(62, 166, 107)");

    await page.reload();
    await expect(
      page.locator(".folder-row", { has: page.getByTestId("folder-filter-Lab") }).locator(".dot"),
    ).toHaveCSS("background-color", "rgb(62, 166, 107)");
  });

  test("offers visible move controls in custom order", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.getByLabel("Sort servers").selectOption("custom");
    const rows = page.locator(".row");
    const firstName = await rows.nth(0).locator(".name-text").innerText();
    await expect(rows.nth(0).getByRole("button", { name: /Move .* up/ })).toBeVisible();
    await rows.nth(0).getByRole("button", { name: /Move .* down/ }).click();
    await expect(rows.nth(1)).toContainText(firstName.split("\n")[0]);
  });

  test("edits an address inline", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.getByRole("button", { name: "10.0.0.5" }).click();
    const input = page.locator(".inline-edit");
    await expect(input).toBeVisible();
    await input.fill("10.0.0.55");
    await input.press("Enter");
    await expect(page.getByRole("button", { name: "10.0.0.55" })).toBeVisible();
    // Enter confirms the inline edit; it must not also open a session.
    await expect(page.locator(".workspace")).toHaveCount(0);
  });

  test("adds a server through the side editor", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.getByRole("button", { name: /Add/ }).click();
    await expect(page.getByRole("heading", { name: "Add server" })).toBeVisible();
    await page.getByLabel("Display name", { exact: true }).fill("QA RDP");

    const viewport = page.viewportSize()!;
    const editor = await page.locator(".editor").boundingBox();
    expect(editor).not.toBeNull();
    expect(editor!.x).toBeGreaterThanOrEqual(0);
    expect(editor!.x + editor!.width).toBeLessThanOrEqual(viewport.width + 1);

    await page.getByRole("button", { name: "Icons", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Icons", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Windows", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Icons", exact: true })).toHaveCount(0);

    await page.getByRole("button", { name: "Connection", exact: true }).click();
    await page.getByLabel("Address", { exact: true }).fill("10.0.0.77");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByText("QA RDP")).toBeVisible();
  });

  test("renders the shared session window shell", async ({ page }) => {
    await page.goto("/?window=sessions");
    await expect(page.locator(".workspace")).toBeVisible();
    await expect(page.getByText("No sessions in this window")).toBeVisible();
    // A session window has one combined header: tabs, actions and controls.
    await expect(page.locator("header")).toHaveCount(1);
    await expect(page.locator(".workspace-bar .tabs")).toHaveCount(1);
    await expect(page.locator(".workspace-bar .window-controls")).toBeVisible();
  });

  test("connects a session, shows frames and reconnects", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);

    const row = page.locator(".row", { hasText: "Production RDP" });
    await row.dblclick();

    const overlay = page.locator(".workspace");
    await expect(overlay).toBeVisible();
    await expect(overlay.locator(".tab .dot.connected").first()).toBeVisible({
      timeout: 8000,
    });
    await expect(overlay.locator("canvas.frame")).toBeVisible();
    const geometry = await overlay.evaluate((root) => {
      const stage = root.querySelector(".stage")!.getBoundingClientRect();
      const canvas = root.querySelector("canvas.frame")!.getBoundingClientRect();
      const workspace = root.getBoundingClientRect();
      return {
        stage: { top: stage.top, bottom: stage.bottom, left: stage.left, right: stage.right },
        canvas: { top: canvas.top, bottom: canvas.bottom, left: canvas.left, right: canvas.right },
        workspace: { top: workspace.top, bottom: workspace.bottom },
        viewport: { width: window.innerWidth, height: window.innerHeight },
      };
    });
    // The whole frame stays inside the stage and the workspace never extends
    // past the viewport (the old grid track overflowed and cropped the bottom).
    expect(geometry.canvas.top).toBeGreaterThanOrEqual(geometry.stage.top - 1);
    expect(geometry.canvas.bottom).toBeLessThanOrEqual(geometry.stage.bottom + 1);
    expect(geometry.canvas.left).toBeGreaterThanOrEqual(geometry.stage.left - 1);
    expect(geometry.canvas.right).toBeLessThanOrEqual(geometry.stage.right + 1);
    expect(geometry.workspace.bottom).toBeLessThanOrEqual(
      geometry.viewport.height + 1,
    );
    await expect(overlay.getByRole("button", { name: "Ctrl+Alt+Del" })).toBeEnabled();

    await overlay
      .locator(".workspace-bar")
      .getByRole("button", { name: "Disconnect", exact: true })
      .click();
    await expect(
      overlay.getByRole("button", { name: "Reconnect" }),
    ).toBeVisible();
    await expect(overlay.locator(".tab .dot.connected")).toHaveCount(0);

    await overlay.getByRole("button", { name: "Reconnect" }).click();
    await expect(overlay.locator(".tab .dot.connected").first()).toBeVisible({
      timeout: 8000,
    });

    await overlay.locator('[title="Close tab"]').first().click();
    await expect(overlay).toHaveCount(0);
  });

  test("hosts multiple tabs, grids them and switches with the keyboard", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);

    await page.getByRole("checkbox", { name: "Select Production RDP" }).check();
    await page.getByRole("checkbox", { name: "Select Staging RDP" }).check();
    await page.getByRole("button", { name: "Connect", exact: true }).click();

    const workspace = page.locator(".workspace");
    await expect(workspace).toBeVisible();
    await expect(workspace.getByRole("tab")).toHaveCount(2);
    await expect(workspace.locator(".tab .dot.connected")).toHaveCount(2, {
      timeout: 8000,
    });

    const tabs = workspace.getByRole("tab");
    const firstLabel = await tabs.nth(0).innerText();
    await tabs.nth(0).dragTo(tabs.nth(1));
    await expect(tabs.nth(1)).toContainText(firstLabel);

    await workspace.getByRole("button", { name: "Grid", exact: true }).click();
    await expect(workspace.locator(".panes .pane")).toHaveCount(2);

    await page.keyboard.press("F11");
    await expect(workspace).toHaveClass(/fullscreen/, { timeout: 3000 });
    await expect(workspace).not.toHaveClass(/controls-visible/);
    await expect(workspace.locator(".content")).toHaveCSS("padding", "0px");
    await page.mouse.move(20, 2);
    await expect(workspace).toHaveClass(/controls-visible/, { timeout: 2000 });
    await page.mouse.move(400, 400);
    await expect(workspace).not.toHaveClass(/controls-visible/, { timeout: 2000 });
    await page.keyboard.press("F11");
    await expect(workspace).not.toHaveClass(/fullscreen/);

    await workspace.getByRole("button", { name: "Display" }).click();
    const display = page.getByRole("dialog");
    await expect(display).toContainText("Session display");
    await display.getByLabel("Virtual monitors").selectOption("2");
    await display.getByRole("button", { name: "Apply" }).click();
    await expect(page.getByText(/2 virtual monitors/)).toBeVisible();

    await workspace.getByRole("button", { name: "Layout", exact: true }).click();
    await workspace.getByRole("menuitem", { name: /Stacked/ }).click();
    await expect(workspace.locator(".panes .pane")).toHaveCount(2);

    await page.keyboard.press("Control+2");
    await expect(workspace.getByRole("tab").nth(1)).toHaveAttribute(
      "aria-selected",
      "true",
    );

    await workspace.getByRole("tab").nth(1).click({ button: "middle" });
    await expect(workspace.getByRole("tab")).toHaveCount(1);

    await workspace.locator('[title="Close tab"]').first().click();
    await expect(workspace).toHaveCount(0);
  });

  test("launcher filters servers and quick-connects from the keyboard", async ({ page }) => {
    await page.goto("/?window=launcher");
    const search = page.getByPlaceholder(/Search servers/);
    await search.fill("Production");
    await expect(page.getByRole("option")).toHaveCount(1);
    await expect(page.getByRole("option")).toContainText("Production RDP");

    await search.press("Tab");
    await expect(page.getByRole("heading", { name: "Quick connect", exact: true })).toBeVisible();
    const address = page.getByPlaceholder("host or IP");
    await expect(address).toHaveValue("10.0.0.5");
    await address.fill("10.5.5.5");
    await page.getByRole("button", { name: "Connect", exact: true }).click();
    await expect(page.getByText("10.5.5.5:3389")).toBeVisible();
  });

  test("imports a legacy 1Remote JSON export", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.getByRole("button", { name: "Import", exact: true }).click();
    const dialog = page.getByRole("dialog");
    await dialog.getByPlaceholder(/Paste file content/).fill(
      '[{"Protocol":"RDP","DisplayName":"Legacy Import","Address":"10.1.1.1","Port":"3389","Tags":["legacy"]}]',
    );
    await dialog.getByRole("button", { name: "Import", exact: true }).click();
    await expect(dialog.getByText("1 server(s) imported")).toBeVisible();
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByText("Legacy Import")).toBeVisible();
  });

  test("opens an SFTP browser and navigates folders", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.locator(".row", { hasText: "File Server" }).dblclick();

    const workspace = page.locator(".workspace");
    await expect(workspace).toBeVisible();
    await expect(workspace.getByLabel("Remote path")).toHaveValue("/srv/data");
    await expect(workspace.getByText("readme.txt")).toBeVisible();
    await expect(workspace.getByText("docs")).toBeVisible();

    await workspace.getByText("docs").dblclick();
    await expect(workspace.getByLabel("Remote path")).toHaveValue("/srv/data/docs");

    await workspace.getByTitle("Close tab").first().click();
    await expect(workspace).toHaveCount(0);
  });

  test("manages credentials in the vault", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-credentials").click();
    await expect(page.getByText("Prod admin")).toBeVisible();

    await page.getByRole("button", { name: "Add credential" }).click();
    await page.getByPlaceholder("prod-admin").fill("staging-admin");
    await page.getByLabel("Username").fill("deploy");
    await page.getByLabel("Password", { exact: true }).fill("secret");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByText("staging-admin", { exact: true })).toBeVisible();
  });

  test("checks for updates and shows the dialog", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.getByTestId("nav-settings").click();

    const feed = page.getByPlaceholder("https://example.invalid/h-remote/latest.json");
    await feed.fill("https://example.invalid/latest.json");
    await feed.press("Tab");
    await page.getByRole("button", { name: "Check now" }).click();
    await expect(page.getByText("Version 0.2.0 is available.").first()).toBeVisible();

    const dialog = page.getByRole("dialog");
    await expect(dialog).toContainText("Update available");
    await dialog.getByRole("button", { name: "Later" }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
  });

  test("adds a read-only data source and marks its servers", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-sources").click();
    await expect(page.getByRole("heading", { name: "Local catalog" })).toBeVisible();

    await page.getByRole("button", { name: "Add source" }).click();
    await page.getByPlaceholder("Team catalog").fill("Team DB");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByText("Team DB", { exact: true })).toBeVisible();

    await page.getByTestId("nav-servers").click();
    await expect(page.getByText("Team DB host")).toBeVisible();
    await expect(page.locator(".source-name").filter({ hasText: "Team DB" })).toBeVisible();
    await expect(page.locator(".source-ro").first()).toHaveText("read-only");

    await page.locator(".row", { hasText: "Team DB host" }).click({ button: "right" });
    await page.getByRole("menuitem", { name: "View", exact: true }).click();
    await expect(page.getByText("Read-only source")).toBeVisible();
    await expect(page.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
  });

  test("repairs catalog secrets from data settings", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-data").click();
    await page.getByRole("button", { name: "Repair secrets" }).click();
    await expect(page.getByRole("heading", { name: "Repair database secrets?" })).toBeVisible();
    await page.getByRole("button", { name: "OK", exact: true }).click();
    await expect(page.getByText(/Re-encrypted|Repaired/)).toBeVisible();
  });

  test("persists general settings across reloads", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);

    await page.getByTestId("nav-settings").click();
    const toggle = page.getByTestId("settings-copy-port");
    await expect(toggle).toHaveAttribute("aria-checked", "true");
    await toggle.click();
    await expect(toggle).toHaveAttribute("aria-checked", "false");

    await page.reload();
    await expect(page.getByTestId("nav-servers")).toBeVisible();
    await page.getByTestId("nav-settings").click();
    await expect(page.getByTestId("settings-copy-port")).toHaveAttribute(
      "aria-checked",
      "false",
    );
  });

  test("applies a preset theme live", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-appearance").click();
    await page.getByTestId("theme-Forest").click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "Forest");
  });

  test("switches language without a reload", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.getByTestId("nav-settings").click();
    await page.getByLabel("Display language").selectOption("de-DE");
    await expect(page.locator("html")).toHaveAttribute("lang", "de-DE");
  });

  test("shows the error report for unhandled UI errors", async ({ page }) => {
    await resetProfile(page);
    await completeWizard(page);
    await page.evaluate(() => {
      window.dispatchEvent(
        new ErrorEvent("error", {
          message: "synthetic e2e failure",
          error: new Error("synthetic e2e failure"),
        }),
      );
    });
    await expect(page.getByRole("heading", { name: "Something went wrong" })).toBeVisible();
    await expect(page.getByText(/synthetic e2e failure/).first()).toBeVisible();
  });
});
