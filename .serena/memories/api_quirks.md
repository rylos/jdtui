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
- Live tests create packages named `jdtui-*` in the grabber; if a test aborts mid-way, leftovers must be removed by hand (`cargo test -- --ignored` again, or the GUI).
