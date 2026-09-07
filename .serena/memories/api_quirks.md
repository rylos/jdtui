# My.JDownloader API quirks (verified live on 2026-09-04)

- `downloadcontroller/getCurrentState` values: `IDLE`, `RUNNING`, `PAUSE`, `STOPPING`, `STOPPED_STATE` (no "STOPPED", no "DOWNLOADING"). `pause(true)` is silently ignored unless RUNNING.
- Stop mark (`downloadsV2/setStopMark(linkId, packageId)`): links only. Passing a package id marks its *first* link; jdtui marks the *last* link of a package instead (`model::stop_mark_target`). `getStopMark` returns link uuid, `0` = hidden, `-1` = none. Remove with `removeStopMark` or `setStopMark(-1,-1)`.
- `downloadsV2/unskip(packageIds, linkIds, filterByReason)` — package ids FIRST, opposite of every other call; `null` reason = unskip all.
- `getDownloadUrls(linkIds, packageIds, ["CONTENT"])` returns `{url: [ids]}` map; keys are the urls.
- `movetoNewPackage(linkIds, pkgIds, name, downloadPath)` — null path keeps the current folder; exists on both downloadsV2 and linkgrabberv2, same for `splitPackageByHoster`, `renameLink/Package`, `setPriority`, `setDownloadDirectory` (packages only), `startOnlineStatusCheck`.
- `extraction/getQueue` → `[{archiveId, archiveName, controllerId, controllerStatus: RUNNING|QUEUED}]`; `startExtractionNow` only queues *complete* archives.
- `accountsV2/listAccounts(query)` needs boolean field selectors like the package queries; `validUntil` is epoch ms, `trafficLeft` -1 = unlimited.
- Each relay round trip ≈ 110 ms; keep the per-refresh call count at 4 (see poller).
- Events (`/events`): `subscribe([regex], [regex])` → `{subscriptionid, maxPolltimeout: 25000, maxKeepalive: 120000}`; patterns are Java regex `find()` on `"publisher.eventid"`. `listen(id)` blocks ≤25 s through the relay fine (needs a per-call HTTP timeout > 25 s: `MyJd::device_call_with_timeout`), returns `[]` on timeout, `[{eventid, publisher, eventData?}]` otherwise (note capital D). `getsubscriptionstatus.subscribed` is `false` whenever no listen is pending — not a liveness signal. Publishers: `downloads`, `downloadwatchdog`, `linkcollector`, `linkcrawler`, `captchas`, `dialogs`; `extraction` accepted but not listed. Adding a link from another session yields linkcrawler STARTED/STOPPED/FINISHED then linkcollector CONTENT_ADDED/STRUCTURE_REFRESH/LINK_ADDED within ~2 s.
- Grabber `queryLinks` flags `variants`, `variantID`, `variantName` → link `variants: bool`, `variant: {id, name}`. `getVariants(linkid)` / `setVariant(linkid, id)`. YouTube crawls take 15–60 s and the plugin renames the package (do not look it up by name).
- Multiple concurrent sessions of the same account work (poller + listener + tests).
- No filesystem listing in the API. Folder sources: `linkgrabberv2/getDownloadFolderHistorySelectionBase` (recent folders incl. `<jd:packagename>` placeholders, as the GUI combo), `system/getStorageInfos(null)` (mount points with free/size). `<jd:packagename>` in `destinationFolder` is expanded by the packagizer; "subfolder by package name" is the packagizer static rule `SubFolderByPackageRule` (`downloadDestination: "<jd:packagename>"`, `enabled`, `matchAlwaysFilter.enabled`) in `config/get(PackagizerSettings, null, "RuleList")`, gated by `PackagizerEnabled`; default folder is `config/get(GeneralSettings, null, "DefaultDownloadFolder")`. `JdApi::folder_policy` reads the three (~0.4 s) and `model::resolve_folder` previews the path. The rule applies on add (crawl), NOT on `setDownloadDirectory`/`movetoNewPackage`. Never add a "subfolder" flag of our own: the placeholder in the path does it, and doubles when the rule is on.
- `config/list(pattern, docs, values, defaults, enumInfo)` — 5 args, regex over "interfaceName.key". The no-arg form returns 2172 entries over 222 interfaces (mostly per-hoster plugin options) with NO values, and times out over the relay at the default timeout — always pass a pattern. `config/query` with `{pattern}` returned nothing — use `list`.
- `config/get|set(interfaceName, storage, key[, value])`: `storage` must be the literal string `"null"` when the entry reports none (`null` the JSON value → BAD_PARAMETERS). `set` takes a raw JSON value and answers `true`/`false`.
- `config/listEnum(javaType)` returns `[{name, label}]`; `label` is translated into JD's own language (Italian on jd2@docker) and is often ABSENT (e.g. `AutoDownloadStartOption`) — jdtui ignores it and carries its own English wording in `options.rs`. Constants seen: IfFileExistsAction {OVERWRITE_FILE, SKIP_FILE, AUTO_RENAME, ASK_FOR_EACH_FILE}, AutoDownloadStartOption {ALWAYS, ONLY_IF_EXIT_WITH_RUNNING_DOWNLOADS, NEVER}, AutoStartOptions {AUTO, DISABLED, ENABLED}, DeleteOption {NO_DELETE, RECYCLE, NULL} — `NULL` means "delete permanently".
- Units are not reported: `GeneralSettings.DownloadSpeedLimit` is bytes/s, `ForcedFreeSpaceOnDisk` is megabytes. That knowledge lives in `options.rs`.
- Live tests create packages named `jdtui-*` in the grabber; if a test aborts mid-way, leftovers must be removed by hand (`cargo test -- --ignored` again, or the GUI).

## Direct connection (verified live 2026-09-06)

- `device/getDirectConnectionInfos` (namespace `device`, not `jd`) → `{infos:[{ip,port}], mode: "LAN_WAN_MANUAL"|…, rebindProtectionDetected}`. Takes ~1.5 s on JD's side (it works out its addresses every call). A Docker JD lists only its container IP (may even be stale), 127.0.0.1 and the WAN IP it detects — not the Docker host's LAN IP.
- Direct calls: same encrypted body, same `/t_<session>_<device>/path`, base `http://ip:port` (https works too; the web UI uses https via `<ip-with-dashes>.mydns.jdownloader.org` only for mixed-content reasons). The web UI (`https://my.jdownloader.org/jdapi/jdapi.min.js`, `_pingForAvailability`) pings all infos with `/device/ping` in parallel and keeps the fastest (`setLocalURL`); the icon is `isInLocalMode()`. jdtui: `MyJd::probe_direct` (scoped threads, `PROBE_TIMEOUT` 1 s), `JdApi::ensure_direct(extra)` from the poller (retry every `PROBE_RETRY` 5 min while on relay), transport error on a direct call → `direct = None` and the call is retried on the relay at once; `DIRECT_TIMEOUT` 5 s for direct calls without their own timeout.
- Shape of a real setup (addresses redacted; this file is in a public repo): JD reports its WAN address, which from inside the same network is no faster than the relay; the public address works only from outside, since the router does no hairpin NAT; the LAN address of the Docker host is the one worth having (~3 ms vs ~115 ms) and JD cannot report it, so it has to come from `direct_addresses` in the config.

- queryLinks/queryPackages omit boolean fields that are false: a disabled link comes with no `enabled` key (and no `status`), so `Option<bool>` None must read as false (`is_enabled()` does).

## Extraction (verified live 2026-09-06)

- No `extraction` event publisher exists (`events/listpublisher`: captchas, downloadwatchdog, downloads, linkcollector, linkcrawler, dialogs). Extraction shows up only as `downloads.LINK_UPDATE.extractionStatus` (IDLE → null while running → SUCCESSFUL) and `extraction/getQueue` (no progress). The localized `status` text of package/link ("Estrazione OK: …") is the only place JD reports extraction progress; jdtui shows that text as-is, no extraction panel (decided not worth it).

- A JDownloader in Docker reports its container address, 127.0.0.1 and the public address, and none of the three is reachable from another machine on the same LAN — hence the config key. An apparent TCP "reset" from JD's direct port is worth suspecting as a malformed probe before a firewall: JD resets a request whose path lacks the `/t_<session>_<device>` prefix.

## Extraction state, language-independent (verified live 2026-09-07)

- JD's `status` on package/link is a sentence in JD's own UI language ("Estrazione OK: film.part01.rar", "Entpacken (ETA: 41s)") — never parse it, never rely on it for a state.
- `extraction/getArchiveInfo(linkIds, packageIds)` and `extraction/getQueue` return the same `ArchiveStatus`: `archiveId`, `archiveName` (base name, no `.partNN.rar`), `controllerId`, `controllerStatus` (`RUNNING`/`QUEUED` in the queue, `NA` otherwise), `type` (RAR_MULTI…), and **`states`: a map member-filename → COMPLETE/INCOMPLETE/MISSING**. Those keys are link names, which is how jdtui matches a queued archive to its package (`model::extraction_of`).
- Per-link `extractionStatus` is language-independent: `SUCCESSFUL`, `ERROR*`, `IDLE`, or absent. It is the outcome of a finished run, so a package still downloading must not be called "Extracted" (jdtui gates that on `package.is_finished()`).
- Still no extraction percentage anywhere in the structured API; the ETA exists only inside JD's localized sentence.

## Extraction ETA (measured live 2026-09-07, whole run observed)

- There is **no extraction progress/percentage anywhere**: a full `queryLinks`/`queryPackages` dump during a live extraction shows only `eta` moving; `bytesLoaded/bytesTotal` stay at the download figures and the queue payload is byte-identical across polls (keys: archiveId, archiveName, controllerId, controllerStatus, states, type).
- **The ETA lives on the links, not the package**: during extraction `package.eta` is absent while each archive link carries `eta` and `status` = the localized "extracting" word. jdtui takes the longest link eta for the package row (`ui::package_eta`).
- **Units differ by state**: download eta is in SECONDS, extraction eta is in MILLISECONDS. Verified twice — the figure fell ~1000-1350 per wall-clock second, and 425811 ms seen at ~10:16:30 predicted the end at 10:23:36 against an actual 10:23:41. `ui::eta_seconds(raw, extracting)` divides by 1000 only while extracting.
- Full lifecycle confirmed on a real 67 GB / 33-volume RAR: queue entry with `controllerStatus: RUNNING` → jdtui "Extracting" (JD meanwhile said only "Completato", i.e. the download); after the run the links flip to `SUCCESSFUL` → jdtui "Extracted".

## Containers and crawljobs (verified 2026-09-07)

- `linkgrabberv2/addContainer(type, content)`: `content` MUST be a base64 **data URL** — JD does `getInputStreamFromBase64DataURL`, which looks for `;base64,` and decodes what follows (source: `LinkCollectorAPIImplV2.loadContainer`). `type` is used as the file extension of the temp file, so pass the container's own extension (`dlc`, `ccf`, `rsdf`). jdtui sends `data:application/octet-stream;base64,<b64>` (`JdApi::add_container`).
- `.crawljob` keys (Folder Watch `explain.txt`): text, packageName, downloadFolder, extractPasswords, downloadPassword, priority, autoStart, forcedStart, enabled, chunks, comment, filename, autoConfirm, deepAnalyseEnabled, addOfflineLink, overwritePackagizerEnabled, extractAfterDownload. Entries separated by a line containing `->NEW ENTRY<-`, `#` comments, and a JSON array form is also accepted. jdtui maps the subset the API can express (`watch::parse_crawljob`).
- The FolderWatch extension itself is a JD extension (`InstallExtensionFOLDERWATCH` in `config/list`), watching folders on the JD machine only — not installed on the user's jd2@docker.

## Direct connection (2026-09-07)

- `JdApi::ensure_direct` must NOT arm `next_probe` on a successful probe, only on a fruitless one — doing both meant the first drop of a working route stuck on the relay for the whole retry window. Waits: `PROBE_FIRST_RETRY` 15 s doubling to `PROBE_RETRY` 300 s, reset on success. Covered by `api::live_direct` (both tests are `#[ignore]`, one needs the real device).
- Header marks: `⇄ direct` / `⇢ relay`, both from the Arrows block. A cloud (U+2601) is drawn double-width by many fonts and gets cut in half.

## Link and package status (2026-09-07)

- `status` on a link/package is JD's own sentence, in JD's language ("Caricamento mirror filestore.me", "Completato (mirror)", "Download (filestore.me)") and, on failure, a multi-line Java stack trace. NEVER put it in a table column: `model::package_status` / `model::link_status` derive the state from the booleans, and JD's first line is used only when nothing can be derived (an idle row that is idle because it failed).
- `statusIconKey` does NOT identify an error: a failed link reports `kc.<md5>` (the hoster's icon), exactly like a healthy one. Values seen: `true` (finished), `true-orange` (finished as the mirror twin), `kc.<md5>` (hoster icon, any state), absent.
- Mirrors: JD keeps one link per hoster for the same file. The twin that is not carrying the download is enabled, not running, not finished, 0 bytes loaded, and its sentence says "loading mirror" — i.e. it is simply waiting.
- `queryLinks` accepts `skipped`, which is a real state and is now requested.
