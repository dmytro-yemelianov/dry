# Design: IP registration and preservation

**Date:** 2026-08-25
**Status:** Proposed
**Branch:** _(unassigned)_
**Source:** owner decisions taken 2026-08-25 (ownership, venues, visibility, no-rewrite); the gap between
`docs/CLEANROOM.md` + `docs/17-provenance-and-licensing.md` (technical provenance, which exists and is good)
and legal IP registration (which does not exist anywhere in the repo — `grep -riE
'trademark|patent|copyright registration|trade secret|escrow|assignment' docs/` returns nothing).
`docs/08-production-transition.md:176` records the only prior legal item: *"Remaining: a formal license
review by counsel before external commercial distribution."*

> **Engineering record, not legal advice.** Every fee, form, deadline and statutory reference below must be
> confirmed with counsel and against the current official tariff before acting. The purpose of this
> document is to make a counsel engagement short, cheap and well-briefed — not to replace one. Items
> marked **[counsel]** are decisions this document deliberately does not make.

## 0. Decisions at a glance

| Question | Decision | Rationale |
|---|---|---|
| Who owns it | Dmytro Yemelianov, individually | Sole author of all 460 commits. Entity formation deferred until a customer contract or investor forces it (§10.1). |
| Registration venues | US + Ukraine; EU for trademark only | US copyright registration is the only one that unlocks statutory damages and standing to sue in US courts. The EU has no copyright registry — copyright there is automatic. |
| Source visibility after cutover | **Private repo, signed binaries released** | Trade secret and EU patent novelty exist only for material that is not published. Publishing source forfeits both permanently, as it already has (§2). |
| Rewrite the codebase clean under a new name? | **No** | §1.3. Months of work; buys almost nothing. The owner already holds copyright in every line. |
| Patents | Option preserved, nothing filed now | §6.4. Published material: EU novelty gone, US grace to ~2027-06-18. The unbuilt ~60% is the real patentable surface, and only if it stays confidential. |
| Name | **`KMET` / `КМЕТ`** | Ukrainian *кмет* — a seasoned warrior, a master of the craft. Reads identically in both scripts; `crates.io`, `npm` and `PyPI` all free. Arbitrary/fanciful in English, so a far stronger mark than `DRY` (§5.1, §6.3). |
| Portfolio structure | **Six layers; three registrable works** | Split by commercial boundary, not directory — 17 U.S.C. §504(c) counts a compilation as one work for statutory damages (§1.1, §6.1). Requires the §5.7 crate split first. |
| Enforcement of the discipline | Ledger + validator + CI, mirroring `proofs/claims.toml` | §7. A prose-only checklist is the one process in this repo that would rot silently. |

## 1. What is being protected, and what a rewrite would and would not fix

### 1.1 The portfolio — six layers

**The organising principle: divide along commercial product boundaries, not along directories.** The
reason is statutory. Under 17 U.S.C. §504(c) all parts of a single compilation count as **one work** for
statutory damages — register the whole engine as one work and an infringer who copies all of it faces one
award, however many modules that was. Independently marketable components registered as separate works
each carry their own award. Courts look to whether a part has **independent economic value**, which is a
question about how it is sold, not about where its files sit. The layering below is therefore a commercial
decomposition that the code should then be made to match (§5.7).

`docs/00-vision-and-scope.md` states the architecture this follows: *"the product is the IR + engine;
authoring languages and target machines are interchangeable front-ends and back-ends hanging off it."*

| Layer | Contents | Size | Protection mode |
|---|---|---|---|
| **0 · Contract** | IR spec (`docs/10`), `spec/` schemas (54 files), conformance vectors (95 files) | 664 KB data | **Public, permanently.** Copyright is thin. The value is the trademark and control of the standard (§6.3) |
| **1 · Kernel** | `resolve` `ir` `features` `emit/` `gcode/` `codec/` `profile/` `units` `frame` `clothoid` `optimize/` `generate/` | ~17.5k LOC | **Trade secret + copyright.** Where future patents concentrate (§6.4) |
| **2 · Assurance** | `formal/` (38 Lean modules, 8 863 LOC), `proofs/` (39 files), `verify.rs` (2 143 — the largest file in core), `report.rs`, assurance tooling | ~11.5k LOC | Separate work, **separate SKU** |
| **3 · Analysis** | `trace.rs` (1 721), `forensics.rs` (834), `compare` `explain` `recommend` `reverse` | ~3.7k LOC | Separate work, separate SKU |
| **4 · Distribution** | CLI (8 061), wasm, `py/`, `sdk/ts`, cloud, verify-runner, `web/`, `services/`, `llm`, `moonraker` | ~18.6k LOC | **Most liberal.** Thin by design — little to protect, much adoption to gain |
| **5 · Commercial infra** | `tools/license-issuer` (704 KB, the largest tool), `crates/license`, `prod-1` key material | — | **Secrecy only.** Never registered, never licensed, never disclosed |
| **X · Encumbered** | `conformance/oracle/` (GPLv3), oracle-derived corpora, `conformance/slicer-corpus/` (3.1 MB third-party output) | — | **Quarantine.** Excluded from every filing and every release |

Two further items sit outside the layering: `docs/marketing/*` (market research, attack maps) is
commercially sensitive, currently public, and classified `secret` going forward under §5.4; and
`spec/examples/profiles/*` is factual numeric data from published spec sheets, protectable as a
compilation at best.

**Why layer 0 stays public, and why that is the moat rather than a leak.** LLVM's power is not its
licence; it is that everyone targets LLVM IR. If the IR is the standard and KMET is the reference
implementation, what is controlled is not the code but **the right to claim conformance** — enforced by a
certification mark over a published conformance suite (§6.3). Published vectors without that mark are
simply a giveaway; with it they are a licensable asset.

**Why layer 5 is never registered.** A US copyright deposit is a partial disclosure even under
trade-secret relief (§6.1). For the licence issuer and key material, any disclosure is worse than the
absence of a registration, and secrecy is the entire protection.

### 1.2 What is already permanently lost, regardless of any future action

These are settled facts, not risks to be managed:

- Every idea disclosed in the public repo and `docs/site/` since `2026-06-18T18:38:07Z` is in the state
  of the art. It is unpatentable in the EU (absolute novelty) and unprotectable as a trade secret
  anywhere.
- `v0.3.0` is irrevocably licensed under Apache-2.0 to everyone who obtained it (§2). That includes the
  express patent grant of Apache-2.0 §3.
- Deleting the GitHub repository does not retract either. Forks survive repository deletion, third-party
  archives are independent, and Apache-2.0 §2 is textually irrevocable. The current `LICENSE` already
  concedes this: *"Versions or copies previously distributed under another license remain governed by the
  license terms attached to those versions or copies."*

### 1.3 Why the codebase is **not** rewritten from scratch

The owner asked whether a clean rewrite under a new name solves the problem. It does not, and the
reasoning belongs in the record because the instinct will recur:

- **Copyright:** unnecessary. The owner is the sole author of every line. Copying his own tree into a new
  private repository under a new name requires no one's permission. A rewrite produces a second work he
  also owns, at the cost of months.
- **Patent novelty:** ineffective. A patent claims the *invention*, not the source text. Re-expressing a
  published idea in new code leaves the idea published. Novelty is not restored by paraphrase.
- **Apache-2.0 exposure:** ineffective. The grant attaches to the copies already distributed. Future code
  is unaffected either way — it was never under Apache-2.0 to begin with.
- **Trade secret:** ineffective for past material, and automatic for future material. What creates the
  secret is ceasing to publish (§5), not retyping.
- **Actively harmful in one respect:** a fresh repository with no history discards 460 dated commits and
  133 reviewed pull requests — the primary evidence of independent authorship (§3, §4).

The one genuine benefit of a rewrite would be the ability to state that the codebase shares no lineage
with anything ever published. That is worth having only if a lineage claim is ever actually made, and it
is purchasable later, more cheaply, by retiring the oracle (§5.5) — which severs the only *external*
dependency that matters.

**The corollary that does matter:** roughly 60% of the technology is unbuilt. That 60%, if developed after
the cutover and kept confidential, is protectable as a trade secret and patentable in the EU as well as
the US. The past 40% is not recoverable by any means. The entire protective value of this plan therefore
sits in the future work, and is unlocked by closing the repository — not by rewriting it.

## 2. Boundary facts — the immutable record

These are established by `git` and the GitHub API as of 2026-08-25, and are the factual base every later
argument rests on. §7.1 pins them in a machine-checked ledger so they cannot silently drift.

| Fact | Value |
|---|---|
| Repository | `github.com/dmytro-yemelianov/dry`, created `2026-06-18T18:38:07Z`, **public** |
| Sole author | `Dmytro Yemelianov <dmytroyemelianov@icloud.com>` — 460/460 commits |
| Apache-2.0 era begins | `14685a1` — 2026-06-18, *"re-licence to Apache-2.0 + clean-room framing"* |
| Apache-2.0 era ends | `4701c11` — 2026-07-25 07:13:40 +0300; tree `51259365a04ddf48d902b6ad02f34ee4f62625b8` |
| Relicense to proprietary | `a40d151` — 2026-07-25; clarified `b924c8f` — 2026-07-26 |
| Released under Apache-2.0 | **`v0.3.0`** — 2026-06-29 13:02:20 +0300 (verified: `git show v0.3.0:LICENSE`) |
| Released proprietary | `v0.4.0` (2026-07-28), `v0.5.0` (2026-08-01), `v0.6.0` (2026-08-03) |
| Release signing | `prod-1` Ed25519; ceremony 2026-08-03 (`ffa08fd`); signing key offline, never in repo |
| Supply-chain provenance | CycloneDX SBOM + `actions/attest-build-provenance` (Rekor) + `SHA256SUMS`, per `release.yml` |

**The consequence of the `v0.3.0` row is the single most important commercial fact in this document.** A
tagged, downloadable release exists under a perpetual, irrevocable, worldwide, royalty-free licence that
permits use, modification, redistribution, commercialisation, and derivative works — including a competing
renamed fork. Apache-2.0 §3 additionally granted its recipients a patent licence to claims necessarily
infringed by that version, so a US provisional filed before the 2027 deadline would not be enforceable
against them as to that code.

Apache-2.0 §6 grants **no trademark rights**. The rename and trademark strategy (§6.3) is entirely
unaffected. This is the reason the rename is the strongest single move available.

## 3. Two liabilities in the current record

### 3.1 `docs/CLEANROOM.md` overstates the clean-room claim

The provenance log asserts: *"the spec predates the code, the code references the spec (not FullControl
source), and the oracle is quarantined to dev/CI."* The first clause is not supported by the history:

| Artifact | First commit |
|---|---|
| `docs/00-04` + `crates/core` scaffold (`3281c94`) | 2026-06-18 21:41:25 +0300 — **same commit** |
| `conformance/oracle/` (FullControl, GPLv3) (`80a41a6`) | 2026-06-18 22:53:10 +0300 |
| `crates/core/src/resolve.rs` (`4b6b906`) | 2026-06-18 23:57:07 +0300 — **after** the oracle |
| `docs/10-dry-ir-v0-spec.md` (`c6b4c3e`) | 2026-06-29 10:15:46 +0300 — **11 days after** the resolver |

This is the first thing an adversary verifies, and a demonstrably false provenance claim discredits the
entire ledger — including the parts that are true and valuable. Overstatement is a net liability.

**The accurate claim is nearly as strong and is defensible.** At `3281c94` the architecture, roadmap and
conformance plan totalled 508 lines of specification against a 73-line `lib.rs` scaffold: the work was
demonstrably design-led, with the design authored contemporaneously and in far greater depth than the
code. The IR specification was written before the IR stabilised. The oracle's presence one hour before
`resolve.rs` is consistent with its documented role — it is a behavioural oracle and must exist before
there is anything to test against.

**Action (§8, Phase 2):** rewrite the `CLEANROOM.md` provenance-log paragraph to the accurate claim,
citing these four commits. Record the correction in `CHANGELOG.md`. Do not quietly edit — an amendment
with its reasoning stated is itself evidence of good faith.

### 3.2 AI co-authorship is extensive, documented, and must be disclosed

**250 of 460 commits (54%)** carry a `Co-Authored-By: Claude` trailer.

US Copyright Office registration guidance (88 FR 16190, 2023-03-16, reaffirmed by the Office's 2025
report on copyrightability) requires an applicant to disclose more-than-de-minimis AI-generated material
and to disclaim it in the Limitation of Claim. A registration that conceals it is vulnerable to
cancellation, and the concealment is not arguable here — the repository's own history states it on every
other commit. **[counsel]** confirms the exact disclaimer wording.

This cuts both ways, and the favourable side is unusually strong. Human authorship in a
human-directed AI workflow is found in selection, coordination, arrangement, specification, direction and
review. The repository holds an exceptional record of exactly that:

- **31 design specifications** in `docs/superpowers/specs/` — human-authored designs preceding implementation
- **14 implementation plans** in `docs/superpowers/plans/`
- **2 architecture decision records** (`docs/adr/0001` formal-assurance constitution, `0002` numeric ingress gates)
- **133 pull requests** and **96 issues** with their review history
- `proofs/claims.toml` and `formal/` — human-specified correctness claims the code is measured against

That corpus is load-bearing for the registration and is preserved as evidence in its own right (§4.4),
not merely as project documentation.

## 4. Phase 1 — Evidence capture, before anything is unpublished

**Gate: nothing in Phase 2 begins until Phase 1 is complete and verified.** Unpublishing is the one
irreversible step in this plan, and the evidence is cheaply capturable only while the artifacts are live.

Working directory: `ip/evidence/2026-08-<dd>-pre-cutover/`. Payloads are stored offline (§4.7); only
manifests and hashes are committed.

### 4.1 Complete history bundle

```sh
git bundle create dry-complete-<date>.bundle --all
git bundle verify dry-complete-<date>.bundle
sha256sum dry-complete-<date>.bundle
```

A single verifiable file containing every commit, branch and tag. This is the authorship record.

### 4.2 Per-tag source archives

`git archive` for `v0.3.0`, `v0.4.0`, `v0.5.0`, `v0.6.0` and the boundary commit `4701c11`, each with its
`SHA256` and its tree hash recorded. This makes the Apache/proprietary boundary provable per artifact
rather than merely asserted in prose.

### 4.3 Release assets and their attestations

`gh release download` for all four releases, plus the existing `SBOM.cdx.json`, `SHA256SUMS` and
provenance attestations. The Rekor transparency-log entries survive independently and permanently, but
the assets those entries attest to are hosted by GitHub and disappear with the repository. An attestation
whose subject is unavailable is much weaker evidence.

### 4.4 GitHub metadata export — the largest silent loss

```sh
gh api --paginate repos/dmytro-yemelianov/dry/pulls?state=all
gh api --paginate repos/dmytro-yemelianov/dry/issues?state=all
# plus review comments and timelines per PR
```

**133 pull requests** and **96 issues** — with their review history, discussion and timelines — exist only in GitHub's database
— none of it is in `git`. This is the narrative record of independent, incremental development, and it is
the precise material that rebuts an allegation of copying. It is also the material most likely to be
forgotten, because unpublishing does not appear to destroy anything.

### 4.5 Trusted timestamping

RFC 3161 timestamps over the manifest hashes from at least two independent TSAs, plus OpenTimestamps as a
free Bitcoin-anchored redundant anchor.

This is the mechanism that proves a date **without publishing the content** — which is exactly the
requirement for material that is about to become a trade secret. Timestamp the manifest, never the payload.

### 4.6 Software Heritage — split decision, deliberately

Software Heritage systematically archives public repositories and issues permanent, third-party-attested
content identifiers (SWHIDs). That is the strongest independent authorship evidence obtainable, and it is
obtainable only while the repository is public.

It is also **permanently public**. Triggering archival of the proprietary-era tree would permanently
publish the material this plan exists to protect.

- **Do:** query whether Software Heritage has already archived the repository. If it has, cite the SWHIDs
  at zero marginal cost. Either answer is needed: a positive result means that code is permanently public
  irrespective of what is deleted, which is itself a fact the ledger must record.
- **Do not:** trigger new archival. Use §4.5 for the proprietary era.

The same query applies to the Wayback Machine for `docs/site/`.

### 4.7 Manifest, signature, storage

A single `MANIFEST.json` listing every captured artifact with its SHA-256, signed with the offline
`prod-1` key — tying the evidence chain to the already-documented key ceremony — and RFC-3161 timestamped.

Storage 3-2-1: encrypted offline copy; second copy in a different geography; one escrow-grade location.
**[counsel]** on whether a notarial deposit under Ukrainian practice adds evidentiary weight over an
RFC-3161 timestamp; the timestamp is likely sufficient and far cheaper.

## 5. Phase 2 — Cutover

### 5.1 The name — `KMET` / `КМЕТ`

**Chosen 2026-08-25.** Ukrainian **кмет**: in the *Tale of Igor's Campaign*, **къмети** are seasoned,
battle-hardened warriors — masters of a craft — rather than the later dictionary sense of "peasant". It
also resonates with *кмітливий* (quick-witted, resourceful).

The name satisfies a deliberate constraint: it reads identically in both scripts. Only seven letters have
matching form **and** sound across Cyrillic and Latin — **А Е І К М О Т** — because В≠B, Р≠P, С≠C, Н≠H,
У≠Y and Х≠X in sound despite identical glyphs. `КМЕТ` = `KMET`, glyph for glyph.

**Backronym** — the technical gloss, mapping one letter to each pipeline stage and each to real modules:

| | Stage | Modules |
|---|---|---|
| **K** | **K**inematics — machine model: profiles, limits, envelope geometry | `profile/`, `units.rs`, `frame.rs`, `clothoid.rs` |
| **M** | **M**otion — the multi-level IR | `ir.rs`, `resolve.rs`, `features.rs` |
| **E** | **E**mission — target output | `emit/`, `gcode/`, `codec/` |
| **T** | **T**race — verification, reports, analytics | `verify.rs`, `report.rs`, `trace.rs`, `forensics.rs` |

> **KMET — Kinematic Motion Emission Toolchain**
> **КМЕТ — Кінематика · Моделювання · Емісія · Трасування**

**Clearance status as of 2026-08-25:**

| Surface | Status |
|---|---|
| `crates.io`, `npm`, `PyPI` | **All three free** — verified by API |
| `github.com/kmet` | **Taken** — needs a different org (`kmet-dev`, `kmetlang`) |
| Domains | **Unknown.** DNS shows no A record for `.io`/`.sh`/`.ua`/`.com.ua`, but absence of an A record is not availability. Check at a registrar. |
| Trademark | **Not searched.** §6.3, and a **[counsel]** gate before filing. |

`dry` was never claimed on any registry, because publication was disabled by policy — which left all three
package names available to anyone. Do not repeat this: **claim all three registry names and the domain
before announcing** (§6.3). Registry names are the cheapest defensive asset in the plan and the easiest to
lose to a squatter between announcement and release.

### 5.2 Make the public repository private — do not delete

Deletion and privatisation both detach existing forks; neither removes them. Deletion therefore buys
nothing and forecloses options, while privatisation is reversible. **[counsel]** on whether continued
availability of `v0.3.0` is preferable for the record, given that Apache-2.0 §4 obligations attach to
distribution rather than to hosting.

### 5.3 Carry the full history into the private repository

No squash, no fresh `init`. The 460 dated commits and their trailers are the authorship evidence
(§3.2, §4.1). A tidy log is worth far less than a provable one.

### 5.4 Classification pass — the precondition for trade secret

Trade secret protection requires **reasonable measures to maintain secrecy** — Ukrainian Civil Code
Art. 505–508 and, in the US, the DTSA (18 U.S.C. §1836). A written classification, consistently applied,
is the canonical evidence that such measures were taken. The repository currently has none, so no trade
secret presently exists over anything.

Every top-level component is classified `public` / `nda` / `secret` as a `[[component]]` entry in `ip/ledger.toml` (§7.1 defines the enumerated set), and the
validator fails CI on any unclassified component. Initial proposal:

| Class | Components |
|---|---|
| `public` | `spec/`, `docs/10-dry-ir-v0-spec.md`, integration docs, conformance vectors, SDK signatures |
| `nda` | Report contracts, profile library, pilot material, benchmark results |
| `secret` | `crates/core` internals (resolve/emit/verify numerics), `tools/license-issuer`, `prod-1` key material, `docs/marketing/*`, unbuilt roadmap |

Note honestly: components already published cannot become secret. Their classification records the
*intended forward posture*, and the ledger marks them `previously-published` so the distinction is never
lost.

### 5.5 Scrub the docs site, and retire the oracle

- `docs/site/` currently publishes implementation detail that operates as continuing public disclosure.
  Reduce it to the integration contract; move the rest behind the NDA boundary.
- Retire `conformance/oracle/`. `CLEANROOM.md` already plans this (*"once Dry is self-consistent, its own
  outputs become the reference and the oracle is retired"*). Executing it severs the only external
  GPLv3 association, and does so far more effectively than rewriting the owner's own code (§1.3).

### 5.6 Rename mechanics

Crate names, package names, CLI binary name, the `LicenseRef-Dry-Proprietary` SPDX identifier, `LICENSE`,
`NOTICE`, docs, schema `$id` URIs, and the `docs/site` domain. Schema `$id` changes are a compatibility
break for any pilot consuming them — sequence with §8's pilot-notification step.

### 5.7 Split the kernel crate — a prerequisite for §6.1, not a nice-to-have

Layers 2 and 3 (§1.1) are currently *inside* `crates/core`. Registering them as separate works while they
ship as one crate is a weak position: independent economic value is the test, and "it is a module in the
same crate, built and licensed together" is the answer an opponent wants. Doing the split before filing
means the boundary is a fact about the product rather than an argument about it.

| New crate | Layer | Drawn from |
|---|---|---|
| `kmet-kernel` | 1 | `resolve` `ir` `features` `emit/` `gcode/` `codec/` `profile/` `units` `frame` `clothoid` `optimize/` `generate/` |
| `kmet-verify` | 2 | `verify.rs`, `report.rs`, and the `proofs/` + `formal/` linkage |
| `kmet-trace` | 3 | `trace.rs`, `forensics.rs`, `compare.rs`, `explain.rs`, `recommend.rs`, `reverse.rs` |

The split pays twice. Legally it makes three registrations defensible instead of one arguable. Structurally
it relieves the two largest files in the tree — `verify.rs` at 2 143 lines and `trace.rs` at 1 721 — which
are already the least pleasant places in the codebase to work, and it gives the SKU boundaries in §1.1
something real to attach to.

Sequence it **before** Phase 3, and after the rename (§5.6) so the crates are born with their final names.
This is ordinary refactoring with a deadline, not new engineering: no behaviour changes, and the existing
drift-gated conformance suite is exactly the instrument that proves it.

## 6. Phase 3 — Registrations

### 6.1 US Copyright Office — the highest-value filing

Registration is a precondition to suing in US courts (17 U.S.C. §411) and, when timely, unlocks statutory
damages and attorney's fees (§412). Foreign authors may register. This is the single best value in the plan.

- **Fee:** ~$45 for the Single Application (one work, one author, one claimant, not work-for-hire — which
  fits exactly), ~$65 standard. **Verify current tariff.**
- **Deposit, with trade-secret relief — the mechanism that makes a private codebase registrable.**
  37 CFR 202.20(c)(2)(vii) requires "identifying portions": ordinarily the first 25 and last 25 pages of
  source. Where the program contains trade secrets, the regulation offers alternatives, including redacted
  first/last 25 pages (redactions proportionately less than the visible material), first/last 10 pages
  unredacted, or object code plus 10+ consecutive unredacted source pages. The application must state that
  the work contains trade secrets. **[counsel]** selects the option.
- **AI disclaimer (§3.2):** disclaim AI-generated material in Limitation of Claim → Material Excluded;
  describe human authorship in New Material Included — human-authored source, plus the selection,
  coordination and arrangement of program code, and the human-authored specifications, architecture and
  correctness claims evidenced by `docs/superpowers/specs/`, `docs/adr/` and `proofs/`.
- **Published vs unpublished — a real subtlety. [counsel]** The source *was* published on GitHub for
  `v0.3.0`–`v0.6.0`. Post-cutover versions distributed as binaries to pilots under signed agreements are
  arguably a limited distribution rather than publication under 17 U.S.C. §101. The two cases are
  registered differently and the answer affects the deposit.
**What to register: three works, not one and not fifteen.** Per the §1.1 principle and the §5.7 split:

| # | Work | Layer | Why separately registrable |
|---|---|---|---|
| 1 | **KMET Engine** | 1 | The commercial core; the thing an infringer would take |
| 2 | **KMET Assurance** | 2 | Machine-checked proofs over a toolpath compiler — the strongest differentiator, and independently marketable to buyers who need evidence rather than features |
| 3 | **KMET Analysis** | 3 | Post-slicer review and forensics address a different buyer entirely (`docs/05` §2–§3) |

Deliberately excluded: **layer 4** — thin by design, low value, and three more filing fees buy nothing;
**layer 5** — never registered, per §1.1; **layer 0** — public contract, thin copyright, protected by
trademark instead; **layer X** — encumbered, excluded from every filing.

One registration would be cheaper by ~$90 and would collapse all three into a single statutory-damages
award. Fifteen would multiply fees and admin while making each "work" easier to attack as an arbitrary
slice of one program. Three matches how the product is actually sold.

- **Cadence:** register the post-`4701c11` proprietary baseline for each of the three after the rename and
  the §5.7 split; re-register at `v1.0` and at major versions, each covering the new material.

### 6.2 Ukraine — УКРНОІВІ

Under the Law of Ukraine "On Copyright and Related Rights" No. 2811-IX (in force 2023-01-01), computer
programs are protected as literary works. State registration is voluntary; the certificate
(*свідоцтво про реєстрацію авторського права на твір*) is a dated official document and strong prima facie
evidence of authorship.

- Modest state fee. **Verify the current tariff and the office's operating procedure**, which has changed
  with the transfer of functions to УКРНОІВІ and may be affected by wartime conditions.
- Deposit is typically a source fragment plus a description — apply the same trade-secret discipline as
  §6.1 and deposit only classified-`public` or deliberately-redacted material. **[counsel]**
- Useful beyond litigation: local contracts, and IP-income treatment if a Дія.City entity is later formed
  (§10.1).

### 6.3 Trademark on `KMET`

The rename converts the weakest element of the portfolio into one of the strongest. `DRY` is a famous
software-engineering principle (*Don't Repeat Yourself*), which makes a bare word mark in the software
classes hard to register and weak to enforce. `KMET` has none of that problem: to an English-speaking
examiner it is a four-letter coined string with no meaning in the field — **arbitrary or fanciful**, the
strongest class of mark, and the class that supports the broadest enforcement.

**Register the string, never the expansion.** The backronym in §5.1 is marketing copy. *Kinematic Motion
Emission Toolchain* is straightforwardly descriptive of a toolpath compiler, and pleading it as the mark —
or leaning on it in the specimen — invites a descriptiveness refusal and weakens what is otherwise a strong
filing. File `KMET`; let the expansion live in the copy.

**Known regional meanings, to disclose to counsel up front:** Bulgarian *кмет* = mayor; Serbian/Croatian
*kmet* = serf. Neither is descriptive of software, so neither should bar registration in Nice 9/42, but
both are the kind of fact an EUIPO examiner or an opponent surfaces, and it is cheaper to raise it than to
be surprised by it.

- **Classes:** Nice 9 (software) and 42 (SaaS, software design services). Consider 7 for
  machine-control aspects. **[counsel]**
- **Venues and approximate official fees — verify current:** USPTO ~$250/class (TEAS Plus) or ~$350
  (Standard); EUIPO ~€850 first class, +€50 second, +€150 each further; Ukraine nationally.
- **Madrid Protocol** is worth pricing: Ukraine is a member, so a Ukrainian base application can support
  an international registration designating the EU and the US, often below the cost of three separate
  national filings. **[counsel]**
- **US intent-to-use (§1(b))** permits filing before launch, with a Statement of Use later — the right
  instrument here, since the name will be filed before it is announced.
- **File before announcing.** Priority and squatting both turn on this.

**A second, different mark: the certification mark.** This is the instrument that makes layer 0's
permanent publicness an asset instead of a giveaway, and it is the least obvious recommendation in this
document.

A **certification mark** (US: 15 U.S.C. §1054; the EU certification mark under the EUTM Regulation) is a
distinct registration type whose whole purpose is to be *licensed to third parties* who meet a published
standard — and which the owner may not use on their own goods. `KMET VERIFIED` (or similar) over the
published conformance suite would let an independent implementation earn the right to claim conformance by
passing `conformance/vectors`, under terms you set. This is how USB-IF, the Bluetooth SIG and the Wi-Fi
Alliance monetise standards they give away.

The effect on the portfolio: **the IR spec and vectors stay public — that drives adoption — while the
right to say "KMET-compliant" stays owned.** Control moves from the code, which cannot be kept secret once
published, to the name, which can be held indefinitely. Without this, publishing the conformance suite is
straightforwardly a donation to any competitor who wants to claim compatibility.

- **Prerequisites:** the standard must be documented and applied consistently, and certification must be
  available on non-discriminatory terms to anyone who meets it. The conformance suite already exists and is
  drift-gated, which is most of the work.
- **Timing:** after the primary `KMET` word mark, not before — and only once there is a third party who
  might plausibly seek certification. Filing a certification mark with no ecosystem is premature. **[counsel]**

### 6.4 Patents — option preserved, nothing filed

No filing now. The decision is deliberate and the option is kept alive by process rather than by fee:

- **Already-published material:** EU novelty is gone (§1.2). US grace period under 35 U.S.C. §102(b)(1)
  runs to approximately **2027-06-18** measured from first publication. Diarise it; it is the only hard
  external deadline in this document.
- **The unbuilt ~60%:** fully novel everywhere, provided it stays confidential until any filing. This is
  the entire patentable surface and it is created, not preserved, by §5.

**Where to aim — the gap between the roadmap and the code shows it.** The strongest candidates are the
directions `docs/05-product-directions.md` describes at length and the tree has barely implemented, because
those are exactly the ideas whose novelty survives:

| Candidate | Evidence it is still novel | Why it is a plausible claim |
|---|---|---|
| **Inferring parametric design intent from machine code** | `reverse.rs` is **175 LOC** against a full section of roadmap (`docs/05` §3) | Reconstructing a parametric program from emitted toolpaths is a concrete technical process, not an abstract idea — the better posture under both §101 and EPO technical-effect practice |
| **Safety-gated optimisation** | `docs/05` §"Optimization safety gates"; `optimize/` is 1 305 LOC and the gates are roadmap | Optimisation bounded by machine-envelope and process constraints, refusing unsafe rewrites, controls a physical machine |
| **Verified emission** — proof-linked codegen | `proofs/` + `formal/` exist; the linkage to emission is partial | Machine-checked correctness properties carried through lowering into emitted machine code |

Note the asymmetry deliberately: `forensics.rs` is already 834 LOC and largely public, so the forensics
surface is mostly spent. `reverse.rs` at 175 is not. **The disclosure gate below is what keeps it that way**
— publishing the reverse-engineering work before filing would repeat §1.2 exactly.

- **Invention disclosure record:** maintain `ip/disclosures/` — dated, signed, RFC-3161 timestamped
  records of each candidate invention as it is conceived. Cheap, and it establishes conception dates
  independently of any filing.
- **Disclosure gate:** before any public disclosure of new engine work (paper, demo, conference, docs,
  release notes), a check runs — is there anything here worth a provisional first? This is the control
  that prevents the §1.2 loss from recurring. Wired into §7.3.
- **Cost if exercised:** US provisional ~$65–325 official depending on entity status, plus attorney time;
  it holds priority for 12 months and permits "patent pending". The real cost arrives at the
  non-provisional (~$8–20k). **[counsel]** on subject-matter eligibility — a compiler that controls
  physical machine motion has a materially better technical-effect story under EPO practice than generic
  software, which matters if EU filing is ever pursued on the new work.

### 6.5 Trade secret regime

Not a registration — a standing posture, established by §5.4 and maintained by §7:

- Written classification, consistently applied and CI-enforced
- Access control on the private repository and on key material
- NDAs before any `nda`-class disclosure; confidentiality terms in every pilot agreement
- Marking of `secret`-class artifacts
- An access log for `secret`-class material

Together these constitute the "reasonable measures" both the Ukrainian and US regimes require.

## 7. Phase 4 — Ongoing preservation, enforced by tooling

Mirrors the existing `proofs/claims.toml` + `proofs/claims.schema.json` +
`tools/validate_proof_claims.py` + `tools/tests/` pattern exactly, so it is maintained by the same reflexes
as the rest of the repository.

### 7.1 `ip/ledger.toml` + schema + validator

```
ip/
  README.md              # the policy, and how to use the ledger
  ledger.toml            # boundary facts, registrations, classification, evidence-pack index
  ledger.schema.json     # JSON Schema, draft 2020-12, mirroring proofs/claims.schema.json
  disclosures/           # dated invention disclosure records (§6.4)
  evidence/              # manifests and hashes only — payloads live offline (§4.7)
tools/
  validate_ip_ledger.py
  tests/test_validate_ip_ledger.py
```

`ledger.toml` sections:

- `[boundary]` — the §2 table, hash-pinned. Immutable: the validator fails on any change absent an
  explicit, reviewed override field, because these facts do not change and a silent edit is the failure
  mode that matters.
- `[[registration]]` — jurisdiction, type (`copyright` / `trademark` / `patent`), status, application and
  registration numbers, filing and grant dates, covered version and commit range, deposit option used,
  AI-disclaimer text as filed.
- `[[component]]` — path, **`layer`** (`0`–`5`, or `X` for encumbered, per §1.1), class
  (`public` / `nda` / `secret`), `previously_published` flag, rationale. The layer field is what ties a
  directory to the portfolio, so the commercial decomposition is machine-checkable rather than a table in
  a document that drifts.
- `[[evidence_pack]]` — release tag, manifest hash, TSA tokens, storage locations.

**The classified set — defined once, here, because the drift gate depends on it being unambiguous.** A
*component* is exactly one of: each `crates/*`; each of the top-level directories `conformance/`, `docs/`,
`examples/`, `formal/`, `proofs/`, `py/`, `sdk/*`, `services/`, `spec/`, `third_party/`, `tools/`, `web/`;
each `containers/*`; and one synthetic `:root` entry covering root-level files. Directories excluded from
the enumeration (`.git/`, `target/`, `node_modules/`, `.github/`) are listed explicitly in the schema so
that exclusion is a reviewed decision rather than an oversight.

Validator checks:

1. Every component in the classified set has exactly one `[[component]]` entry, and every `[[component]]`
   path exists — **a bidirectional drift gate**: a new crate or top-level directory without a
   classification fails CI, and so does a stale entry for something deleted. This is the same discipline
   as an unregistered `RuleId` or an unlisted corpus.
2. `[boundary]` matches the pinned hashes.
3. Registrations are structurally complete and internally date-consistent.
4. Every release tag has an `[[evidence_pack]]`.
5. Nothing classified `secret` is reachable from a published path (docs site sources, public schemas,
   release artifact manifests) — the check that would have caught §1.2 before it became permanent.
6. Layer/class consistency: every layer-`0` component is class `public`, every layer-`5` component is
   class `secret`, and no layer-`X` path appears in any release-artifact manifest. These are the three
   invariants of §1.1 that silently reverse if nobody is watching.

CI job `ip-ledger`, alongside the existing validator jobs in `ci.yml`.

### 7.2 Per-release evidence pack in `release.yml`

Extends the existing SBOM/attestation step rather than duplicating it:

- `EVIDENCE.json` — tag, commit, tree hash, build timestamp, authorship summary (commit count, AI-trailer
  count, contributor set), classification-snapshot hash, dependency-licence summary
- `EVIDENCE.tsr` — RFC-3161 timestamp over its hash
- Both appended to `SHA256SUMS` and covered by the existing provenance attestation

Every release then carries a self-contained, independently dated authorship record, with no manual step
and therefore nothing to forget.

### 7.3 Cadence

| Trigger | Action |
|---|---|
| Every release | Evidence pack (automated, §7.2) |
| Every new top-level component | Classification entry, or CI fails (§7.1) |
| Before any public disclosure of new engine work | Disclosure gate (§6.4) |
| Quarterly | Ledger review; verify offline evidence copies are readable |
| Each minor version | Assess whether new material warrants copyright re-registration |
| 2027-03-18 (3 months before deadline) | Final US patent grace-period decision (§6.4) |
| On entity formation | Execute assignment; update `[[registration]]` claimants (§10.1) |

## 8. Sequencing

```
Phase 1  Evidence capture ...................... §4   ← blocks everything; repo still public
   ├─ history bundle, tag archives, release assets
   ├─ GitHub metadata export (133 PRs + 96 issues)          ← largest silent loss
   ├─ RFC-3161 + OpenTimestamps
   ├─ Software Heritage / Wayback query only    ← do not trigger archival
   └─ signed MANIFEST.json, 3-2-1 storage

GATE   Phase 1 verified: every hash reproducible from stored payloads

Phase 2  Cutover ............................... §5
   ├─ KMET trademark search + domain check      ← last clearance gate
   ├─ claim crates.io / npm / PyPI / domain / org ← BEFORE announcing
   ├─ private repo, full history preserved
   ├─ classification pass                        ← creates the trade secret
   ├─ CLEANROOM.md correction                    ← §3.1
   ├─ docs-site scrub; oracle retirement plan
   ├─ rename mechanics; notify pilots of schema $id change
   └─ split crates: kernel / verify / trace       ← §5.7, prerequisite for 3 works

GATE   Crate split merged and green — the §6.1 work boundaries are now
       facts about the product, not arguments about it

GATE   [counsel] engagement — brief is this document

Phase 3  Registrations ......................... §6
   ├─ US trademark ITU: KMET  ← file before announcing
   ├─ EUIPO / Madrid: KMET
   ├─ US copyright ×3 — Engine / Assurance / Analysis   ← §6.1, layers 1/2/3
   │    each with AI disclaimer + trade-secret deposit relief
   └─ УКРНОІВІ copyright ×3

   (later, once an ecosystem exists: KMET VERIFIED certification mark — §6.3)

Phase 4  Tooling ............................... §7   ← can start in parallel with Phase 2
   ├─ ip/ledger.toml + schema + validator + tests
   ├─ ci.yml: ip-ledger job
   └─ release.yml: evidence pack
```

Phase 4 is independent of the legal phases and can be built while name clearance runs.

## 9. Indicative cost

All figures require verification against current official tariffs; attorney time dominates and is not
estimated here.

| Item | Official fee (approx.) |
|---|---|
| US copyright, Single Application | $45–65 × **3 works** (§6.1) |
| УКРНОІВІ copyright certificate | modest state fee — verify |
| USPTO trademark, per class | $250–350 |
| EUIPO trademark | €850 first class, +€50 / +€150 |
| Madrid Protocol international registration | price against separate national filings |
| Domain + defensive registry names | tens of dollars |
| RFC-3161 timestamps, OpenTimestamps | free to negligible |
| US provisional patent, if exercised | $65–325 + attorney time |

The protective core — evidence capture, classification, closing the repository, the ledger and CI — costs
no fees at all. It costs sequencing discipline, which is why it is enforced by tooling rather than by
intention.

## 10. Risks and open questions

### 10.1 Chain of title — clean; assignment deferred to entity formation

Title is as unencumbered as a software project's realistically gets, and this is worth stating positively
because it is what makes §6 straightforward:

- **One author, one identity, throughout.** All 460 commits are authored
  `Dmytro Yemelianov <dmytroyemelianov@icloud.com>` — a personal identity, consistent from `b492cad`
  (2026-06-18) to `84e6ad7` (2026-08-03). No co-authors, no contractor contributions, no CLA backlog, no
  competing claimants to reconcile before filing.
- **The work is personal, not employment output.** Confirmed by the owner. No employer or third party has
  a claim on it, and no assignment is needed to file in his own name.
- **The AI trailers are not a title question.** They bear on what is *registrable* (§3.2), not on *who owns*
  what is registrable. Anthropic asserts no ownership of Claude's output. The 250 co-authored commits change
  the disclaimer wording in §6.1; they do not introduce a second claimant.

The one future action: **on entity formation**, existing registrations require a written assignment plus
recordation with each office (US Copyright Office recordation under 17 U.S.C. §205; the equivalent
УКРНОІВІ procedure; USPTO/EUIPO assignment recordal for any trademark). Cheap and routine, but it must
actually be done — registrations left in a personal name after the operating entity exists are a standard
diligence finding. Wired into the §7.3 cadence.

### 10.2 Open items

| Item | Status |
|---|---|
| `KMET` domain availability | Unknown — DNS is not proof; check at a registrar (§5.1) |
| `KMET` trademark search (USPTO / EUIPO / UA) | Not run — **[counsel]** gate before filing (§6.3) |
| GitHub org for `KMET` | `github.com/kmet` taken — pick an alternative (§5.1) |
| Software Heritage archival status | Unknown — must be queried in Phase 1 (§4.6) |
| УКРНОІВІ current procedure and wartime operation | Verify |
| Published vs unpublished for US registration | **[counsel]** (§6.1) |
| Deposit option under 37 CFR 202.20(c)(2)(vii) | **[counsel]** (§6.1) |
| Oracle retirement timing | Depends on engine self-consistency; already roadmapped |
| Pilot agreements' confidentiality terms | Review against §6.5 before any `nda` disclosure |

### 10.3 Risks

- **Phase 1 skipped or rushed.** The only irreversible failure in the plan. Mitigated by the explicit gate
  in §8 and by requiring every hash to be reproducible from stored payloads before Phase 2 begins.
- **The `v0.3.0` Apache fork materialises.** Not preventable. Mitigated by the fact that the unbuilt 60%,
  the trademark, and the trade-secret surface all sit outside what was granted.
- **Classification decays.** Mitigated by the §7.1 drift gate — the same mechanism that keeps the
  conformance corpora honest.
- **Trade secret asserted over previously published material.** Mitigated by the `previously_published`
  flag: the ledger records precisely what may never be claimed as secret.
- **Registrations left in a personal name after an operating entity exists.** The only chain-of-title risk
  in the plan, and it is a future one — title today is clean (§10.1). Mitigated by the entity-formation row
  in the §7.3 cadence and by the `[[registration]]` claimant field, which makes a stale claimant visible in
  the ledger rather than only in a diligence questionnaire.

## 11. Out of scope

- Drafting the pilot/commercial agreements themselves (separate work; §6.5 states the requirement only)
- Entity formation and Дія.City structuring
- Open-source compliance beyond what `docs/17` already covers
- Export control classification of the CAM/robotics surface — flagged as a separate question, not
  analysed here
- Insurance, escrow-for-customers (distinct from evidence escrow), and any litigation strategy

## 12. Verification

The legal phases are verified by artifacts — certificates, receipts, timestamp tokens — recorded in
`ip/ledger.toml`. The tooling is verified conventionally:

- `tools/tests/test_validate_ip_ledger.py`, mirroring the existing
  `test_validate_proof_claims.py` / `test_validate_spec_claim_links.py` structure: valid ledger passes;
  each of the six §7.1 checks fails on a targeted malformed fixture.
- An unclassified-component fixture proves the drift gate fires.
- A `secret`-reachable-from-published fixture proves check 5 fires.
- Phase 1 is verified by re-deriving every `MANIFEST.json` hash from stored payloads on a second machine,
  and by validating the `git bundle` independently of the working repository.
