const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");

const html = fs.readFileSync("site/index.html", "utf8");
const installerUrl = "https://github.com/SanXiaoXing/Codex-for-win/releases/download/v0.1.3/Win.CodexBar_0.1.3_x64-setup.exe";

// The page must still offer a real download and version if the API or JavaScript fails.
assert.equal((html.match(/href="https:\/\/github\.com\/SanXiaoXing\/Codex-for-win\/releases\/download\/v0\.1\.3\/Win\.CodexBar_0\.1\.3_x64-setup\.exe" data-release-download/g) || []).length, 3);
assert.equal((html.match(/data-release-version>v0\.1\.3</g) || []).length, 3);

const script = html.match(/<script id="release-data">([\s\S]*?)<\/script>/)?.[1];
assert.ok(script, "Release logic must ship inside index.html");

async function renderRelease(response) {
  const elements = new Map([
    ["[data-release-version]", [{ textContent: "v0.1.3" }]],
    ["[data-release-download]", [{ href: installerUrl }]],
    ["[data-release-button-version]", [{ textContent: " v0.1.3" }]],
    ["[data-release-size]", [{ textContent: "3.1 MB · NSIS" }, { textContent: "3.1 MB" }]],
  ]);
  const document = {
    querySelectorAll(selector) {
      return elements.get(selector) || [];
    },
  };
  const context = { document, console: { warn() {} }, fetch: async () => response };
  vm.runInNewContext(script, context);
  await new Promise(setImmediate);
  return elements;
}

(async () => {
  const nextUrl = "https://github.com/SanXiaoXing/Codex-for-win/releases/download/v0.2.0/Win.CodexBar_0.2.0_x64-setup.exe";
  const next = await renderRelease({ ok: true, json: async () => ({ tag_name: "v0.2.0", assets: [{ name: "Win.CodexBar_0.2.0_x64-setup.exe", browser_download_url: nextUrl, size: 4194304 }] }) });
  assert.equal(next.get("[data-release-download]")[0].href, nextUrl);
  assert.equal(next.get("[data-release-version]")[0].textContent, "v0.2.0");

  const failed = await renderRelease({ ok: false, status: 503 });
  assert.equal(failed.get("[data-release-download]")[0].href, installerUrl);

  const missing = await renderRelease({ ok: true, json: async () => ({ tag_name: "v0.2.0", assets: [] }) });
  assert.equal(missing.get("[data-release-download]")[0].href, installerUrl);
  assert.equal(missing.get("[data-release-version]")[0].textContent, "v0.1.3");
  console.log("Release links and version passed");
})().catch((error) => { console.error(error); process.exitCode = 1; });
