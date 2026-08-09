import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { generateManifests } from "./generate-updater-manifests.mjs";

const SIGNATURE_FILES = [
  "NextMail_1.2.3_aarch64.app.tar.gz.sig",
  "NextMail_1.2.3_x64.app.tar.gz.sig",
  "NextMail_1.2.3_amd64.AppImage.sig",
  "NextMail_1.2.3_amd64.deb.sig",
  "NextMail-1.2.3-1.x86_64.rpm.sig",
  "NextMail_1.2.3_x64_en-US.msi.sig",
  "NextMail_1.2.3_x64-setup.exe.sig",
];

test("generates direct and CN updater manifests from release signatures", async () => {
  const temporaryDirectory = await mkdtemp(path.join(os.tmpdir(), "nextmail-updater-"));

  try {
    const signaturesDirectory = path.join(temporaryDirectory, "signatures");
    const outputDirectory = path.join(temporaryDirectory, "output");
    const notesPath = path.join(temporaryDirectory, "notes.md");
    await Promise.all([
      mkdir(signaturesDirectory),
      writeFile(notesPath, "### Fixed\n\n- Updater manifests."),
    ]);
    await Promise.all(
      SIGNATURE_FILES.map((name) =>
        writeFile(path.join(signaturesDirectory, name), "U0lHTkFUVVJF\n"),
      ),
    );

    const { manifest, cnManifest } = await generateManifests({
      tag: "v1.2.3",
      repo: "nextmail-dev/nextmail",
      signaturesDirectory,
      notesPath,
      outputDirectory,
      pubDate: "2026-08-09T12:00:00Z",
    });

    assert.equal(manifest.version, "1.2.3");
    assert.equal(Object.keys(manifest.platforms).length, 11);
    assert.equal(
      manifest.platforms["windows-x86_64"].url,
      "https://github.com/nextmail-dev/nextmail/releases/download/v1.2.3/NextMail_1.2.3_x64_en-US.msi",
    );
    assert.equal(
      cnManifest.platforms["windows-x86_64"].url,
      `https://proxy.next-mail.app/${manifest.platforms["windows-x86_64"].url}`,
    );
    assert.deepEqual(
      JSON.parse(await readFile(path.join(outputDirectory, "latest.json"), "utf8")),
      manifest,
    );
  } finally {
    await rm(temporaryDirectory, { recursive: true, force: true });
  }
});
