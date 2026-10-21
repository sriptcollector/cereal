// Downloads what ships inside the installer, so Cereal works the moment it's
// installed: the speech engine (whisper.cpp, CPU) with a starter model, the notes
// engine (llama.cpp, CPU), and the speaker engine (sherpa-onnx) with its two small
// models. The notes AI model itself (1-2.5 GB, picked for the computer) comes through
// the Toolbay hub's install ("files" in toolbay.json) or downloads on first start.
// Runs before every `tauri build`. Skips files already present.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync, copyFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "..", "src-tauri", "bundled");
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || "";
const os = triple.includes("windows") ? "windows" : triple.includes("apple") ? "macos" : process.platform === "win32" ? "windows" : process.platform === "darwin" ? "macos" : "linux";
const arch = triple ? (triple.startsWith("aarch64") ? "arm64" : "x86_64") : process.arch === "arm64" ? "arm64" : "x86_64";
const SH = "https://github.com/k2-fsa/sherpa-onnx/releases/download";

const MODELS = [
  { file: "ggml-base.en-q5_1.bin", url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en-q5_1.bin", sha256: "4baf70dd0d7c4247ba2b81fafd9c01005ac77c2f9ef064e00dcf195d0e2fdd2f" },
  { file: "3dspeaker_speech_campplus_sv_en_voxceleb_16k.onnx", url: `${SH}/speaker-recongition-models/3dspeaker_speech_campplus_sv_en_voxceleb_16k.onnx` },
  { file: "sherpa-onnx-pyannote-segmentation-3-0.onnx", url: `${SH}/speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2`, inner: "model.onnx" },
];

const ENGINES = {
  windows: [
    { id: "whisper-cpu", url: "https://github.com/ggml-org/whisper.cpp/releases/download/b5454/whisper-bin-x64.zip", exe: "whisper-server.exe", keep: (n) => n === "whisper-server.exe" || (n.endsWith(".dll") && !/^(SDL2|parakeet|llama)\.dll$/i.test(n)) },
    { id: "llama-cpu", url: "https://github.com/ggml-org/llama.cpp/releases/download/b11748/llama-b11748-bin-win-cpu-x64.zip", exe: "llama-server.exe", keep: (n) => n === "llama-server.exe" || n.endsWith(".dll") },
    { id: "sherpa-cpu", url: `${SH}/v1.13.8/sherpa-onnx-v1.13.8-win-x64-shared-MD-Release-no-tts.tar.bz2`, exe: "sherpa-onnx-offline-speaker-diarization.exe", keep: (n) => n === "sherpa-onnx-offline-speaker-diarization.exe" || /^(onnxruntime|onnxruntime_providers_shared|sherpa-onnx-c-api|sherpa-onnx-cxx-api)\.dll$/i.test(n) },
  ],
  macos: [
    { id: arch === "arm64" ? "whisper-metal" : "whisper-mac-intel", url: `https://github.com/sriptcollector/toolbay-flow/releases/download/engines-v1/whisper-server-macos-${arch}.tar.gz`, exe: "whisper-server", keep: (n) => n === "whisper-server" },
    { id: arch === "arm64" ? "llama-metal" : "llama-mac-intel", url: `https://github.com/ggml-org/llama.cpp/releases/download/b11748/llama-b11748-bin-macos-${arch === "arm64" ? "arm64" : "x64"}.tar.gz`, exe: "llama-server", keep: (n) => n === "llama-server" || n.endsWith(".dylib") || n.endsWith(".metal") },
    // macOS: keep the bin/ + lib/ layout (the tool finds its dylibs through ../lib).
    { id: "sherpa-cpu", url: `${SH}/v1.13.8/sherpa-onnx-v1.13.8-osx-universal2-shared-no-tts.tar.bz2`, exe: "sherpa-onnx-offline-speaker-diarization", tree: true },
  ],
}[os];

async function download(url, dest) {
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  writeFileSync(dest + ".part", Buffer.from(await res.arrayBuffer()));
  renameSync(dest + ".part", dest);
}
const sha256 = (f) => createHash("sha256").update(readFileSync(f)).digest("hex");
// Windows' own bsdtar reads zip and bz2; Git Bash's GNU tar may not.
const tar = process.platform === "win32" ? join(process.env.SystemRoot || "C:\\Windows", "System32", "tar.exe") : "tar";
const walk = (d) => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : [join(d, f)]));

async function main() {
  const models = join(root, "models");
  mkdirSync(models, { recursive: true });
  for (const m of MODELS) {
    const dest = join(models, m.file);
    if (existsSync(dest)) continue;
    console.log(`bundle: downloading ${m.file}`);
    if (m.inner) {
      const tmp = join(root, "tmp-model");
      rmSync(tmp, { recursive: true, force: true });
      mkdirSync(tmp, { recursive: true });
      await download(m.url, join(tmp, "pkg.tar.bz2"));
      execFileSync(tar, ["-xf", join(tmp, "pkg.tar.bz2"), "-C", tmp]);
      const f = walk(tmp).find((p) => p.endsWith(m.inner));
      copyFileSync(f, dest);
      rmSync(tmp, { recursive: true, force: true });
    } else {
      await download(m.url, dest);
    }
    if (m.sha256 && sha256(dest) !== m.sha256) {
      rmSync(dest);
      throw new Error(`${m.file}: checksum mismatch`);
    }
  }
  if (!ENGINES) {
    console.log("bundle: no engines for this platform");
    return;
  }
  for (const e of ENGINES) {
    const dir = join(root, "engines", e.id);
    if (existsSync(dir) && walk(dir).some((f) => f.endsWith(e.exe))) continue;
    const tmp = join(root, "tmp");
    rmSync(tmp, { recursive: true, force: true });
    mkdirSync(tmp, { recursive: true });
    const pkg = join(tmp, e.url.endsWith(".zip") ? "pkg.zip" : e.url.endsWith(".bz2") ? "pkg.tar.bz2" : "pkg.tar.gz");
    console.log(`bundle: downloading ${e.url}`);
    await download(e.url, pkg);
    execFileSync(tar, ["-xf", pkg, "-C", tmp]);
    rmSync(pkg);
    mkdirSync(dir, { recursive: true });
    if (e.tree) {
      const top = walk(tmp).find((f) => f.endsWith("/" + e.exe) || f.endsWith("\\" + e.exe));
      const base = join(top, "..", "..");
      cpSync(join(base, "bin", e.exe), join(dir, "bin", e.exe));
      cpSync(join(base, "lib"), join(dir, "lib"), { recursive: true, filter: (s) => statSync(s).isDirectory() || s.endsWith(".dylib") });
      execFileSync("chmod", ["+x", join(dir, "bin", e.exe)]);
    } else {
      for (const f of walk(tmp)) {
        const name = f.split(/[\\/]/).pop();
        if (e.keep(name)) copyFileSync(f, join(dir, name));
      }
      if (os !== "windows") execFileSync("chmod", ["+x", join(dir, e.exe)]);
    }
    rmSync(tmp, { recursive: true, force: true });
  }
  console.log(`bundle: ready (${os}-${arch})`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
