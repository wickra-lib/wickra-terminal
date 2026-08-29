# Governance

`wickra-terminal` is part of the Wickra project and follows the same lightweight
governance model.

## Roles

- **Maintainers** review and merge changes, cut releases and set direction. The
  current maintainers are listed in [MAINTAINERS.md](MAINTAINERS.md).
- **Contributors** propose changes via pull requests. Anyone may contribute; see
  [CONTRIBUTING.md](CONTRIBUTING.md).

## Decision making

Day-to-day changes are merged by a maintainer once CI is green and the change
has been reviewed. Larger or breaking changes (config format, view-model schema,
public API) are discussed in an issue first and decided by maintainer consensus;
the lead maintainer breaks ties.

## Releases

Releases follow semantic versioning. Pre-1.0, the config and view-model schemas
may change between minor versions. A release is tagged `vX.Y.Z` by a maintainer
and published to the language registries by CI.

## Contribution flow

Anyone may open an issue or a pull request. A change is reviewed by a
maintainer, has to pass the full CI matrix, and is squash-merged once both hold.
Substantial changes are better raised as an issue or a discussion first, so the
shape can be agreed before the work is done rather than after.

What a change is held to is in [`CONTRIBUTING.md`](CONTRIBUTING.md); it is the
same bar whoever wrote it.

## Becoming a maintainer

Maintainers are added by the existing maintainers. There is no fixed count of
merged pull requests: what is looked for is sustained, good-quality
contribution and the judgement to say no to a change that does not belong. That
judgement is easiest to see in review comments and in issues, not only in code.

An invitation is made privately first. A maintainer who becomes inactive may be
moved to emeritus by agreement, and can return the same way.

## Continuity and succession

The project is meant to survive the loss of any single individual, so that
issues can be triaged, changes accepted, and releases published within about a
week of a confirmed loss:

- **Credentials.** Everything needed to operate the project — the `wickra-lib`
  GitHub organisation, the publishing tokens for crates.io, PyPI, npm, NuGet and
  Maven Central, and the `wickra.org` domain registrar — is held in a password
  manager. A trusted contact holds **emergency access** to it and can obtain
  those credentials if the maintainer can no longer continue.
- **Continuity actions.** With that access the trusted contact, or someone they
  appoint, can open and close issues, accept pull requests, and publish releases
  through the CI workflows that already exist. Nothing about a release depends
  on a machine only the maintainer has: a release is a signed tag, and the
  workflow does the rest.
- **Account recovery.** The maintainer's GitHub account has recovery configured,
  and ownership of the `wickra-lib` organisation can be transferred.
- **Legal rights.** Rights to the project name and the DNS are covered by the
  maintainer's estate arrangements.

## Code of conduct

Everyone taking part is expected to follow the
[Code of Conduct](CODE_OF_CONDUCT.md). Maintainers are responsible for applying
it, and are held to it themselves.

## Changes to governance

This document is changed by a pull request approved by the maintainers.
