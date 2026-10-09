# Security

## Reporting a vulnerability

Please report security issues privately through [GitHub's private vulnerability reporting](https://github.com/kowalunioo/echo/security/advisories/new), not in a public issue. Include the Echo version, what you found and how to reproduce it. You will get a reply within a week.

## Supported versions

Only the latest release receives fixes. Echo's in-app updater installs it automatically unless updates are turned off.

## Scope

Echo never sends audio, Transcripts, Vocabulary or settings over the network. Its only network traffic is Model downloads, verified against their SHA-256, and update checks, where an update installs only if its signature matches the key built into the app. Reports about either path, about the keyboard hook or about text insertion are especially welcome.
