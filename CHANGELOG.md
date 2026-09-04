# Changelog

## 1.1.0 — 2026-09-04

Everything the My.JDownloader API offers for day-to-day use is now reachable
from the terminal.

### Downloads

- Pause and resume on `P`; the header shows the paused state.
- Stop mark: `t` or the context menu makes the download list stop after a
  row. Marking a package puts the mark on its last link, since the API marks
  links only.
- Resume, unskip, check availability and extract now in the context menu.
- The total speed comes from the download controller instead of being summed
  from the links.

### Packages and links

- Set the priority of a selection.
- Rename a package or a link.
- Set the download folder of packages.
- Move a selection to a new package; split a package by hoster.
- Show the urls of a selection and copy them to the clipboard through the
  terminal (OSC 52), which also works over SSH.

### Link Grabber

- `C` clears the list, `x` aborts crawling, and the tab says when JDownloader
  is still collecting links.
- `e` adds a password to the list tried on every archive.

### JDownloader

- `A` lists the premium accounts with traffic and expiry, and enables,
  disables or refreshes them.
- `D` opens a menu on the JDownloader itself: check for updates, update and
  restart (only when an update is reported), restart, reconnect, exit.
- The header shows captchas waiting to be solved and archives being extracted.

### Interface

- `?` opens a key reference; the footer keeps the frequent keys only.
- A refresh stays at four round trips: the rarely changing status is fetched
  every fifth refresh and right after an action.

## 1.0.0 — 2026-09-04

First release.
