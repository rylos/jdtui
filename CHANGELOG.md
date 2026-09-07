# Changelog

## 1.7.0 — 2026-09-07

- `o` opens a curated Settings panel: the twenty-odd settings of the
  JDownloader that get touched while downloads are running, out of the two
  thousand its advanced configuration holds. Types, values, defaults and the
  translated wording of every choice come from the device, so a setting it
  does not have is left out. `Enter` changes one, `r` puts it back to the
  default, and every change is read back so the panel shows what JDownloader
  kept rather than what was asked for.

## 1.6.0 — 2026-09-07

- `jdtui add --autostart` takes an answer as well as standing alone:
  `no`, `yes`, `true`, `false`, `on`, `off`, `1`, `0`, for a script whose
  answer sits in a variable. It overrules what a crawljob asked for.
- `jdtui add` takes files as well as urls: `.dlc`, `.ccf` and `.rsdf`
  containers and `.crawljob` files on this machine, the same ones the
  watched folder accepts.
- Panels keep a space before their border, so nothing reads as if it had
  been cut off. Two key descriptions that filled the column exactly are
  shorter.

## 1.5.0 — 2026-09-07

- `watch_folder` in the config names a folder to watch: `jdtui watch`
  takes it when given none, and the interface watches it in the
  background while it is open. `watch = false` stops that without
  forgetting the path.
- `jdtui watch [folder]` sends what is dropped into a folder on this
  machine to JDownloader, `.crawljob` files and `.dlc`, `.ccf`, `.rsdf`
  containers, filing each away afterwards. JDownloader cannot watch a
  folder it has no access to; this covers that. Suggested by the
  JDownloader team.
- `jdtui status` names the JDownloader that answered, which an account
  with more than one needs.
- jdtui answers questions in a script: `status`, `downloads`, `grabber`,
  `devices`, `start`, `stop`, `pause`, `resume` and `add`, in text or with
  `--json` for `jq`. `--device` picks the JDownloader. Suggested by the
  JDownloader team.
- A direct connection prefers IPv6 over IPv4 when both answer, since an
  IPv4 address is often behind carrier-grade NAT while IPv6 is native.
  Suggested by the JDownloader team.
- The device menu (`D`) opens an About panel: version and core revision,
  uptime, pending update, whether calls go direct or through the relay,
  the operating system, whether JDownloader runs in a container, its Java
  and heap, and the free space of every path it can write to.

## 1.4.0 — 2026-09-07

- Extraction is shown in jdtui's own words — `Extracting`, `Extract queued`,
  `Extracted`, `Extraction failed` — worked out from the extraction queue and
  each link's status instead of JDownloader's localised sentence, which was
  truncated and only readable in JDownloader's language.
- The ETA of an extraction is shown: JDownloader reports it on the links
  rather than on the package, and in milliseconds rather than seconds, so
  neither the package row nor the link rows used to show anything.
- Link rows show their own ETA while downloading, which they never did.
- Finished, downloading and disabled links are told apart by colour —
  green, cyan, grey — taken from their state, not from the words
  JDownloader wrote.
- The Status column has a share of the width of its own, and the header no
  longer cuts off what is waiting.
- `config.example.toml` documents every config key, and the README links
  to it.
- `direct_addresses` can be keyed by device name, so an extra address is
  tried for the JDownloader it belongs to and not for the others. A plain
  list still serves every device.

## 1.3.2 — 2026-09-06

- `p` pauses and resumes the downloads (was `P`); the properties of a row
  are on `i`, with `P` as an alias.
- Disabled packages and links are greyed out in both lists, as in the web
  interface: the recovery volumes JDownloader adds disabled stand out from
  what will download.

## 1.3.1 — 2026-09-06

- The header shows an idle or stopped list in grey instead of red, and an
  error in red instead of yellow: red now means trouble only.

## 1.3.0 — 2026-09-06

- Direct connection: calls go straight to JDownloader when one of the
  addresses it reports (or `direct_addresses` in the config) answers a
  ping, and back through the relay when it stops answering. The header
  shows `⇄ direct` or `☁ relay`. `direct = false` or `--no-direct` keeps
  everything on the relay.

## 1.2.0 — 2026-09-04

Live updates, video variants, and a few things the forms and lists were
missing.

### Live

- jdtui listens to JDownloader's event channel: changes made elsewhere show
  up within a second or two, and while nothing downloads the periodic
  refresh slows to thirty seconds. `events = false` in the config or
  `--no-events` turns it off. The header says `live` while the channel is
  up.

### Link Grabber

- Links that come in several variants (video qualities, audio only) show
  the chosen one, and the context menu switches it.
- The add-links form says where the files will go, reading JDownloader's
  default folder and its "subfolder by package name" packagizer rule.
- `Ctrl-F` on a "Save to" field picks from the folders JDownloader used
  lately, the folders of the packages in the lists and the mount points.

### Lists and forms

- `/` filters the list by name, hoster or status as you type; `Esc` clears
  it.
- `PgUp`/`PgDn` move by a page, `Home`/`End` (or `g`/`G`) jump to the first
  and last row.
- The form cursor no longer takes a cell of its own: the char under it is
  drawn in reverse, so the text does not shift while typing.

### Other

- Copy urls also goes through `wl-copy`, `xclip` or `xsel` when a display is
  at hand, for terminals without OSC 52.
- The device menu (`D`) offers to skip the captchas JDownloader is waiting
  on.

## 1.1.1 — 2026-09-04

- Text fields have a cursor: `←` `→`, `Home`/`End` (or Ctrl-A/Ctrl-E),
  `Delete`, and Ctrl-U to clear the field. Typing inserts at the cursor
  instead of appending.

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
