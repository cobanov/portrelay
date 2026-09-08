import { cp, mkdir, readFile, readdir, rm, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = path.join(root, "web");
const output = path.join(root, "dist");
const html = await readFile(path.join(source, "index.html"), "utf8");
const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]);
if (new Set(ids).size !== ids.length) throw new Error("Duplicate HTML IDs");
for (const [, target] of html.matchAll(/(?:href|src)="([^"]+)"/g)) {
  if (target === "#" || /^(https?:|mailto:|data:)/.test(target)) continue;
  if (target.startsWith("#")) {
    if (!ids.includes(target.slice(1)))
      throw new Error(`Missing anchor: ${target}`);
  } else {
    const asset = path.resolve(source, "." + target);
    if (!asset.startsWith(source + path.sep) || !(await stat(asset)).isFile()) {
      throw new Error(`Invalid public asset: ${target}`);
    }
  }
}
const config = JSON.parse(
  await readFile(path.join(root, "wrangler.json"), "utf8"),
);
if (config.pages_build_output_dir !== "dist")
  throw new Error("Unexpected deployment directory");
await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });
await cp(source, output, { recursive: true });
console.log(
  `Built ${(await readdir(output)).length} public files. HTML links and assets verified.`,
);
