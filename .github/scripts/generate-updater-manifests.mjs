import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const TARGETS = [
  {
    label: "macOS Apple Silicon updater",
    pattern: /_aarch64\.app\.tar\.gz\.sig$/i,
    keys: ["darwin-aarch64", "darwin-aarch64-app"],
  },
  {
    label: "macOS Intel updater",
    pattern: /_x64\.app\.tar\.gz\.sig$/i,
    keys: ["darwin-x86_64", "darwin-x86_64-app"],
  },
  {
    label: "Linux AppImage updater",
    pattern: /_amd64\.appimage\.sig$/i,
    keys: ["linux-x86_64", "linux-x86_64-appimage"],
  },
  {
    label: "Linux Debian updater",
    pattern: /_amd64\.deb\.sig$/i,
    keys: ["linux-x86_64-deb"],
  },
  {
    label: "Linux RPM updater",
    pattern: /\.x86_64\.rpm\.sig$/i,
    keys: ["linux-x86_64-rpm"],
  },
  {
    label: "Windows MSI updater",
    pattern: /_x64(?:_[^.]+)?\.msi\.sig$/i,
    keys: ["windows-x86_64", "windows-x86_64-msi"],
  },
  {
    label: "Windows NSIS updater",
    pattern: /_x64-setup\.exe\.sig$/i,
    keys: ["windows-x86_64-nsis"],
  },
];

const REQUIRED_PLATFORM_KEYS = TARGETS.flatMap(({ keys }) => keys).sort();

async function listFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listFiles(entryPath)));
    } else if (entry.isFile()) {
      files.push(entryPath);
    }
  }

  return files;
}

function parseArguments(arguments_) {
  const options = {};

  for (let index = 0; index < arguments_.length; index += 2) {
    const name = arguments_[index];
    const value = arguments_[index + 1];
    if (!name?.startsWith("--") || value === undefined) {
      throw new Error(`Invalid argument near ${name ?? "end of command"}`);
    }
    options[name.slice(2)] = value;
  }

  for (const required of ["tag", "repo", "signatures", "notes", "output"]) {
    if (!options[required]) {
      throw new Error(`Missing required argument --${required}`);
    }
  }

  return options;
}

function validateManifest(manifest, tag, repo) {
  const platformKeys = Object.keys(manifest.platforms).sort();
  if (JSON.stringify(platformKeys) !== JSON.stringify(REQUIRED_PLATFORM_KEYS)) {
    throw new Error(`Unexpected updater platforms: ${platformKeys.join(", ")}`);
  }

  const expectedPrefix = `https://github.com/${repo}/releases/download/${encodeURIComponent(tag)}/`;
  for (const [key, platform] of Object.entries(manifest.platforms)) {
    if (!platform.signature || !platform.url.startsWith(expectedPrefix)) {
      throw new Error(`Invalid updater entry for ${key}`);
    }
  }
}

function validateCnManifest(manifest, cnManifest) {
  if (
    cnManifest.version !== manifest.version ||
    cnManifest.notes !== manifest.notes ||
    cnManifest.pub_date !== manifest.pub_date
  ) {
    throw new Error("CN updater metadata differs from the direct manifest");
  }

  for (const [key, platform] of Object.entries(manifest.platforms)) {
    const cnPlatform = cnManifest.platforms[key];
    if (
      cnPlatform?.signature !== platform.signature ||
      cnPlatform?.url !== `https://proxy.next-mail.app/${platform.url}`
    ) {
      throw new Error(`Invalid CN updater entry for ${key}`);
    }
  }
}

export async function generateManifests({
  tag,
  repo,
  signaturesDirectory,
  notesPath,
  outputDirectory,
  pubDate = new Date().toISOString(),
}) {
  if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag)) {
    throw new Error(`Unsupported release tag: ${tag}`);
  }
  if (!/^[0-9A-Za-z_.-]+\/[0-9A-Za-z_.-]+$/.test(repo)) {
    throw new Error(`Invalid GitHub repository: ${repo}`);
  }
  if (Number.isNaN(Date.parse(pubDate))) {
    throw new Error(`Invalid publication date: ${pubDate}`);
  }

  const signatureFiles = (await listFiles(signaturesDirectory)).filter((file) =>
    file.toLowerCase().endsWith(".sig"),
  );
  const platforms = {};
  const baseUrl = `https://github.com/${repo}/releases/download/${encodeURIComponent(tag)}`;

  for (const target of TARGETS) {
    const matches = signatureFiles.filter((file) => target.pattern.test(path.basename(file)));
    if (matches.length !== 1) {
      throw new Error(
        `Expected exactly one ${target.label} signature, found ${matches.length}`,
      );
    }

    const signaturePath = matches[0];
    const signature = (await readFile(signaturePath, "utf8")).trim();
    if (!/^[0-9A-Za-z+/=]+$/.test(signature)) {
      throw new Error(`Invalid signature content in ${path.basename(signaturePath)}`);
    }

    const artifactName = path.basename(signaturePath, ".sig");
    const entry = {
      signature,
      url: `${baseUrl}/${encodeURIComponent(artifactName)}`,
    };
    for (const key of target.keys) {
      platforms[key] = entry;
    }
  }

  const manifest = {
    version: tag.slice(1),
    notes: (await readFile(notesPath, "utf8")).trim(),
    pub_date: new Date(pubDate).toISOString(),
    platforms,
  };
  if (!manifest.notes) {
    throw new Error("Release notes are empty");
  }
  validateManifest(manifest, tag, repo);

  const cnManifest = {
    ...manifest,
    platforms: Object.fromEntries(
      Object.entries(platforms).map(([key, value]) => [
        key,
        { ...value, url: `https://proxy.next-mail.app/${value.url}` },
      ]),
    ),
  };
  validateCnManifest(manifest, cnManifest);

  await mkdir(outputDirectory, { recursive: true });
  await Promise.all([
    writeFile(path.join(outputDirectory, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`),
    writeFile(
      path.join(outputDirectory, "latest-cn.json"),
      `${JSON.stringify(cnManifest, null, 2)}\n`,
    ),
  ]);

  return { manifest, cnManifest };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const options = parseArguments(process.argv.slice(2));
  await generateManifests({
    tag: options.tag,
    repo: options.repo,
    signaturesDirectory: options.signatures,
    notesPath: options.notes,
    outputDirectory: options.output,
    pubDate: options["pub-date"],
  });
}
