import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
const names = new Set<string>();
function visit(name: string) {
  if (names.has(name)) return;
  names.add(name);
  const pkg = JSON.parse(
    readFileSync(join("node_modules", name, "package.json"), "utf8"),
  );
  for (const dependency of Object.keys(pkg.dependencies ?? {}))
    visit(dependency);
}
const root = JSON.parse(readFileSync("package.json", "utf8"));
for (const name of Object.keys(root.dependencies)) visit(name);
const sections = [...names].sort().map((name) => {
  const dir = join("node_modules", name);
  const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  const files = readdirSync(dir)
    .filter((n) => /^(licen[sc]e|copying|notice)([.\-]|$)/i.test(n))
    .sort();
  if (!files.length)
    throw new Error(
      `Missing license notice for ${name}; review before release`,
    );
  return `${name} ${pkg.version} (${pkg.license})\n${files.map((n) => readFileSync(join(dir, n), "utf8")).join("\n")}`;
});
writeFileSync(
  "dist/THIRD_PARTY_NOTICES.txt",
  "FML Explorer — third-party dependency notices\n\n" +
    sections.join("\n\n----------------------------------------\n\n"),
);
