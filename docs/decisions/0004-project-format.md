# ADR 0004: Project format: directory bundle for work, ZIP package for transport

- **Status:** Accepted (2026-10-20)

## Context

The PRD asks for a `.sonara` format, preferably ZIP-compatible, with a manifest, integrity
hashes, atomic saves and migrations. It also says gigabytes of media must not be rewritten on
every save.

A ZIP archive cannot remove or replace a member in place. Saving an edited `project.json`
into an archive means writing a new archive, which copies all media.

## Options considered

1. **ZIP only.** Simple to distribute, but every save rewrites all media, and autosave
   becomes prohibitively expensive for large sessions.
2. **Directory only.** Saves are cheap, but the project is awkward to email or attach and
   easy to break by copying it partially.
3. **Both, with the same logical layout.** A directory bundle (`Name.sonara/`) is the working
   representation. A ZIP archive (`Name.sonara` as a file) is the packaged form for
   transport and archiving.

## Decision

Option 3. Both forms contain the same members:

```text
manifest.json   format id and version, project id, required features, asset inventory
                (relative path, size, SHA-256), and the SHA-256 of project.json
project.json    the project document (JSON, UTF-8, sorted keys, human-inspectable)
media/audio/    imported and recorded audio; files are immutable once written
media/midi/     imported Standard MIDI Files (kept for provenance)
media/impulses/ impulse responses for convolution
presets/        device presets saved with the project
analysis/       cached analysis results (regenerable)
thumbnails/     optional preview image
```

Bundle-only members such as `backups/`, `.autosave/` and `cache/` are never packaged.

- **Opening** a ZIP extracts it with path-traversal, symlink and size checks into a managed
  working directory. Saving writes a new archive to a temporary file, fsyncs it and
  atomically replaces the original. The documentation recommends bundles for large sessions.
- **Saving a bundle** writes `project.json` and then `manifest.json`, each as
  write-temp, fsync and `os.replace`. The manifest records the hash of `project.json`. A
  mismatch on open means the save was interrupted. The loader then falls back to the newest
  valid file in `backups/` and reports what it recovered.
- **Media** is added to the bundle as files and never rewritten. Linked (external) media is
  recorded with its absolute path and, where possible, a path relative to the bundle, plus
  its size and hash for relinking.
- **Format versioning:** `manifest.format_version` is an integer. Migrations are pure
  functions `dict -> dict`, one per version step, and run before schema validation. Unknown
  optional keys are preserved through an `extra` map on each major object.
- **Schema validation** uses JSON Schema (draft 2020-12), shipped inside the package.

## Consequences

Two code paths (bundle and archive) share one serializer and one validator. Users can see a
`.sonara` "file" that is either a directory or a ZIP. Sonara detects which form it is, and
macOS can register the directory form as a document package.
