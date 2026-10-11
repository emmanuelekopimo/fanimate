# Security policy

## Reporting a vulnerability

Please **do not open a public issue** for security problems. Report them privately through GitHub's [private vulnerability reporting](../../security/advisories/new) for this repository.

Include what you found, how to reproduce it, and what an attacker could do with it. You will get an acknowledgement within a week.

## What counts

Fanimate opens project files that other people make, so the most important area is **file loading**: a crafted `.fnm`, Moho, Animate, SVG or PSD file that crashes the app, runs code, reads or writes files outside the project, or uses unbounded memory or time.

Later, the plugin sandbox and the Fanimate Hub server will be in scope too.

## Supported versions

Fanimate has not had a release yet. Until 1.0, fixes go into `main` only.
