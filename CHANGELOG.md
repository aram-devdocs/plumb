# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

From the first release onward, this file is maintained automatically by [`release-please`](https://github.com/googleapis/release-please) based on [Conventional Commits](https://www.conventionalcommits.org/) on `main`. Do not edit released sections by hand.

## [0.0.14](https://github.com/aram-devdocs/plumb/compare/v0.0.13...v0.0.14) (2026-06-21)


### Features

* **codegen:** infer workspace token modules ([#311](https://github.com/aram-devdocs/plumb/issues/311)) ([ab126c9](https://github.com/aram-devdocs/plumb/commit/ab126c975e860ae240c3ef84a3b9b875a5938de9))


### Bug Fixes

* **cdp,mcp:** stabilize page linting noise ([#309](https://github.com/aram-devdocs/plumb/issues/309)) ([627c4ec](https://github.com/aram-devdocs/plumb/commit/627c4ec2689a9195114903e93d200cd9695c99c9))
* **cdp:** keep default raw readiness best effort ([#310](https://github.com/aram-devdocs/plumb/issues/310)) ([8c952d5](https://github.com/aram-devdocs/plumb/commit/8c952d5de87734fa80ed686a08aa812bfe02ffa4))
* **codegen:** parse rem token lengths ([#313](https://github.com/aram-devdocs/plumb/issues/313)) ([8c2a3bf](https://github.com/aram-devdocs/plumb/commit/8c2a3bf02aa684e11784b6a187818bd5f2099935))
* **codegen:** prefer light token exports ([#312](https://github.com/aram-devdocs/plumb/issues/312)) ([8be27ec](https://github.com/aram-devdocs/plumb/commit/8be27ec8011ca4f46aeeab608ff589ab046321fe))


### Documentation

* ship the full brand kit and polish the book theme ([#307](https://github.com/aram-devdocs/plumb/issues/307)) ([59d1046](https://github.com/aram-devdocs/plumb/commit/59d1046674706915db696486d3c34143129c66fe))

## [0.0.13](https://github.com/aram-devdocs/plumb/compare/v0.0.12...v0.0.13) (2026-06-19)


### Features

* brand identity across the CLI, docs, and assets ([#305](https://github.com/aram-devdocs/plumb/issues/305)) ([2d28ec0](https://github.com/aram-devdocs/plumb/commit/2d28ec027f5a4da731f62e7e197d57d3a9a2ed49))


### Bug Fixes

* **ci:** unstick Preflight + tolerate Dogfood warnings/nextjs flake ([#292](https://github.com/aram-devdocs/plumb/issues/292)) ([6bba313](https://github.com/aram-devdocs/plumb/commit/6bba313d174ad4ff7300486e56c07c973d75db8f))
* **cli:** triage flags, exit codes, and grid honoring the spacing scale ([#304](https://github.com/aram-devdocs/plumb/issues/304)) ([5e9d34b](https://github.com/aram-devdocs/plumb/commit/5e9d34b6f7bb56115345aaf8c4427c2103f9a678))
* **mcp:** cap and aggregate tool output; load project config ([#303](https://github.com/aram-devdocs/plumb/issues/303)) ([dd1ab99](https://github.com/aram-devdocs/plumb/commit/dd1ab99a04331548f8595cb6139981aab7f7f933))
* **release:** post-process homebrew formula + include .zip in upload glob ([#288](https://github.com/aram-devdocs/plumb/issues/288)) ([9e0b476](https://github.com/aram-devdocs/plumb/commit/9e0b476a0cc641f9dccbd12b2adfd84e9e63e72a))
* **rules:** precision pass to cut real-world false positives ([#302](https://github.com/aram-devdocs/plumb/issues/302)) ([3b42259](https://github.com/aram-devdocs/plumb/commit/3b422598f3f8e95f55a985c1eec17053f598e800))

## [0.0.12](https://github.com/aram-devdocs/plumb/compare/v0.0.11...v0.0.12) (2026-05-08)


### Features

* **core:** apply [rules.&lt;id&gt;].severity overrides at engine layer ([#278](https://github.com/aram-devdocs/plumb/issues/278)) ([b15ba41](https://github.com/aram-devdocs/plumb/commit/b15ba41b7e8fa0bb81ef978bd441052fb937371e))


### Bug Fixes

* **cdp,cli,mcp:** chrome detection, log noise, error chain, explain path leak, watch flag, echo response ([#280](https://github.com/aram-devdocs/plumb/issues/280)) ([0ca2744](https://github.com/aram-devdocs/plumb/commit/0ca2744b531fd240c6a91e51120a6225ceb7ab73))
* **ci,docs:** repair install path and attestation references ([#279](https://github.com/aram-devdocs/plumb/issues/279)) ([51ea69c](https://github.com/aram-devdocs/plumb/commit/51ea69cc6c27898637df848e9b07b165de1ab0a6))


### Documentation

* add versioning policy, security link, contributing link, dedup MCP agent gotchas ([#281](https://github.com/aram-devdocs/plumb/issues/281)) ([dfbc922](https://github.com/aram-devdocs/plumb/commit/dfbc9225dc31868169471eea58e2c1aeff5fd235))
* **cli:** npm-first README + flag wrong-arch Windows ARM64 mapping ([#277](https://github.com/aram-devdocs/plumb/issues/277)) ([6491b62](https://github.com/aram-devdocs/plumb/commit/6491b623d852c83277e57b8bea37a0d732e43390))
* **readme:** demo, Intel Mac note, badges, fix walking-skeleton ref ([#282](https://github.com/aram-devdocs/plumb/issues/282)) ([f417055](https://github.com/aram-devdocs/plumb/commit/f417055e8cbb3153ecf79c06eac40498309b42a7))
* remove stale blog, complete config reference, sweep dead PRD refs ([#276](https://github.com/aram-devdocs/plumb/issues/276)) ([04440da](https://github.com/aram-devdocs/plumb/commit/04440da6d403f8319470e6fd719ad42e712ad306))
* **security:** document actual SLSA attestation verification path ([#285](https://github.com/aram-devdocs/plumb/issues/285)) ([a5626c3](https://github.com/aram-devdocs/plumb/commit/a5626c3bd93f28b43d4c7fc4c550e00ee67962d9))
* **theme:** bump sidebar link min-height to 24px to satisfy a11y/touch-target ([#284](https://github.com/aram-devdocs/plumb/issues/284)) ([9a03f97](https://github.com/aram-devdocs/plumb/commit/9a03f97c686d078c71fbd460f4ba638e3e95930d))

## [0.0.11](https://github.com/aram-devdocs/plumb/compare/v0.0.10...v0.0.11) (2026-05-07)


### Bug Fixes

* **ci:** wire homebrew + npm publish jobs into release.yml ([#272](https://github.com/aram-devdocs/plumb/issues/272)) ([e570b46](https://github.com/aram-devdocs/plumb/commit/e570b46958f4353951a699ffade21b0653cc86a1))

## [0.0.10](https://github.com/aram-devdocs/plumb/compare/v0.0.9...v0.0.10) (2026-05-07)


### Bug Fixes

* **ci:** drop x86_64-apple-darwin target post-V0 ([#270](https://github.com/aram-devdocs/plumb/issues/270)) ([2077aff](https://github.com/aram-devdocs/plumb/commit/2077affed2a7b578a2bcfa08a393b81d05a99fcf))


### Documentation

* **install:** pin --version example to 0.0.9 (latest) ([#268](https://github.com/aram-devdocs/plumb/issues/268)) ([91ec6ec](https://github.com/aram-devdocs/plumb/commit/91ec6ec2ad7699c8bdd178697b0a982a23ce9c0a))

## [0.0.9](https://github.com/aram-devdocs/plumb/compare/v0.0.8...v0.0.9) (2026-05-07)


### Bug Fixes

* **ci:** drop aarch64-pc-windows-msvc target post-V0 ([#266](https://github.com/aram-devdocs/plumb/issues/266)) ([cc7fb64](https://github.com/aram-devdocs/plumb/commit/cc7fb64c4cef88cee249ecb18b97d29f86fe9e10))

## [0.0.8](https://github.com/aram-devdocs/plumb/compare/v0.0.7...v0.0.8) (2026-05-07)


### Bug Fixes

* **cli:** point init template include_str at in-crate templates dir ([#263](https://github.com/aram-devdocs/plumb/issues/263)) ([933be57](https://github.com/aram-devdocs/plumb/commit/933be57286e6d55f1a83480e177b5e5e789b3a84))

## [0.0.7](https://github.com/aram-devdocs/plumb/compare/v0.0.6...v0.0.7) (2026-05-07)


### Bug Fixes

* **ci:** include plumb-codegen in crates.io publish chain ([#261](https://github.com/aram-devdocs/plumb/issues/261)) ([474caa9](https://github.com/aram-devdocs/plumb/commit/474caa9cfd7a4e6a8440093274474e300acad045))

## [0.0.6](https://github.com/aram-devdocs/plumb/compare/v0.0.5...v0.0.6) (2026-05-07)


### Bug Fixes

* **mcp:** rule-docs embed + windows-11-arm runner ([#259](https://github.com/aram-devdocs/plumb/issues/259)) ([35db074](https://github.com/aram-devdocs/plumb/commit/35db074e6d84ca2a6554b4fe81d617e2d0e6afb1))

## [0.0.5](https://github.com/aram-devdocs/plumb/compare/v0.0.4...v0.0.5) (2026-05-07)


### Bug Fixes

* **cdp:** enable plumb-core/test-fake unconditionally ([#257](https://github.com/aram-devdocs/plumb/issues/257)) ([84d9cce](https://github.com/aram-devdocs/plumb/commit/84d9ccef099fd3184feb2ea065a4525c9610f156))

## [0.0.4](https://github.com/aram-devdocs/plumb/compare/v0.0.3...v0.0.4) (2026-05-07)


### Bug Fixes

* **ci:** cargo update + dist rename for v0.0.3 publish ([#253](https://github.com/aram-devdocs/plumb/issues/253)) ([7d31bc2](https://github.com/aram-devdocs/plumb/commit/7d31bc238c70c75eb0d1942091abfeb4bc35cbf9))
* **ci:** publish_tag dispatch + [profile.dist] for v0.0.3 release ([#255](https://github.com/aram-devdocs/plumb/issues/255)) ([b8e525b](https://github.com/aram-devdocs/plumb/commit/b8e525b8d6726e0dc49b68a9173c179ded000da5))

## [0.0.3](https://github.com/aram-devdocs/plumb/compare/v0.0.2...v0.0.3) (2026-05-07)


### Bug Fixes

* **ci:** pin workspace deps to current crate version ([#248](https://github.com/aram-devdocs/plumb/issues/248)) ([4c8ce85](https://github.com/aram-devdocs/plumb/commit/4c8ce8581c3a4c00c4c5c996e93803e484b95a4c))

## [0.0.2](https://github.com/aram-devdocs/plumb/compare/v0.0.1...v0.0.2) (2026-05-07)


### Features

* bootstrap workspace, walking skeleton, and tooling ([0ad9924](https://github.com/aram-devdocs/plumb/commit/0ad9924aa27f4d00c78bc36fb634c4057060adaa))
* **cdp,cli:** driver ergonomics — wait/cookies/storage/animations ([#215](https://github.com/aram-devdocs/plumb/issues/215)) ([d38888c](https://github.com/aram-devdocs/plumb/commit/d38888cfcdceadb78fd35a8b0b93b1b154ad2260))
* **cdp:** add Chromium launch detection ([#94](https://github.com/aram-devdocs/plumb/issues/94)) ([ce9afb9](https://github.com/aram-devdocs/plumb/commit/ce9afb91eb954716a368d48078d158535e4572da))
* **cdp:** chromium auto-fetch via chromiumoxide fetcher ([#78](https://github.com/aram-devdocs/plumb/issues/78)) ([#220](https://github.com/aram-devdocs/plumb/issues/220)) ([b8c6f3b](https://github.com/aram-devdocs/plumb/commit/b8c6f3b474b254f721064422c94571a0922c0143))
* **cdp:** populate computed_styles from inline style="" in snapshot_from_html ([#236](https://github.com/aram-devdocs/plumb/issues/236)) ([feb14a6](https://github.com/aram-devdocs/plumb/commit/feb14a66b58ff869609e00a553b6751ccaca38f8))
* **cdp:** wire DOMSnapshot.captureSnapshot with style whitelist ([#104](https://github.com/aram-devdocs/plumb/issues/104)) ([d062a21](https://github.com/aram-devdocs/plumb/commit/d062a21f8cd218f720596d2126c6161c5b2adb18))
* **cli:** --selector flag scoping lint to subtree ([#127](https://github.com/aram-devdocs/plumb/issues/127)) ([f61428b](https://github.com/aram-devdocs/plumb/commit/f61428b794df6c8deac8b87f4b9f4ae66d1df9db))
* **cli,core,cdp:** --viewport flag + viewport multi-run orchestration ([#103](https://github.com/aram-devdocs/plumb/issues/103)) ([0b7ee6e](https://github.com/aram-devdocs/plumb/commit/0b7ee6e7bb16c39444b77c7f3a761614d3712622))
* **cli,format:** --suggest-ignores flag ([#84](https://github.com/aram-devdocs/plumb/issues/84)) ([#218](https://github.com/aram-devdocs/plumb/issues/218)) ([e6e0ee8](https://github.com/aram-devdocs/plumb/commit/e6e0ee82a4867ac52aed84d734968ce4302c4c0a))
* **cli,format:** deterministic JSON envelope with plumb_version, run_id, summary ([#123](https://github.com/aram-devdocs/plumb/issues/123)) ([8f2d9c3](https://github.com/aram-devdocs/plumb/commit/8f2d9c3bd8a4a531411a5b14712975ba84b8c6c0))
* **cli:** add lint output file flag ([#163](https://github.com/aram-devdocs/plumb/issues/163)) ([3ee72d2](https://github.com/aram-devdocs/plumb/commit/3ee72d235399f4cf3dc01cabe9e633ad3e71a55e))
* **cli:** plumb init with Tailwind detection ([#33](https://github.com/aram-devdocs/plumb/issues/33)) ([#128](https://github.com/aram-devdocs/plumb/issues/128)) ([90fdd50](https://github.com/aram-devdocs/plumb/commit/90fdd5017f6c008d0abb56cdac03c6278cdb8a19))
* **cli:** plumb watch ([#83](https://github.com/aram-devdocs/plumb/issues/83)) ([#219](https://github.com/aram-devdocs/plumb/issues/219)) ([68bbb4f](https://github.com/aram-devdocs/plumb/commit/68bbb4f7f1fc585bd1cd19a30cb4fb8e41ed42c2))
* **codegen:** add plumb-codegen crate + plumb init --from ([#212](https://github.com/aram-devdocs/plumb/issues/212)) ([6b39937](https://github.com/aram-devdocs/plumb/commit/6b39937654b2d7e097f2a63ccb4f8fdb4d0c171f)), closes [#82](https://github.com/aram-devdocs/plumb/issues/82)
* **config:** [color], [radius], [alignment], [a11y] sections + schema ([#109](https://github.com/aram-devdocs/plumb/issues/109)) ([f5b0047](https://github.com/aram-devdocs/plumb/commit/f5b004773dd40cf21f00e81f8a690b85ca70ee6a)), closes [#21](https://github.com/aram-devdocs/plumb/issues/21)
* **config:** add CSS custom-properties scraper ([#114](https://github.com/aram-devdocs/plumb/issues/114)) ([b6dbde1](https://github.com/aram-devdocs/plumb/commit/b6dbde10ed18cfa559ddba5d8931309da1b5bb36))
* **config:** add DTCG 2025.10 token adapter ([#111](https://github.com/aram-devdocs/plumb/issues/111)) ([778e212](https://github.com/aram-devdocs/plumb/commit/778e212318181d65edb3124827c6ad8004703128))
* **config:** add spacing and type schema sections ([#91](https://github.com/aram-devdocs/plumb/issues/91)) ([a649708](https://github.com/aram-devdocs/plumb/commit/a649708f4d9e7ad46e9f3ba1886a0c6c32bcd7fc))
* **config:** span-annotated validation errors via miette ([#110](https://github.com/aram-devdocs/plumb/issues/110)) ([6ea352d](https://github.com/aram-devdocs/plumb/commit/6ea352d4d04cc856663b13631294998b3c13ecf0))
* **config:** tailwind config adapter ([#115](https://github.com/aram-devdocs/plumb/issues/115)) ([36d2c54](https://github.com/aram-devdocs/plumb/commit/36d2c5420a7a51ee598c33d6c987c4b7be6b1594))
* **core:** add baseline/rhythm rule ([#207](https://github.com/aram-devdocs/plumb/issues/207)) ([a1b32eb](https://github.com/aram-devdocs/plumb/commit/a1b32ebecf664b466805d86ba093ed854dd49602))
* **core:** add color/contrast-aa rule ([#202](https://github.com/aram-devdocs/plumb/issues/202)) ([1794cc2](https://github.com/aram-devdocs/plumb/commit/1794cc2ef7da43d1241dc7a2f204617f0fc92963))
* **core:** add phase 7 small rules bundle ([#204](https://github.com/aram-devdocs/plumb/issues/204)) ([1e5abd5](https://github.com/aram-devdocs/plumb/commit/1e5abd59a718391468690bc73c6cc20feb0d2426))
* **core:** bundle MVP rules (radius/a11y/sibling/edge) ([#113](https://github.com/aram-devdocs/plumb/issues/113)) ([2c54e94](https://github.com/aram-devdocs/plumb/commit/2c54e947f64230d59254e6d40cc36f7b18428e18))
* **core:** enrich snapshot context ([#93](https://github.com/aram-devdocs/plumb/issues/93)) ([b0a592d](https://github.com/aram-devdocs/plumb/commit/b0a592dd522aed156a0e5c05eb28e867eb8b42f4))
* **core:** rule color/palette-conformance ([#112](https://github.com/aram-devdocs/plumb/issues/112)) ([0220e55](https://github.com/aram-devdocs/plumb/commit/0220e55e7edb6b6273571b6acd3a62ce255f803e))
* **core:** rules spacing/grid + spacing/scale + type/scale ([#108](https://github.com/aram-devdocs/plumb/issues/108)) ([b00b5e6](https://github.com/aram-devdocs/plumb/commit/b00b5e661941e7dba4fc0c21d6d2cd01ca921287))
* **e2e:** real-world test-site matrix + harness + CI ([#223](https://github.com/aram-devdocs/plumb/issues/223)) ([9b247ef](https://github.com/aram-devdocs/plumb/commit/9b247efb4fd41ee634b2204e253f23abae8a0ea2))
* **format:** add deterministic stats output ([#162](https://github.com/aram-devdocs/plumb/issues/162)) ([f299611](https://github.com/aram-devdocs/plumb/commit/f2996119725d0a97717822a7a0a53fbf551059bd))
* **format:** add SARIF rule metadata ([#161](https://github.com/aram-devdocs/plumb/issues/161)) ([0a7cfc6](https://github.com/aram-devdocs/plumb/commit/0a7cfc687bacd89d0c8dce50354270cdbe2ddea0))
* **gh-issue:** Phase 7/8 — mandatory PR + converge on CI green + Claude review approve ([d2ff65e](https://github.com/aram-devdocs/plumb/commit/d2ff65e32203779729eee1b5b5ca6de0172fb701))
* **gh-runbook:** adaptive dispatch — recommend split/bundle/cluster per batch ([1e3f9fd](https://github.com/aram-devdocs/plumb/commit/1e3f9fd322fc30f764154731851b7c7cf49e455f))
* **gh-runbook:** gate-based progression with parallel batches + per-ticket dispatch ([ccc3fb8](https://github.com/aram-devdocs/plumb/commit/ccc3fb8f4e8d36f210c7cbc3c0e363306bc97bfd))
* **gh-runbook:** GoudEngine-style parent bodies — batch table + copy-pasteable /gh-issue commands ([2ff501b](https://github.com/aram-devdocs/plumb/commit/2ff501b8b6c89e2a496ef55afbfe5a4ae5fb87d3))
* **mcp,cdp:** lint_page_html tool with static HTML parser ([#79](https://github.com/aram-devdocs/plumb/issues/79)) ([#222](https://github.com/aram-devdocs/plumb/issues/222)) ([9a2efe9](https://github.com/aram-devdocs/plumb/commit/9a2efe9438bc86b8758ca859c7966ed1ca877a04))
* **mcp,cdp:** persistent browser contexts for lint_url ([#157](https://github.com/aram-devdocs/plumb/issues/157)) ([6aac6b7](https://github.com/aram-devdocs/plumb/commit/6aac6b7289b733d7f1d44193eae8fb10b9b4b013))
* **mcp:** add authenticated HTTP transport ([#205](https://github.com/aram-devdocs/plumb/issues/205)) ([302ec19](https://github.com/aram-devdocs/plumb/commit/302ec19c527f6445c1940d437e9ae4e137d07e32))
* **mcp:** add explain_rule tool ([#130](https://github.com/aram-devdocs/plumb/issues/130)) ([4bf8237](https://github.com/aram-devdocs/plumb/commit/4bf8237f4663b8e78de545c8890edb1c0d52503c)), closes [#35](https://github.com/aram-devdocs/plumb/issues/35)
* **mcp:** add lint_url full detail mode ([#158](https://github.com/aram-devdocs/plumb/issues/158)) ([46fe12a](https://github.com/aram-devdocs/plumb/commit/46fe12aa1178b7ed2dab92968787fd3704c33e5c))
* **mcp:** compare_viewports tool ([#80](https://github.com/aram-devdocs/plumb/issues/80)) ([#221](https://github.com/aram-devdocs/plumb/issues/221)) ([5a603fb](https://github.com/aram-devdocs/plumb/commit/5a603fbf70002fe558bccf9dd967d2c14e3e66bd))
* **mcp:** expose resolved config resource ([#159](https://github.com/aram-devdocs/plumb/issues/159)) ([222a82a](https://github.com/aram-devdocs/plumb/commit/222a82a8df7989743c864bba444a116593bcf7c2))
* **mcp:** get_config tool ([#132](https://github.com/aram-devdocs/plumb/issues/132)) ([53eb838](https://github.com/aram-devdocs/plumb/commit/53eb83849ab49287c79a974a0ad0811a1aa9e00c))
* **mcp:** lint_url accepts real http(s) URLs via ChromiumDriver ([#131](https://github.com/aram-devdocs/plumb/issues/131)) ([2924d5d](https://github.com/aram-devdocs/plumb/commit/2924d5d7486e371457f3eff9443787056f1c42ba))
* **mcp:** list_rules tool ([#129](https://github.com/aram-devdocs/plumb/issues/129)) ([5d6335e](https://github.com/aram-devdocs/plumb/commit/5d6335eab60c9bc214179db93961cb67855137a6))
* selector-scoped [[ignore]] runtime suppression ([#227](https://github.com/aram-devdocs/plumb/issues/227)) ([ceb28e2](https://github.com/aram-devdocs/plumb/commit/ceb28e2fe866d0911f1e04d92fe015800e282e0f))
* ship every deferred pattern — skills, hooks, workflows, rmcp, xtask ([561bd45](https://github.com/aram-devdocs/plumb/commit/561bd454a1eb29b023e8d43d076c2e583f1e5ccd))
* **skills:** rewrite gh-runbook as spec-driven generator + JSON Schema ([91c4180](https://github.com/aram-devdocs/plumb/commit/91c41808097b71dc508959c6805a3d174d6b048e))
* **xtask:** validate-runbooks subcommand ([7ca7bd5](https://github.com/aram-devdocs/plumb/commit/7ca7bd5afc4b34fdb84b64b0a536d512ec40b364))


### Bug Fixes

* **cdp:** accept Chromium major-version range; e2e tests fail loud ([#126](https://github.com/aram-devdocs/plumb/issues/126)) ([2a5d1e2](https://github.com/aram-devdocs/plumb/commit/2a5d1e2a5a29f83f06c383e4c7b78f3153c28320))
* **cdp:** handle optional DOMSnapshot string sentinels ([#191](https://github.com/aram-devdocs/plumb/issues/191)) ([b1d0639](https://github.com/aram-devdocs/plumb/commit/b1d06396612e67ba0f04ff517867392c08bc5c51)), closes [#190](https://github.com/aram-devdocs/plumb/issues/190)
* **cdp:** scale FakeDriver viewport rects to configured target ([#125](https://github.com/aram-devdocs/plumb/issues/125)) ([c0a853a](https://github.com/aram-devdocs/plumb/commit/c0a853ab65c4b40ed277e5cde7cb27f570c48c82)), closes [#121](https://github.com/aram-devdocs/plumb/issues/121)
* **ci:** bypass delegation-guard during merge conflict resolution ([#133](https://github.com/aram-devdocs/plumb/issues/133)) ([3c951d0](https://github.com/aram-devdocs/plumb/commit/3c951d0769fb9b9ac1e8d2b51c64d4a603c17e8e))
* **ci:** drop cargo-workspace plugin from release-please ([#99](https://github.com/aram-devdocs/plumb/issues/99)) ([a15f910](https://github.com/aram-devdocs/plumb/commit/a15f9103c8ccad18c3ce8e4cbaf56cf66ffa422a))
* **ci:** hide MSRV version from dependabot's action-tag scanner ([#102](https://github.com/aram-devdocs/plumb/issues/102)) ([2261152](https://github.com/aram-devdocs/plumb/commit/22611521f5af567f1ff417975a2224a2468954d6))
* **ci:** make phase 3 gate environment reproducible ([#160](https://github.com/aram-devdocs/plumb/issues/160)) ([926eb83](https://github.com/aram-devdocs/plumb/commit/926eb836109327fff31c6ee6f3f2f32252a6add9))
* **ci:** post Claude review verdict as sticky PR comment ([#106](https://github.com/aram-devdocs/plumb/issues/106)) ([ecd1f3f](https://github.com/aram-devdocs/plumb/commit/ecd1f3ff23a602c0e4b8640528181928e2a74871))
* **ci:** skip Claude review on dependabot/renovate PRs ([#225](https://github.com/aram-devdocs/plumb/issues/225)) ([9a0f90c](https://github.com/aram-devdocs/plumb/commit/9a0f90c360e6e1f4c8a32b863fad58713e87e06a))
* **ci:** use simple release-type with TOML extra-file for workspace version ([#100](https://github.com/aram-devdocs/plumb/issues/100)) ([547aeab](https://github.com/aram-devdocs/plumb/commit/547aeabb58af3a264545d6c93f1a4aef66279a14))
* **cli:** error when --viewport is passed without configured [viewports] ([#119](https://github.com/aram-devdocs/plumb/issues/119)) ([#124](https://github.com/aram-devdocs/plumb/issues/124)) ([5849499](https://github.com/aram-devdocs/plumb/commit/58494990d48dd9e9b4d6a465840b662067f5e136))
* **cli:** serialize mcp_http tests to fix port-reservation TOCTOU race ([#226](https://github.com/aram-devdocs/plumb/issues/226)) ([ffddaee](https://github.com/aram-devdocs/plumb/commit/ffddaee2f4df525120db06712a705a62a4851eb0))
* **e2e:** drop nextjs advisory by threading wait_for sentinel ([#231](https://github.com/aram-devdocs/plumb/issues/231)) ([d3811d4](https://github.com/aram-devdocs/plumb/commit/d3811d4e4937ce973edaf4cbb3d54c6507b67709))
* **e2e:** static-export refactor for Next.js fixture (closes [#233](https://github.com/aram-devdocs/plumb/issues/233)) ([#235](https://github.com/aram-devdocs/plumb/issues/235)) ([3ffecbf](https://github.com/aram-devdocs/plumb/commit/3ffecbf72e22f9d7b102d9a08963074635e4de6c))
* **gh-runbook:** file-backed state + substitute {{PARENT_ISSUE}} before child creation ([1304a83](https://github.com/aram-devdocs/plumb/commit/1304a83c295399a514a28d12da4606790f82375d))
* **hooks:** detect subagent context at PreToolUse, not SessionStart ([#116](https://github.com/aram-devdocs/plumb/issues/116)) ([b39a55b](https://github.com/aram-devdocs/plumb/commit/b39a55bc6e0fed140ba48babae26cdc97c0761cb))
* **hooks:** pin HOOK_SESSION_ID to stdin session_id so review gates compose across hook invocations ([#105](https://github.com/aram-devdocs/plumb/issues/105)) ([dd0cd18](https://github.com/aram-devdocs/plumb/commit/dd0cd18e993bace545d607d411f58946b4a779c5))


### Performance

* **cdp:** add benchmark harness report ([#180](https://github.com/aram-devdocs/plumb/issues/180)) ([9ce7a26](https://github.com/aram-devdocs/plumb/commit/9ce7a26fb07d1e34a76f00a209c0009730742bec)), closes [#61](https://github.com/aram-devdocs/plumb/issues/61)
* **cdp:** add fixed DOM benchmark fixtures ([#179](https://github.com/aram-devdocs/plumb/issues/179)) ([5083a54](https://github.com/aram-devdocs/plumb/commit/5083a5437351ae06a54aef842d2deb18bd90c93a))


### Documentation

* acknowledge v3 multi-agent setup in README ([#141](https://github.com/aram-devdocs/plumb/issues/141)) ([1adf039](https://github.com/aram-devdocs/plumb/commit/1adf039be201cde0bd0237f9d28309657ff88e14))
* add landing demo asset and CTA verification ([#184](https://github.com/aram-devdocs/plumb/issues/184)) ([1bd59a7](https://github.com/aram-devdocs/plumb/commit/1bd59a7d683e3d4149e7c0591c30b3dd5a04da80))
* add MCP cookbook and FAQ ([#175](https://github.com/aram-devdocs/plumb/issues/175)) ([546e075](https://github.com/aram-devdocs/plumb/commit/546e075cdcd0e9f2882ef6949a187eba7a368bf8))
* add V0 decision ADRs ([#177](https://github.com/aram-devdocs/plumb/issues/177)) ([3b56d62](https://github.com/aram-devdocs/plumb/commit/3b56d62601ba5b8cb3bfa4a9d5ccde01489f1129))
* add V0 release-readiness runbook shell ([#195](https://github.com/aram-devdocs/plumb/issues/195)) ([a08bff3](https://github.com/aram-devdocs/plumb/commit/a08bff3f40b1b3b2a8d5f35af99f9053aa5462f5))
* add v0.6 launch post ([#182](https://github.com/aram-devdocs/plumb/issues/182)) ([190becf](https://github.com/aram-devdocs/plumb/commit/190becf82b65c35dd9efeec5dfcec46edd2bd386))
* address config review feedback ([#139](https://github.com/aram-devdocs/plumb/issues/139)) ([7819bc9](https://github.com/aram-devdocs/plumb/commit/7819bc9dafe04df9790fc66d13a1758907c765b2))
* **adr:** record Chromium exact-pin to version-range remediation ([#136](https://github.com/aram-devdocs/plumb/issues/136)) ([d408bde](https://github.com/aram-devdocs/plumb/commit/d408bdef55020a8cb99a0b1201faf13a33fd542a))
* **agents:** hierarchical AGENTS.md + CLAUDE.md symlinks + size validator ([25e7719](https://github.com/aram-devdocs/plumb/commit/25e7719edbe9b9e4b633b2d5d677c8ed9a26d5d1))
* **ci:** add code scanning and reviewdog guides ([#164](https://github.com/aram-devdocs/plumb/issues/164)) ([b12a384](https://github.com/aram-devdocs/plumb/commit/b12a3842510b24b168fdd1dbe502f6e15e1681cf))
* **docs:** add v3.1.1 ignoreOtherMentions proof ([#152](https://github.com/aram-devdocs/plumb/issues/152)) ([3fe44e3](https://github.com/aram-devdocs/plumb/commit/3fe44e3a55074fa500be5ac8eff4eadc19e772b5))
* install + quick-start + config reference chapters ([#135](https://github.com/aram-devdocs/plumb/issues/135)) ([95da8e7](https://github.com/aram-devdocs/plumb/commit/95da8e72de499978c6aeb0cbc732fd1d6a189466))
* live MCP verification + honest dogfood acceptance (closes [#230](https://github.com/aram-devdocs/plumb/issues/230)) ([#237](https://github.com/aram-devdocs/plumb/issues/237)) ([7eab7eb](https://github.com/aram-devdocs/plumb/commit/7eab7eba243910c8c6ceee7139a9b4d1b62bfafb))
* refresh landing page copy ([#183](https://github.com/aram-devdocs/plumb/issues/183)) ([fda4468](https://github.com/aram-devdocs/plumb/commit/fda446820547e64519b1996f5e39956ebb67e259))
* retarget phase 1 acceptance criterion to checked-in fixture ([#122](https://github.com/aram-devdocs/plumb/issues/122)) ([fae2b39](https://github.com/aram-devdocs/plumb/commit/fae2b394b7c53c44645e6f27dbe85c4b1a8fd7dd)), closes [#120](https://github.com/aram-devdocs/plumb/issues/120)
* **rules:** add no-legacy-code policy + workspace lints ([#107](https://github.com/aram-devdocs/plumb/issues/107)) ([2f280e0](https://github.com/aram-devdocs/plumb/commit/2f280e06c8783e9c36c81419cfa5c3f2574652ab))
* **runbooks:** V0→V1 delivery specs for all 7 phases + roadmap umbrella + phase labels ([6e47ea7](https://github.com/aram-devdocs/plumb/commit/6e47ea733ea9e9c36a3bdbbef0f5adb0dd539be4))
* V0 ship-ready cleanup pass — DRY, accurate, no AI-tell ([#247](https://github.com/aram-devdocs/plumb/issues/247)) ([d56262f](https://github.com/aram-devdocs/plumb/commit/d56262f30bc908b2c788588187eb1c8e1456754b))
* validate landing-page demo asset and CTAs ([#185](https://github.com/aram-devdocs/plumb/issues/185)) ([086c0d2](https://github.com/aram-devdocs/plumb/commit/086c0d2b6ab83c620ca572b6cccbc7c94d9f3ccb))
