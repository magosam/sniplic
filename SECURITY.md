# Security Policy

Sniplic Core processes complex audio and video formats, manipulates timeline boundaries, and exposes Node.js bindings (NAPI-RS). Treat files, paths, metadata, automation requests, and command parameters as potentially malicious, especially when accepting user-generated media.

## Reporting a Vulnerability

Please give the repository maintainers a reasonable opportunity to investigate and coordinate a fix before publishing exploitable details, proof-of-concept code, or weaponized media files.

This repository does not currently document a dedicated private security email or private reporting channel. Contact the repository maintainers through the repository owner/maintainer channels already listed in the project, provide only a minimal non-exploitable summary in public, and ask for an appropriate private transfer method before sharing sensitive details. Do not open a public issue containing a working exploit, secret, sensitive crash dump, or malicious attachment.

If no private channel can be established, withhold weaponized material and report the smallest safe description that allows maintainers to make contact.

## What to Include

A useful report includes:
* affected Sniplic Core version, commit, or release artifact;
* operating system, architecture, build profile, and relevant feature flags;
* the affected component or path (e.g., Timeline state, Node wrapper, FFmpeg graph builder);
* security impact and required attacker access;
* exact reproduction steps;
* expected behavior and actual behavior;
* crash logs, backtraces, sanitizer output, or resource measurements when applicable;
* whether the problem reproduces from a clean checkout;
* any known workaround or containment.

Remove credentials, personal paths, private media content, and unrelated system information from logs.

## Malicious Test Files

Do not attach a potentially harmful or non-redistributable media file to a public issue. Initially provide its format, size, cryptographic hash, observed effect, provenance category (for example, synthetic or fuzz-generated), and the command needed to reproduce the problem. Coordinate a private transfer method with maintainers before sending the sample.

Test files accepted into the repository must be minimized, free of personal or proprietary data, legally redistributable, and documented with the expected failure behavior. A fixed parser defect should receive a permanent regression test whenever safe and practical.

## Scope

Security reports may cover:
* Media import parsing, FFprobe spoofing, or format confusion;
* Decompression bombs, oversized video dimensions, integer overflow, memory exhaustion, recursion, or hangs during timeline rendering;
* Import/export and filesystem path handling, including traversal, symlinks, and unsafe overwrite behavior during FFmpeg rendering or proxy generation;
* Engine JSON validation and state serialization/deserialization;
* Node.js boundary limits, batch API limits, and NAPI-RS thread resource exhaustion;
* Dependencies, CI, packaging, signing, checksums, and release artifact integrity.

General bugs without a security impact should use the project's normal issue and contribution process.
