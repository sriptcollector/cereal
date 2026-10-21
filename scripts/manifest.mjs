// Writes toolbay.json (the Toolbay hub's install manifest) with the real size and
// sha256 of each release asset. Usage: node scripts/manifest.mjs <assets-dir> [out]
// The installer bundles the speech and speaker engines and a starter speech model.
// "files" adds a notes AI model that works on any computer, so a hub install is
// complete in one step; Notes upgrades models for the hardware in the background.
import { createHash } from "node:crypto";
import { readFileSync, statSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const dir = process.argv[2] || ".";
const out = process.argv[3] || "toolbay.json";
const pkg = JSON.parse(readFileSync(join(import.meta.dirname, "..", "package.json"), "utf8"));
const v = pkg.version;
const brand = JSON.parse(readFileSync(join(import.meta.dirname, "..", "brand.json"), "utf8"));

function entry(asset, extra) {
  const p = join(dir, asset);
  const e = { asset, ...extra };
  if (existsSync(p)) {
    e.size = statSync(p).size;
    e.sha256 = createHash("sha256").update(readFileSync(p)).digest("hex");
  }
  return e;
}

const manifest = {
  schema: 1,
  id: "cereal",
  name: "Cereal",
  tagline: "Free, local AI meeting notes. No bot joins your call.",
  description:
    "Records your side and the other side of any call (Zoom, Google Meet, Teams, Slack) without a bot, transcribes it on your computer, tells speakers apart, and turns your rough notes into clean notes with decisions and action items. Search and chat across all your meetings. No account, no limits, no telemetry.",
  version: v,
  repo: "sriptcollector/cereal",
  identifier: "ai.toolbay.cereal",
  license: "MIT",
  homepage: "https://github.com/sriptcollector/cereal",
  icon: "https://raw.githubusercontent.com/sriptcollector/cereal/main/app-icon.png",
  replaces: brand.inspiredBy.name,
  inspiredBy: brand.inspiredBy,
  categories: ["productivity", "meetings", "ai"],
  platforms: {
    "windows-x86_64": entry(`Cereal_${v}_x64-setup.exe`, {
      kind: "nsis",
      // /NS: the hub makes its own desktop shortcut (opens Notes through the hub).
      silentArgs: ["/S", "/NS"],
      exe: "cereal.exe",
      uninstaller: "uninstall.exe",
    }),
    "darwin-aarch64": entry(`Cereal_${v}_aarch64.dmg`, { kind: "dmg", app: "Cereal.app" }),
    "darwin-x86_64": entry(`Cereal_${v}_x64.dmg`, { kind: "dmg", app: "Cereal.app" }),
  },
  files: [
    {
      url: "https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q4_K_M.gguf",
      size: 1107409472,
      sha256: "b139949c5bd74937ad8ed8c8cf3d9ffb1e99c866c823204dc42c0d91fa181897",
      path: "models/Qwen3-1.7B-Q4_K_M.gguf",
    },
  ],
  launch: {
    args: ["--from-toolbay"],
    singleInstance: true,
    control: { show: ["--show"], hide: ["--hide"], quit: ["--quit"], start: ["--start"], stop: ["--stop"] },
    stateFile: "running.json",
  },
};

writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
console.log(`wrote ${out} for ${v}`);
