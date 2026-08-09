<div align="center">
  <img src="./app-icon.png" width="96" height="96" alt="NextMail icon" />
  <h1>NextMail</h1>
  <p><strong>A calm, local-first desktop email client.</strong></p>
  <p>Fast offline reading, faithful mail rendering, and reliable delivery — without giving your inbox to another cloud.</p>

  <p>
    English
    ·
    <a href="./README_ZH.md">简体中文</a>
  </p>

  <p>
    <a href="https://github.com/nextmail-dev/nextmail/releases"><img alt="Latest release" src="https://img.shields.io/github/v/release/nextmail-dev/nextmail?display_name=tag&amp;label=release&amp;style=flat-square" /></a>
    <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&amp;logo=tauri&amp;logoColor=white" />
    <img alt="React 19" src="https://img.shields.io/badge/React-19-149ECA?style=flat-square&amp;logo=react&amp;logoColor=white" />
    <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-000000?style=flat-square&amp;logo=rust&amp;logoColor=white" />
    <img alt="Windows and macOS" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS-4C566A?style=flat-square" />
  </p>
</div>

> [!IMPORTANT]
> NextMail is currently a `0.2.2` preview. Windows 10 22H2+ x64 is the primary hands-on validation platform; macOS 12+ is a target platform. Linux packages are built for early testing, but Linux is not yet deeply adapted or validated.

## Preview

<!-- Replace the cells below with real screenshots when they are ready. Suggested files:
     docs/screenshots/main-workspace.png
     docs/screenshots/composer.png
     docs/screenshots/appearance.png
-->

<table>
  <tr>
    <td colspan="2" align="center">
      <br />
      <strong>Mail workspace</strong><br />
      <sub>Screenshot placeholder · main workspace</sub>
      <br /><br />
    </td>
  </tr>
  <tr>
    <td width="50%" align="center">
      <br />
      <strong>Rich composer</strong><br />
      <sub>Screenshot placeholder · compose and reply</sub>
      <br /><br />
    </td>
    <td width="50%" align="center">
      <br />
      <strong>Light &amp; dark</strong><br />
      <sub>Screenshot placeholder · themes and accent colors</sub>
      <br /><br />
    </td>
  </tr>
</table>

## Highlights

| | |
| --- | --- |
| **📬 Multiple accounts**<br />Add, edit, re-authenticate, switch, and safely remove IMAP/SMTP password accounts. | **⚡ Local-first reading**<br />Open the local mailbox immediately, then let background synchronization bring it up to date. |
| **✍️ Serious composing**<br />Rich text, HTML source, attachments, inline images, templates, signatures, drafts, replies, and forwarding. | **🛡️ Safe, faithful mail**<br />Keep common email layouts and inline images while scripts, forms, unsafe URLs, and remote content stay constrained. |
| **🔎 Offline search**<br />Search the current account and folder across subjects, addresses, previews, downloaded bodies, and attachment names. | **🗂️ Real folder workflows**<br />Create, rename, move, delete, reorder, and mark IMAP folders read without leaving the desktop app. |
| **🔁 Durable operations**<br />Reads, stars, moves, copies, deletes, drafts, and outgoing mail survive interruptions through persistent queues. | **🖥️ Desktop-native experience**<br />Dedicated windows, remembered geometry, native credential storage, notifications, bilingual UI, and flexible themes. |

## Designed around the inbox, not the cloud

### Local first, network second

NextMail treats the local mailbox as the primary reading surface. Existing mail appears before a network round trip, and server work continues in the background. Your chosen data directory remains portable; account passwords stay in the operating system credential store.

### Progressive by default

Synchronization makes useful content visible as early as possible: headers arrive first and each message becomes readable in the list as it is committed. Bodies are fetched on demand unless full-message synchronization is explicitly enabled for an account.

### Fidelity without surrendering safety

Email is messy HTML, not a normal web page. NextMail preserves the layouts, tables, author styles, CID images, and common legacy attributes that real mail depends on, while Rust-side sanitization and a sandboxed reader keep active content and unapproved remote resources outside the trust boundary.

### Failure is a state, not data loss

Mail changes and outgoing messages are recorded before network execution. Retries reuse durable intent instead of reconstructing it from UI state, and SMTP success is separated from Sent-folder archival so a filing failure cannot send the same message twice.

## What works today

- Password-based IMAP and SMTP accounts with TLS, STARTTLS, auto-discovery, and explicit confirmation for plaintext connections.
- Header-first synchronization across selectable folders, optional full-message synchronization, on-demand bodies, and offline raw EML recovery.
- Read/unread, star, move, copy, archive, delete, mark-all-read, folder management, and local sibling ordering.
- Safe HTML/CSS and plain-text reading, controlled remote images, CID/data images, original EML, attachment download, save, and system open.
- Local FTS5 search scoped to the current account and folder.
- Rich composing with Tiptap/ProseMirror and CodeMirror, explicit draft saving, Drafts/Sent synchronization, templates, signatures, and variables.
- Reply, reply all, and forward with complete original HTML, inline images, attachments, and stable signature placement.
- Account-scoped local contacts with contact suggestions, identity cards, and multi-select mail/contact actions.
- Chinese and English UI, system/light/dark appearance, accent colors, dedicated business windows, and NextMail desktop notifications.

For exact implementation details, engineering conventions, and current limitations, see the [project development guide](./docs/project.md).

## Downloads

Version tags build release assets for three desktop platforms on GitHub Actions:

| Platform | Build | Current support status |
| --- | --- | --- |
| Windows 10 22H2+ | x64 installers | Primary validation target |
| macOS 12+ | Separate Intel x64 and Apple Silicon arm64 apps | Target platform; ad-hoc signed, not notarized |
| Linux | x64 bundles from Ubuntu 22.04 | Experimental; no deep adaptation guarantee |

Download available builds from [GitHub Releases](https://github.com/nextmail-dev/nextmail/releases).

> [!WARNING]
> Preview artifacts do not yet use production Windows code signing or Apple notarization. Your operating system may show an unverified-developer warning. Only download builds from this repository.

## Development

Install Node.js, pnpm, Rust stable, and the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform.

```powershell
pnpm install
pnpm tauri dev
```

Run the frontend checks from the repository root:

```powershell
pnpm test
pnpm build
```

Run Rust checks from the single Tauri package:

```powershell
Push-Location src-tauri
cargo fmt --all -- --check
cargo test --offline --locked
cargo clippy --offline --locked --all-targets -- -D warnings
Pop-Location
```

Node.js dependencies are managed only with pnpm. The project does not currently use Python; future Python tooling must use uv.

## Documentation

- [Changelog](./CHANGELOG.md)
- [Project development guide](./docs/project.md)
- [Iteration records](./docs/iterations/)
- [Architecture decisions](./docs/adr/)
- [Third-party notices](./docs/third-party-notices.md)

## Scope

NextMail does not currently provide a unified inbox, conversation aggregation, cross-account search, a tray application, system notification-center integration, automatic updates, or production signing/notarization. These are not implied by the current preview or release workflow.

## License

The NextMail Rust package is declared under the MIT license. See [`src-tauri/Cargo.toml`](./src-tauri/Cargo.toml) and the [third-party notices](./docs/third-party-notices.md).
