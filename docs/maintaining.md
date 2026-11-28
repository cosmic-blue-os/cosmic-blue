# Maintaining Celenn OS

The steps that keep Celenn OS going: what runs on its own, what needs a
person, and what to do when something breaks. For changes and translations,
see [CONTRIBUTING.md](../CONTRIBUTING.md).

## What runs on its own

| When (UTC) | Workflow | What it does |
| --- | --- | --- |
| Every day, 05:11 | `check-cosmic-version.yml` | Compares Fedora and COSMIC with the image, opens an update pull request when one moved |
| Every day, 06:17 | `build.yml` | Builds both images with Fedora's updates and publishes them to `testing` (skipped if `testing` was built in the last hours) |
| Each merge to `main` | `build.yml` | Builds, boot-tests and publishes to `dev` |
| Mondays, 04:41 | `check-app-sources.yml` | Checks that the apps the image installs on request still exist on Flathub and Homebrew |
| Tuesdays, 12:23 | `promote-stable.yml` | Moves `stable` to the daily build from 7 days earlier, after an update test |
| After each promotion | `release-notes.yml`, `build-live-iso.yml`, `pages.yml` | Release notes and README versions, live ISOs, website and its screenshots |
| 1st of the month | `clean-images.yml` | Lists the image versions older than 90 days (deletes only when run by hand with "delete") |
| Weekly | Renovate | Pull requests for base images, Bluefin parts and Actions; non-major ones merge themselves when green |

A failed scheduled run opens a **"CI failure: …" issue**. That issue is the
signal to look: nothing else needs watching.

## The weekly stable promotion

`stable` moves on its own on Tuesdays. Before it moves, an update test
installs the current `stable` in a virtual machine, updates it to the new
image, reboots, checks the desktop and rolls back. If that test fails,
`stable` stays where it is.

- **Before a promotion**, merge or hold the pull requests in progress: the
  website and README screenshots are taken from the new `stable`, so they
  show whatever it contains.
- **To promote sooner** (a fix users need now): run `build.yml` by hand on
  `main` (channel `testing`), wait for it, then run `promote-stable.yml` by
  hand with `source_tag` set to `testing`.
- **To roll `stable` back** after a bad week: run `promote-stable.yml` by hand
  with `source_tag` set to a known good date tag (for example `20261117`),
  with "skip update test" ticked if it is urgent. Then run
  `release-notes.yml` by hand with no input to refresh the README versions.
- **To freeze `stable`**: disable `promote-stable.yml` in the Actions tab
  until the problem is fixed.

Tags are copied by digest, so the signature made by the build stays valid.

## Live ISOs

`build-live-iso.yml` builds one ISO per image from `stable`, installs each in
a virtual machine, and uploads it to Cloudflare R2 only if the installed
system starts.

- Each ISO should stay under **5 GB**: the R2 free tier is 10 GB for both.
  Over that, the run warns and opens an issue. Find what grew before
  publishing anything else.
- The bucket is the repository variable `R2_BUCKET`, and its public URL,
  which the website's download buttons use, is `ISO_BASE_URL`. To move the
  ISOs to another bucket: give the R2 keys access to it, change both
  variables, run this workflow by hand, then run the website workflow. Once
  the website links to the new bucket, delete the old one within a few days:
  the 10 GB free tier counts every bucket of the account.
- The ISO tool (titanoboa) is pinned to an older commit on purpose, and its
  Renovate rule is paused. Its newer versions work differently. Move only
  when Bluefin's own ISO build moves, and turn the Renovate rule back on then.

## COSMIC updates

The COSMIC version is pinned in `build_files/cosmic-version`. When Fedora
ships a new one, `check-cosmic-version.yml` opens a pull request (label
`cosmic-update`), builds it and merges it if it is green.

When that pull request fails, the cause is usually one of Celenn OS's
changes to COSMIC no longer fitting the new code:

- **Login screen** (`build_files/greeter/user-switch.patch`, applied with no
  fuzz). Regenerate it against the new release:
  1. Clone `pop-os/cosmic-greeter` and check out the new release tag (for
     example `epoch-1.12.0`).
  2. Apply the old patch with fuzz (`patch -p1 --fuzz=3 <
     user-switch.patch`), fix the rejected parts by hand, and check that it
     builds (`cargo check`).
  3. Write the new patch with `git diff > user-switch.patch`. It must still
     contain the marker comment `Celenn OS: with 2 to 8 accounts`.
- **COSMIC Store and Files** (`build_files/store/patch.py`,
  `build_files/filemanager/patch.py`). Each replacement must match exactly
  once. The build log names the one that no longer matches: update its
  "before" text to the new code.
- **Compositor** (`build_files/compositor/build.sh`): one line, the
  background color shown before the wallpaper (`CLEAR_COLOR`). If it no
  longer matches, update its "before" text to the new code.
- **Our apps** (`build_files/first_steps`) follow the libcosmic revision of
  the new COSMIC. Fix what `cargo clippy --all-targets -- -D warnings` and
  `cargo test` report.

New defaults reach existing accounts by themselves: the default migration
(`celenn-migrate-defaults`) runs at each boot.

## New Fedora release

`check-cosmic-version.yml` follows the Fedora release of Bluefin's `stable`
tag. Its pull request for a new Fedora release is **merged by hand**: the
pull request build only boot-tests the image, not the ISO install. Before
merging, run `build-live-iso.yml` by hand on the branch (or right after the
merge) and check the install test. A Fedora release that ships an older
COSMIC than the pinned one is never proposed.

## When CI fails

Open the failed run from the issue or the pull request, then the first red
step. The usual causes:

| Failure | Cause | What to do |
| --- | --- | --- |
| "Update stable to testing": the staged update is not the expected digest | A build published a new `testing` during the test | Re-run the failed job once |
| Image build stops at `just helpers-keys` | Fedora ships a new COSMIC before the update pull request | Wait for the daily `check-cosmic-version.yml` run, or run it by hand |
| `--check`: "Not in po/…" | An app text missing from its template or from a language | Add it to the `.pot` and every `.po` (see CONTRIBUTING.md) |
| Lint: clippy or rustfmt | A warning or unformatted Rust code | `cargo fmt`, then fix what `cargo clippy --all-targets -- -D warnings` shows |
| NVIDIA ISO install test, btrfs errors | A known intermittent failure of the test VM | Re-run once. A second failure is a real one |
| "Check app sources" | An app left Flathub or Homebrew, or changed its id | Replace or remove it in the first-boot list or the recipe |

Never skip or disable a test to get green. If the same failure happens again
after a re-run, it is a real one.

When a pull request conflicts with `main`, its CI does not run: merge `main`
into it first. In `.po` files, a conflict often ends on a `msgid` line whose
`msgstr` is outside the conflict markers. When keeping both sides, check that
every `msgid` still has its `msgstr` and that no text appears twice.

## Signing and secrets

- Images are signed with cosign at build time. The public key is
  `cosign.pub` in this repository, and installed systems check it before any
  update (`policy.json`, scoped to `ghcr.io/celenn-os`).
- The private key (`SIGNING_SECRET`) and the R2 keys (`R2_ACCESS_KEY_ID`,
  `R2_SECRET_ACCESS_KEY`, `R2_ACCOUNT_ID`) are only in the GitHub environment
  `release`, which only `main` can use. Pull requests never see them.
- Losing or changing the signing key breaks updates for every installed
  system: a new key would need a release that trusts both keys first.
- The NVIDIA image's kernel modules are signed with Universal Blue's akmods
  key. Users enroll it once (`ujust enroll-secure-boot-key`, password
  `celennos`).

## Old images

`clean-images.yml` only lists what it would delete. Run it by hand with
"delete" ticked to delete versions older than 90 days. It always keeps every
`stable-YYYYMMDD` image, so users can roll back to any past stable.
