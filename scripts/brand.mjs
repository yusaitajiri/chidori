// Copies the kamon brand kit's tokens, Chidori's logos and its app icon into the app, which
// embeds them (crates/app/brand/, crates/app/icons/). Run after changing the kit's version in
// package.json:
//
//   pnpm install && pnpm brand
//
// The kit's install also copies the brand-ui skill into .claude/skills/ (brand-sync).

import { copyFileSync, mkdirSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const KIT = join(ROOT, "node_modules", "@yusai", "kamon-react");
const BRAND = join(ROOT, "crates", "app", "brand");

const copy = (from, to) => {
  mkdirSync(dirname(to), { recursive: true });
  copyFileSync(from, to);
  console.log(`${from} -> ${to}`);
};

copy(join(KIT, "tokens", "tokens.json"), join(BRAND, "tokens.json"));
copy(join(KIT, "LICENSE"), join(BRAND, "KAMON-LICENSE"));
for (const file of readdirSync(join(KIT, "logo")).filter((f) => f.startsWith("chidori"))) {
  copy(join(KIT, "logo", file), join(BRAND, "logo", file));
}
// The kit's .ico draws the small sizes properly.
copy(join(KIT, "icons", "chidori", "icon.ico"), join(ROOT, "crates", "app", "icons", "icon.ico"));
