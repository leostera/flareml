import { test, expect } from "@playwright/test";
import { spawn, execFileSync, type ChildProcess } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { stripVTControlCharacters } from "node:util";
import { resolve, join } from "node:path";
import fixture from "./fixtures/lossless.json" with { type: "json" };
const binary = resolve("../target/debug/fml");
let directory: string;
const children: ChildProcess[] = [];
const diagnostics = new Map<ChildProcess, string>();
const exits = new Map<ChildProcess, number>();
test.beforeAll(() => {
  directory = mkdtempSync(join(tmpdir(), "fml-explorer-"));
});
test.afterAll(async () => {
  for (const child of children) {
    if (child.exitCode === null) {
      const done = new Promise<number | null>((resolve) =>
        child.once("exit", resolve),
      );
      child.kill("SIGINT");
      expect(await done, diagnostics.get(child)).toBe(exits.get(child) ?? 0);
    } else
      expect(child.exitCode, diagnostics.get(child)).toBe(
        exits.get(child) ?? 0,
      );
  }
  rmSync(directory, { recursive: true, force: true });
});
async function start(name: string, source?: string, checkUI = false) {
  const model = source
    ? join(directory, `${name}.fml`)
    : resolve(`../examples/${name}.fml`);
  if (source) writeFileSync(model, source);
  const runs = join(directory, `${name}-runs`);
  if (!checkUI)
    try {
      execFileSync(binary, ["check", model, "--artifacts-dir", runs], {
        stdio: "ignore",
      });
    } catch (e) {
      if ((e as { status: number }).status !== 1) throw e;
    }
  const args = checkUI
    ? ["check", model, "--artifacts-dir", runs]
    : ["replay", join(runs, readdirSync(runs)[0])];
  const child = spawn(binary, [...args, "--ui", "--no-open"], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  children.push(child);
  exits.set(child, checkUI ? 1 : 0);
  return await new Promise<string>((resolve, reject) => {
    let text = "";
    const timer = setTimeout(
      () => reject(new Error("server startup timed out")),
      10000,
    );
    child.stderr!.on("data", (data) => {
      text += data.toString();
      diagnostics.set(child, text);
      const match = text.match(
        /Explorer: (http:\/\/127\.0\.0\.1:\d+\/#[a-f0-9]+)/,
      );
      if (match) {
        clearTimeout(timer);
        resolve(match[1]);
      }
    });
    child.once("exit", (code) => {
      clearTimeout(timer);
      if (code) reject(new Error(`server exited ${code}: ${text}`));
    });
  });
}
async function withDevServer(
  origin: string | undefined,
  run: (url: string) => Promise<void>,
) {
  const env = { ...process.env };
  delete env.FML_EXPLORER_ORIGIN;
  if (origin) env.FML_EXPLORER_ORIGIN = origin;
  const child = spawn(
    process.execPath,
    ["node_modules/vite/bin/vite.js", "--host", "127.0.0.1", "--port", "5180"],
    { env, stdio: ["ignore", "pipe", "pipe"] },
  );
  let stderr = "";
  child.stderr!.on("data", (data) => {
    stderr += data.toString();
  });
  try {
    const url = await new Promise<string>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error("Vite startup timed out")),
        10000,
      );
      child.stdout!.on("data", (data) => {
        const match = stripVTControlCharacters(data.toString()).match(
          /http:\/\/127\.0\.0\.1:\d+\//,
        );
        if (match) {
          clearTimeout(timer);
          resolve(match[0]);
        }
      });
      child.once("exit", () => {
        clearTimeout(timer);
        reject(new Error(`Vite exited before startup: ${stderr}`));
      });
    });
    await run(url);
  } finally {
    if (child.exitCode === null) {
      const done = new Promise((resolve) => child.once("exit", resolve));
      child.kill("SIGTERM");
      await done;
    }
  }
}
test("Vite development uses the real native API, or explains missing configuration", async ({
  page,
}) => {
  const native = await start("dev-lossless", fixture.metadata.source);
  await withDevServer(native.split("/#")[0], async (dev) => {
    await page.goto(`${dev}#${native.split("#")[1]}`);
    await expect(page.locator(".react-flow__node")).toHaveCount(1);
    await expect(page.getByRole("alert")).toHaveCount(0);
  });
  await withDevServer(undefined, async (dev) => {
    await page.goto(dev);
    await expect(page.getByRole("alert")).toContainText(
      "Missing session token",
    );
    await page.goto(`${dev}#${native.split("#")[1]}`);
    await page.reload();
    await expect(page.getByRole("alert")).toContainText(
      "No native replay server configured",
    );
    await expect(page.getByRole("alert")).not.toContainText("Unexpected token");
  });
});
test("native embedded UI: exact values, safe labels, navigation, source and security", async ({
  page,
}) => {
  const url = await start("lossless", fixture.metadata.source);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);
  await expect(page.getByRole("tree")).toBeVisible();
  await expect(page.locator("img")).toHaveCount(0);
  await page.locator(".react-flow__node").first().click();
  await expect(page.locator(".inspector")).toContainText("9223372036854775807");
  await expect(page.locator(".react-flow__node")).toHaveCount(1);
  await page.getByRole("button", { name: "Next step", exact: true }).click();
  await expect(page.locator(".step-number")).toHaveText("1 / 1");
  await expect(page.locator('[aria-current="step"]')).toContainText("A");
  await expect(page.locator(".transition-panel")).toContainText("Received");
  await expect(page.locator(".field-change del")).toHaveText(
    "9223372036854775807",
  );
  await expect(page.locator(".field-change ins")).toHaveText("0");
  await expect(page.locator(".entity-panel")).toContainText("Current state");
  await page.getByRole("tab", { name: "Entities", exact: true }).click();
  await page.locator(".entity-list button").first().click();
  await expect(page.locator(".entity-panel")).toContainText("A #0");
  await expect(page.locator(".transition-panel")).toContainText("What changed");
  await page.getByRole("tab", { name: "Traces", exact: true }).click();
  await page.locator(".brand").click();
  await page.keyboard.press("ArrowLeft");
  await expect(page.locator(".step-number")).toHaveText("0 / 1");
  await expect(page.locator('[aria-current="step"]')).toContainText(
    "Initial state",
  );
  const origin = url.split("/#")[0];
  expect((await page.request.get(`${origin}/api/session`)).status()).toBe(403);
  expect(
    (
      await page.request.get(`${origin}/api/session`, {
        headers: {
          Authorization: `Bearer ${url.split("#")[1]}`,
          Origin: "https://evil.test",
        },
      })
    ).status(),
  ).toBe(403);
  expect(errors).toEqual([]);
});
test("real creation and choice events are inspectable without unborn nodes", async ({
  page,
}) => {
  await page.goto(await start("spawn-choice-workers"));
  await expect(page.locator(".react-flow__node")).toHaveCount(1);
  await page.getByRole("button", { name: "Last step" }).click();
  await expect(page.locator(".react-flow__node")).toHaveCount(3);
  await expect(page.locator('[aria-current="step"]')).toContainText("choice");
  await expect(page.locator(".transition-panel")).toContainText("Choices");
  await expect(page.locator(".transition-panel")).toContainText(
    "Created 2 entities",
  );
  await expect(page.locator(".changed-entity")).toHaveCount(3);
});
test("message-flow arrows activate on sends, go dotted when idle, and rewind without future routes", async ({
  page,
}) => {
  const source =
    'actor Gateway { handle_message(target: Actor<Processor>): unit { send(target, ()); send(target, ()); } } actor Processor { init(): Int { 0 } handle_message(s: Int, m: unit): Int { s + 1 } } property "done" { reachable (exists (a in instances(Processor)) { a.state == Some(2) }) } check C { domain Int = 0..2 spawn_bound Gateway = 1 spawn_bound Processor = 1 mailbox_bound = 2 main { let p = spawn(Processor); let g = spawn(Gateway); send(g, p); } }';
  await page.goto(await start("message-flow", source));
  await expect(page.locator(".react-flow__node")).toHaveCount(2);
  await expect(page.locator(".message-flow")).toHaveCount(0);
  await page.getByRole("button", { name: "Next step", exact: true }).click();
  await expect(page.locator(".active-flow")).toHaveCount(1);
  await expect(page.locator(".active-flow")).toContainText("2 messages");
  await expect(page.locator(".active-flow")).toHaveAttribute(
    "aria-label",
    /actor:Gateway:0 to actor:Processor:0/,
  );
  await page.locator(".active-flow .react-flow__edge-textbg").click();
  await expect(page.locator(".entity-panel")).toContainText("Processor #0");
  await page.screenshot({ path: "test-results/message-flow.png" });
  await page.getByRole("button", { name: "Next step", exact: true }).click();
  await expect(page.locator(".active-flow")).toHaveCount(0);
  await expect(page.locator(".past-flow")).toHaveCount(1);
  await page
    .getByRole("button", { name: "Previous step", exact: true })
    .click();
  await expect(page.locator(".active-flow")).toHaveCount(1);
  await page.getByRole("button", { name: "First step", exact: true }).click();
  await expect(page.locator(".step-number")).toHaveText("0 / 3");
  await expect(page.locator(".message-flow")).toHaveCount(0);
});
test("spawn order places Processor before Gateway, and Receipt reverses the existing connection", async ({
  page,
}) => {
  const source =
    'type Request = Start(Actor<Processor>, Actor<Gateway>) | Receipt actor Gateway { init(): Bool { false } handle_message(s: Bool, m: Request): Bool { match m { | Start(p, reply) -> { send(p, reply); false } | Receipt -> true } } } actor Processor { handle_message(reply: Actor<Gateway>): unit { send(reply, Receipt); } } property "done" { reachable (exists (g in instances(Gateway)) { g.state == Some(true) }) } check C { spawn_bound Gateway = 1 spawn_bound Processor = 1 mailbox_bound = 1 main { let p = spawn(Processor); let g = spawn(Gateway); send(g, Start(p, g)); } }';
  await page.goto(await start("receipt-flow", source));
  await expect(page.locator(".react-flow__node")).toHaveCount(2);
  const p = page
      .locator(".react-flow__node")
      .filter({ hasText: "Processor #0" }),
    g = page.locator(".react-flow__node").filter({ hasText: "Gateway #0" });
  expect((await p.boundingBox())!.x).toBeLessThan((await g.boundingBox())!.x);
  await page.getByRole("tab", { name: "Entities", exact: true }).click();
  await expect(page.locator(".entity-list button").first()).toContainText(
    "Processor #0",
  );
  await page.getByRole("button", { name: "Next step", exact: true }).click();
  await expect(
    page.locator(".active-flow .react-flow__edge-path"),
  ).toHaveAttribute("marker-start", /url/);
  const path = await page
    .locator(".active-flow .react-flow__edge-path")
    .getAttribute("d");
  const id = await page.locator(".active-flow").getAttribute("data-id");
  await page.getByRole("button", { name: "Next step", exact: true }).click();
  await expect(page.locator(".message-flow")).toHaveCount(1);
  await expect(page.locator(".active-flow")).toContainText("Receipt");
  await expect(page.locator(".active-flow")).toHaveAttribute("data-id", id!);
  await expect(
    page.locator(".active-flow .react-flow__edge-path"),
  ).toHaveAttribute("d", path!);
  await expect(
    page.locator(".active-flow .react-flow__edge-path"),
  ).toHaveAttribute("marker-end", /url/);
  await expect(
    page.locator(".active-flow .react-flow__edge-path"),
  ).not.toHaveAttribute("marker-start", /url/);
  await page.locator(".active-flow .react-flow__edge-textbg").click();
  await expect(page.locator(".entity-panel h2")).toHaveText("Gateway #0");
  await page.getByRole("button", { name: "Next step", exact: true }).click();
  await expect(page.locator(".past-flow")).toHaveCount(1);
  await expect(page.locator(".active-flow")).toHaveCount(0);
});
test("self-send has a visible directed loop instead of a zero-length edge", async ({
  page,
}) => {
  const source =
    'actor A { init(): Bool { false } handle_message(s: Bool, me: Actor<A>): Bool { send(me, me); true } } property "done" { reachable (exists (a in instances(A)) { a.state == Some(true) }) } check C { spawn_bound A = 1 mailbox_bound = 1 main { let a = spawn(A); send(a, a); } }';
  await page.goto(await start("self-flow", source));
  await page.getByRole("button", { name: "Last step" }).click();
  await expect(page.locator(".active-flow")).toHaveCount(1);
  const path = page.locator(".active-flow .react-flow__edge-path");
  await expect(path).toHaveAttribute("marker-end", /url/);
  expect(
    await path.evaluate(
      (element) => (element as SVGPathElement).getBBox().width,
    ),
  ).toBeGreaterThan(100);
  await page.screenshot({ path: "test-results/self-flow.png" });
});
test("payment failure explains the invariant and renders named fields with an always-visible diff", async ({
  page,
}) => {
  await page.goto(await start("payment-idempotency-bug"));
  await page
    .locator(".endpoint.violation")
    .filter({ hasText: "at most one charge" })
    .click();
  await expect(page.locator(".property-detail")).toContainText(
    "The invariant is false",
  );
  await expect(page.locator(".property-detail pre")).toContainText(
    "charges(processor.state) <= 1",
  );
  await expect(
    page.locator(".field-change").filter({ hasText: "charges" }).locator("del"),
  ).toHaveText("1");
  await expect(
    page.locator(".field-change").filter({ hasText: "charges" }).locator("ins"),
  ).toHaveText("2");
  await page.getByRole("tab", { name: "Entities", exact: true }).click();
  await page
    .locator(".entity-list button")
    .filter({ hasText: "Processor #0" })
    .click();
  await expect(
    page.locator(".state-field").filter({ hasText: "charges" }).locator("code"),
  ).toHaveText("2");
  await expect(page.locator(".state-fields")).toContainText("PaymentState");
  await expect(page.locator(".state-fields")).toContainText("PaymentA");
  await expect(page.locator(".state-fields")).not.toContainText("Variant");
  await expect(page.locator(".transition-panel")).toContainText("What changed");
  await page.screenshot({ path: "test-results/payment-debugger.png" });
});
test("saved choice branches merge prefixes and selecting them changes system state", async ({
  page,
}) => {
  const source =
    'type Pick = Zero | One | Two actor A { init(): Pick { Zero } handle_message(s: Pick, m: unit): Pick { let pick = choose([One, Two]); pick } } property "one" { reachable (exists (a in instances(A)) { a.state == Some(One) }) } property "two" { reachable (exists (a in instances(A)) { a.state == Some(Two) }) } check C { spawn_bound A = 1 mailbox_bound = 1 main { let a = spawn(A); send(a, ()); } }';
  await page.goto(await start("branches", source));
  await expect(page.getByRole("treeitem")).toHaveCount(3);
  await page.locator(".endpoint").filter({ hasText: "two" }).click();
  await expect(page.locator('[aria-current="step"]')).toContainText("choice 2");
  await page.locator(".react-flow__node").first().click();
  await expect(page.locator(".inspector")).toContainText("Two");
  await page.getByRole("button", { name: "First step" }).click();
  await expect(page.locator('[aria-current="step"]')).toContainText(
    "Initial state",
  );
  await page.locator(".endpoint").filter({ hasText: "one" }).click();
  await expect(page.locator('[aria-current="step"]')).toContainText("choice 1");
  await page.locator(".react-flow__node").first().click();
  await expect(page.locator(".inspector")).toContainText("One");
});
test("check --ui opens failed evidence and preserves the violation exit code", async ({
  page,
}) => {
  await page.goto(
    await start(
      "failed-check",
      'property "broken" { always false } check C { mailbox_bound = 1 main {} }',
      true,
    ),
  );
  await expect(page.locator(".brand")).toHaveText("flareml");
  await expect(page).toHaveTitle("flareml · Trace explorer");
  await expect(page.locator(".endpoint.violation")).toContainText("broken");
  await expect(page.locator(".step-number")).toHaveText("0 / 0");
  expect(readdirSync(join(directory, "failed-check-runs"))).toHaveLength(1);
  const good = join(directory, "passing.fml");
  writeFileSync(
    good,
    'property "safe" { always true } check C { mailbox_bound = 1 main {} }',
  );
  const output = execFileSync(
    binary,
    [
      "check",
      good,
      "--ui",
      "--no-open",
      "--artifacts-dir",
      join(directory, "passing-runs"),
    ],
    { timeout: 5000, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
  );
  expect(output).toContain("VERIFIED_IN_SCOPE");
});
test("fair lasso closing state is labeled as repetition, not termination", async ({
  page,
}) => {
  await page.goto(await start("missing-reply"));
  await expect(page.locator(".loop")).toContainText("Loop to");
  await expect(page.locator(".endpoint.violation")).toHaveCount(1);
  await page.getByRole("button", { name: "Last step" }).click();
  await expect(page.locator(".loop")).toContainText("closing state");
  await page.locator(".endpoint:not(.violation)").last().click();
  await expect(page.locator(".loop")).toHaveCount(0);
  await expect(page.locator(".timeline-top")).toContainText(
    "the counter commits",
  );
  await expect(page.locator(".react-flow__node")).toHaveCount(2);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await page.locator(".react-flow__node").first().click();
  await page.screenshot({ path: "test-results/minimal-explorer.png" });
});
