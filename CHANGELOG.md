# Changelog

All notable changes to this project will be documented in this file.

## [1.2.1] - 2026-03-21

### Bug Fixes

- Guard ResizeObserver callback against disconnected canvas([66982e7](https://github.com/auser/workflow-graph/commit/66982e7cf012192e7ce469e83df10a7fbf5330a9))

### Miscellaneous

- Bump workspace version to 1.2.1([290e349](https://github.com/auser/workflow-graph/commit/290e349d4b0d4c2ee3dd5f4910e254c4a82e0045))
## [1.2.0] - 2026-03-21

### Features

- Production node rendering, persistence fix, NodeDefinition API([013b095](https://github.com/auser/workflow-graph/commit/013b095b7d848e995c2af631f9006e2daf6c4956))

### Miscellaneous

- Bump workspace version to 1.2.0([25d6fd3](https://github.com/auser/workflow-graph/commit/25d6fd3fd9d1208af4ccc46157530f1bfc10c633))
## [0.5.1] - 2026-03-21

### Miscellaneous

- Bump workspace version to 0.5.1([c82b420](https://github.com/auser/workflow-graph/commit/c82b420e77c48b36d1a623443ae54d2a95ce74b4))
## [0.5.1] - 2026-03-21

### Features

- Add native drag-drop support to canvas([ef08fd8](https://github.com/auser/workflow-graph/commit/ef08fd80f35cb2e6c62fe9d50cc5c549ba265753))

### Miscellaneous

- Bump to 0.5.1([1ad7e58](https://github.com/auser/workflow-graph/commit/1ad7e58e6ff1eaaf3f81a45763ed755a76b41966))
## [0.5.0] - 2026-03-20

### Features

- Add metadata field to Job/Edge and WASM-level node CRUD API([a86bbfa](https://github.com/auser/workflow-graph/commit/a86bbfa474b7226b2c4a22016e5f713adf82795a))

### Bug Fixes

- Correct workspace dep pattern in bump-versions recipe([c889c1c](https://github.com/auser/workflow-graph/commit/c889c1c9589a1dc302970ddeb83cbd451ebb87ae))
- Move test module to end of file to satisfy clippy([ba7a0ad](https://github.com/auser/workflow-graph/commit/ba7a0ada8ba45ebef3e7af39e367da030bd438dc))

### Styling

- Apply clippy auto-fixes([4bd43a0](https://github.com/auser/workflow-graph/commit/4bd43a0fbe3e6b84af1891c1902dd5bce39d3bc4))

### Testing

- Add tests for node CRUD API and update documentation([8124320](https://github.com/auser/workflow-graph/commit/8124320f2918dd0397c0bc31d3956a6f3a2375df))

### Miscellaneous

- Bump npm packages to 0.5.0([5ae3dad](https://github.com/auser/workflow-graph/commit/5ae3dadd69402375e99755acd608a41da5d9b04c))
- Add changelog for v0.5.0([85cc2f9](https://github.com/auser/workflow-graph/commit/85cc2f9773c432467b96739f279b119ee97e44d0))
- Bump workspace version to 0.5.0([ad055e3](https://github.com/auser/workflow-graph/commit/ad055e324c5b510ba3bf52740667fdf61aad3915))
## [0.4.0] - 2026-03-17

### Bug Fixes

- Tolerate already-published npm packages on re-run([a07be90](https://github.com/auser/workflow-graph/commit/a07be90a32d9d7a895b16c6b70d91c2e7c3ed1ee))
- Tolerate already-published crates on re-run([11fc86e](https://github.com/auser/workflow-graph/commit/11fc86e6bf5b06aa6857aeaf50ee52be1446f9ba))
## [0.4.1] - 2026-03-17

### Features

- React-app example with Vite + React([e827997](https://github.com/auser/workflow-graph/commit/e827997eb56ea9bddce84dadb3da62a92a702bc6))
- Vanilla-web example with Vite dev server([8724e0a](https://github.com/auser/workflow-graph/commit/8724e0a942afcf1cec14c9cc52c4fc0a6295e648))

### Bug Fixes

- Bump npm package versions to 0.4.0 to match workspace([b515ea8](https://github.com/auser/workflow-graph/commit/b515ea86b061e519f94582a39829c95e95817d50))
- Serve WASM as static asset and set explicit URL in React example([3345e33](https://github.com/auser/workflow-graph/commit/3345e334f5eef0ff16c16e4acc969f2553dad259))
- Use Vite aliases to resolve packages from source([e613646](https://github.com/auser/workflow-graph/commit/e61364686be1a73574c5badd87f77deeb627f7b3))
- Add PORT env var to client-polling and server URL logging to worker([62220b8](https://github.com/auser/workflow-graph/commit/62220b874705ef36615fc11b7ce434b9c3ac1dc4))
- Allow Vite to serve WASM files from monorepo root([e04969a](https://github.com/auser/workflow-graph/commit/e04969a0da92f464d5e56a513e385d0543963f9e))
- Don't interrupt spinner animation with static redraws([95ebdb1](https://github.com/auser/workflow-graph/commit/95ebdb1a793a1a4185338ba28fd02f0d97f6fc81))
- Only guard re-run when jobs are Running, not Queued([b145be0](https://github.com/auser/workflow-graph/commit/b145be0207c65d5c7394ac40482cd86d13bbb0c3))
- Client-polling example uses local package until next npm release([51e1e01](https://github.com/auser/workflow-graph/commit/51e1e01029671d31fa5560d592752cd05977159a))
- Prevent re-running a workflow that is already active([fa26e03](https://github.com/auser/workflow-graph/commit/fa26e03b6650c60192d4d1b69823ea52407fc53c))
- Justfile port reads from PORT env var([94647e4](https://github.com/auser/workflow-graph/commit/94647e40f56abe0ff11d38795d5d5c59c7795d06))
- Default client-polling example to port 4000([f40cb02](https://github.com/auser/workflow-graph/commit/f40cb02de09aa546ac71eb9f84e28cb7f08b8d7a))
- Add package.json and @types/node to client-polling example([fe09b1c](https://github.com/auser/workflow-graph/commit/fe09b1cfc6b01cc46096e675b1edb7dc5545d93d))
- Add type:module to npm packages and fix client-polling example([7073bc9](https://github.com/auser/workflow-graph/commit/7073bc9836cda29663841b037882469d61f2436b))

### Documentation

- Add setWasmUrl() requirement and update all examples([52fd8bb](https://github.com/auser/workflow-graph/commit/52fd8bbe6b83a7cc09f09564e7fc7513285c07f0))
- Update client-polling README with 3-terminal setup([6a11270](https://github.com/auser/workflow-graph/commit/6a11270e5f72552073ade84ad82dfbbea0d731ed))
- Add working examples for each npm package([15f1dc1](https://github.com/auser/workflow-graph/commit/15f1dc11c1edf2eeb18ce7a2be1c51d1a5d3b1f4))
- Add npm badges to README and update quick-start guide([a7e63e1](https://github.com/auser/workflow-graph/commit/a7e63e1e3706e9e2044e56074d90dc2c21b4d883))

### Miscellaneous

- Add changelog entry for v0.4.1([c780795](https://github.com/auser/workflow-graph/commit/c780795217d0d7340ba51a637a1e9bc0fd133ea9))
- Bump workspace version to 0.4.1([16b2329](https://github.com/auser/workflow-graph/commit/16b2329bc28b813fb6c3b2e8029927516437da56))
- Add changelog for v0.4.0([932d217](https://github.com/auser/workflow-graph/commit/932d2174db7557d7c4f8bdbd607cefb2341f2c90))
- Bump workspace version to 0.4.0([4c91ce2](https://github.com/auser/workflow-graph/commit/4c91ce21a68e7b627e9e4b7fa0a78075f3379042))
## [0.3.0] - 2026-03-17

### Features

- Rename npm scope from @workflow-graph to @auser([2e954e7](https://github.com/auser/workflow-graph/commit/2e954e7d6b8913042710fb77a7c0ae9d046cf7ef))

### Bug Fixes

- Sync workspace dep versions to 0.3.0([82a2131](https://github.com/auser/workflow-graph/commit/82a21316b64677745b6a6a80dee68cc82293ab2b))
- Bump-versions recipe now syncs workspace.dependencies versions([3c55c22](https://github.com/auser/workflow-graph/commit/3c55c22b339595970b1583004a8279151829a3a6))

### Miscellaneous

- Add changelog for v0.3.0([e47fa1c](https://github.com/auser/workflow-graph/commit/e47fa1c5968a8ce9f9c2fdb1f5251678843a0bd6))
## [0.2.6] - 2026-03-17

### Bug Fixes

- Resolve React TS build in CI with paths mapping([9426776](https://github.com/auser/workflow-graph/commit/9426776026de69641ec7aa8edd8ac67c13c13841))

### Miscellaneous

- Add changelog for v0.2.6([19abb6a](https://github.com/auser/workflow-graph/commit/19abb6aed6b3d15d04b8b9d68d336ce8f43564cb))
- Bump workspace version to 0.2.6([7620187](https://github.com/auser/workflow-graph/commit/76201874222bb86de633fc7d7be237e127a61794))
## [0.2.5] - 2026-03-17

### Bug Fixes

- Use --legacy-peer-deps and --no-save for React CI build([fb89dca](https://github.com/auser/workflow-graph/commit/fb89dca309cdaa3c2299b038506d496aa0a216a3))

### Miscellaneous

- Add changelog for v0.2.5([3b551ce](https://github.com/auser/workflow-graph/commit/3b551ce1d242b04ede9d7cb29610fa768e70d3f1))
- Bump workspace version to 0.2.5([87e341c](https://github.com/auser/workflow-graph/commit/87e341c80fe0d6329cc83b4d6de41cd4a01dad50))
## [0.2.4] - 2026-03-17

### Bug Fixes

- Install React types and sibling dep before building in CI([aaf0655](https://github.com/auser/workflow-graph/commit/aaf0655c560b0f1c533599bd1faf1c8ffa52672c))

### Miscellaneous

- Add changelog for v0.2.4([96a669d](https://github.com/auser/workflow-graph/commit/96a669d5361e2760bd76197f01a67af407dcdf1d))
- Bump workspace version to 0.2.4([2b3c8a0](https://github.com/auser/workflow-graph/commit/2b3c8a04af643e2c243a62d02df922b276ec0a61))
## [0.2.3] - 2026-03-17

### Bug Fixes

- Bump actions to v5 and fix TS compilation for WASM import([eb38c83](https://github.com/auser/workflow-graph/commit/eb38c838d259de29aebb2381b97b631fbdc329c5))
- Sync workspace dependency versions to 0.2.2([be4d63f](https://github.com/auser/workflow-graph/commit/be4d63f7d8ab2a11e12f03dbf915d4cd6fde7917))

### Miscellaneous

- Add changelog for v0.2.3([89bf201](https://github.com/auser/workflow-graph/commit/89bf2019879b27954e737bdca72221bcc6eedaa4))
- Bump workspace version to 0.2.3([b8a26bf](https://github.com/auser/workflow-graph/commit/b8a26bf78ccc078e0147c11d74fd62e053a98e7d))
- Remove Dependabot — managing dependencies manually([7ce3280](https://github.com/auser/workflow-graph/commit/7ce3280b975718ed4e3fa16d7bc5ecbef7cb0234))
## [0.2.2] - 2026-03-17

### Bug Fixes

- Resolve Publish Packages CI failures([ffc5ce1](https://github.com/auser/workflow-graph/commit/ffc5ce15a16563f1182c7ec0786dc1de6f27fa7e))

### Miscellaneous

- Add changelog for v0.2.2([a0476bc](https://github.com/auser/workflow-graph/commit/a0476bc4946a159b024c9d75a76f61f4a3577dd5))
- Bump workspace version to 0.2.2([86d725a](https://github.com/auser/workflow-graph/commit/86d725a9d1425a8d933216489142a91df0c41979))
## [0.2.1] - 2026-03-17

### Bug Fixes

- Resolve CI failures in Publish and Deploy Docs workflows([401c5d4](https://github.com/auser/workflow-graph/commit/401c5d4ae7f23288d859915be12f394809298dc0))
- Reduce Dependabot noise with grouped PRs and lower limits([fc9e541](https://github.com/auser/workflow-graph/commit/fc9e541248b518d6b8baea0803d28d24b3b0c311))

### Miscellaneous

- Add changelog for v0.2.1([4e4c6ae](https://github.com/auser/workflow-graph/commit/4e4c6aed59d4fb207b9a7074689e42e9f3a1ceb4))
- Bump workspace version to 0.2.1([b493c0b](https://github.com/auser/workflow-graph/commit/b493c0b366cf933f35d02e4f88671be8e031bb8f))
## [0.2.0] - 2026-03-17

### Features

- Production-ready customizable component([6f2fc75](https://github.com/auser/workflow-graph/commit/6f2fc75867184678fabbc4a52411e44409f217b0))
- Integrate real WASM library into landing page demo([98fc3bf](https://github.com/auser/workflow-graph/commit/98fc3bf3ef6ab95337630013c558c730a366b143))
- Add drag-and-drop nodes, YAML view, and fix layout clipping([d239b29](https://github.com/auser/workflow-graph/commit/d239b2967430abd6c9b50b5e9ef46dd8821d02bf))
- Rebuild landing page demo to match full ci.yml workflow([e374312](https://github.com/auser/workflow-graph/commit/e3743129eea8b1ba1caf602d30b03db31d320103))

### Bug Fixes

- Resolve clippy warnings and add AGENTS.md([945ec31](https://github.com/auser/workflow-graph/commit/945ec31c8cf594f5cac1260d4f6f0b9b6a01ecc5))
- Ensure trailing slash on BASE_URL for WASM path([bd4cf67](https://github.com/auser/workflow-graph/commit/bd4cf67402fb572b6c9e38b7a40666da265191c4))
- Remove YAML tab toggle from demo([d5ee2d4](https://github.com/auser/workflow-graph/commit/d5ee2d451cd5ccf684d2f47025f342862a93a5be))
- Remove arrow markers from edge connections in demo([1aeb0f8](https://github.com/auser/workflow-graph/commit/1aeb0f8f40d985a3c5821a6f9ba97a618dfec179))
- Use global scoped styles for demo and opt into Node.js 24([b3c07f5](https://github.com/auser/workflow-graph/commit/b3c07f59965bf4880d0c99793ed4b697b7a2b9fc))
- Override Starlight global styles on demo labels and buttons([1cb01fa](https://github.com/auser/workflow-graph/commit/1cb01faa87fda5bf13b186f2a17919c456b83577))
- Include Cargo.lock update in release version bump([119c427](https://github.com/auser/workflow-graph/commit/119c427a274cb0af425fa913eb4969600b761692))

### Miscellaneous

- Add changelog for v0.2.0([a96d2cf](https://github.com/auser/workflow-graph/commit/a96d2cf19aa6302d1fd4929514146720d23d10ae))
- Bump workspace version to 0.2.0([a2b47d8](https://github.com/auser/workflow-graph/commit/a2b47d87089864645a4c72ab924162ec07b689f7))
## [0.1.1] - 2026-03-17

### Bug Fixes

- Use workspace-inherited versions and fix bump-versions recipe([366df7a](https://github.com/auser/workflow-graph/commit/366df7a47691b7e1edb5be53e1295c3ecf003399))

### CI/CD

- Fix deploy-pages version and add release workflow([cba20e0](https://github.com/auser/workflow-graph/commit/cba20e0691712f8918e8824bdb3e9084b0fc78ff))

### Miscellaneous

- Add changelog for v0.1.1([a8d0cb8](https://github.com/auser/workflow-graph/commit/a8d0cb865e7278d4d9f2385432477457e1b24cc1))
- Bump workspace version to 0.1.1([076b5d9](https://github.com/auser/workflow-graph/commit/076b5d9d337cdfe85af1d85a18ff01fd391fb9f0))
## [0.1.0] - 2026-03-17

### Bug Fixes

- Fix release-auto cliff config and add bump-versions recipe([d8451cf](https://github.com/auser/workflow-graph/commit/d8451cfe80540546d61bc17c9526c4accbeda281))
- Resolve all clippy warnings in web crate([d2e1a4d](https://github.com/auser/workflow-graph/commit/d2e1a4d35dfc7a5dbc69a33e9205ce9a1e495231))

### Documentation

- Add Vercel, Cloudflare Workers, and Supabase Edge deployment guides([55ff436](https://github.com/auser/workflow-graph/commit/55ff43680c2bf08f0b9c0b47bbdbbc333a378e80))

### Refactoring

- Improve scheduler, web, and worker-sdk implementations([420d4eb](https://github.com/auser/workflow-graph/commit/420d4ebeb40cef513f4ee0bf2c7cc2d3863e2e77))
- Simplify crate code and add release verification script([b08cd40](https://github.com/auser/workflow-graph/commit/b08cd40a2bba679db83d5292ed9a9c6767084a8b))

### CI/CD

- Bump GitHub Actions to v5 (Node.js 24 support)([5038435](https://github.com/auser/workflow-graph/commit/5038435e1a43153fdf8125a44204c12d82daf2cf))

### Miscellaneous

- Add changelog for v0.1.0([cd12d1b](https://github.com/auser/workflow-graph/commit/cd12d1b5230df7ba690c4bb3c56d61ad84d77ce4))
- Bump workspace version to 0.1.0([1e88070](https://github.com/auser/workflow-graph/commit/1e880707f94bfbb6fef0d42ecadd47b489f9591e))

### Other

- Update README guide links to point to GitHub Pages docs site

Replace broken local file links (docs/guide-workers.md, etc.) with
links to the live documentation at auser.github.io/workflow-graph.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([1b1ccd6](https://github.com/auser/workflow-graph/commit/1b1ccd60ab31cb5b1834e9beddd69995c442f62e))
- Add git-cliff config and release-auto Justfile recipe

- cliff.toml with conventional commit parsing and GitHub link generation
- `just release-auto <version>` bumps all crate versions, generates
  CHANGELOG.md, commits, tags, and pushes
- `just changelog` previews unreleased changes

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([a6212f8](https://github.com/auser/workflow-graph/commit/a6212f8d97618495beb5d8baa385185fcd55b4a2))
- Merge branch 'feat/queue-worker-system'([91acc88](https://github.com/auser/workflow-graph/commit/91acc88f803a1549a340c6c614550907205eae78))
- Add docs/.astro/ to gitignore

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([f450238](https://github.com/auser/workflow-graph/commit/f4502382129eb327dc7c67472c2bee3a9285f8a0))
- Add edge deployment split, standalone scheduler, and workflow ops

- Add standalone scheduler crate for split deployments (stateless API
  + separate long-running scheduler process)
- Extract workflow operations into workflow_ops module for the server
- Update server to support API_ONLY mode via environment variable
- Add edge deployment architecture plan (003)
- Update sprint tracking, specs, and dependency versions across all
  crates and NPM packages

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([e69f1db](https://github.com/auser/workflow-graph/commit/e69f1dbbe3bb456e12bc217eb175d2c4c6bfc187))
- Add creating workers guide with Python, TypeScript, Go examples

Step-by-step tutorial covering standalone binary, Rust SDK, and
custom HTTP workers in Python, TypeScript/Node.js, and Go, plus
best practices for heartbeats, idempotency, and error handling.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([eba91a5](https://github.com/auser/workflow-graph/commit/eba91a5731c43da6b0bfa9156948389f299ff2fc))
- Add animated workflow demo to landing page, Postgres and Redis guides

- Add SVG-based animated workflow graph demo that cycles through job
  statuses (queued → running → success/failure) with hover edge
  highlighting and a replay button
- Add Postgres/pg-boss backend guide with full schema, trait impls,
  and LISTEN/NOTIFY for split deployments
- Add Redis backend guide with Lua-scripted atomic claiming, pub/sub
  events, and comparison table

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([41c513f](https://github.com/auser/workflow-graph/commit/41c513f2a2460186d3e8c8a60c7ea05029526d34))
- Add Astro + Starlight documentation site with GitHub Pages deployment

Create a full documentation site in docs/ using Astro + Starlight with
13 content pages migrated from README.md, guide-workers.md, and
guide-postgres.md. Includes a landing page with hero section and feature
cards, organized sidebar navigation, and a GitHub Actions workflow for
automated deployment to GitHub Pages.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([9ff6fea](https://github.com/auser/workflow-graph/commit/9ff6feab022c25426a101e14acb96b9c1ab073fe))
- Add integration guides for Postgres, Redis, and worker authoring

Three new docs covering how to implement all four backend traits
(JobQueue, ArtifactStore, LogSink, WorkerRegistry) with Postgres/pg-boss
and Redis, plus a comprehensive guide for writing workers in any language
including HTTP protocol reference, custom executors, and a Python example.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([5f5f271](https://github.com/auser/workflow-graph/commit/5f5f271b9efedf1215f86ad8a2155b0cf2c63f1f))
- Complete all sprint items: SSE, NPM packages, accessibility, docs

SSE Log Streaming:
- GET /api/workflows/{wf_id}/jobs/{job_id}/logs/stream — SSE endpoint
  replays existing chunks then streams live via broadcast
- Frontend log panel: click node → fetch and display logs
- Click empty space → hide log panel

TypeScript NPM Packages:
- @github-graph/web: WorkflowGraph class wrapping WASM with full API
- @github-graph/react: <WorkflowGraphComponent /> with props
- @github-graph/client: WorkflowClient with REST + SSE streaming

Accessibility:
- Canvas role="img" + aria-label + tabindex="0"
- Tab/Shift+Tab cycles through nodes
- Enter/Space activates (fires click callback)
- Escape deselects all

All sprint items marked complete. 22 tests passing.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([f943ebc](https://github.com/auser/workflow-graph/commit/f943ebc6267fad6b31f6593c0495aa0a0e2f8224))
- Add comprehensive README with architecture, API docs, and guides

Covers: features, architecture diagram, crate structure, quick start,
workflow YAML schema, WASM API reference, REST API table, custom
queue backend guide (pg-boss mapping), server embedding guide, testing.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([f9517f0](https://github.com/auser/workflow-graph/commit/f9517f06e1e3cf3bb582f3539f8a9cdfc3c054db))
- Add pan/zoom, selection, events, and control API (Web Component)

Pan & Zoom:
- Mouse wheel zoom centered on cursor (0.25x to 4x)
- Click+drag on empty canvas space to pan
- Transform applied via ctx.translate/scale in render

Selection:
- Click node → selected (blue border ring)
- Shift+click → toggle multi-select
- Click empty space → deselect all
- Selected state persists across redraws

Event Callbacks (all optional):
- on_node_click(jobId) — fires on click, not drag
- on_node_hover(jobId | null) — fires on hover enter/exit
- on_node_drag_end(jobId, x, y) — fires when drag completes
- on_canvas_click() — fires on empty space click
- on_selection_change(selectedIds[]) — fires on selection change

Programmatic Control API:
- select_node, deselect_all, reset_layout, zoom_to_fit
- set_zoom, get_node_positions, set_node_positions, destroy

22 tests passing.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([f18ea47](https://github.com/auser/workflow-graph/commit/f18ea475e8b0bc8ac661f5858ee30ad7e12e9ed7))
- Add labels/retries to YAML schema, complete log API (Phases 5-6)

Shared types: add required_labels, max_retries, attempt to Job
(all #[serde(default)] for backwards compat).

YAML parser: add labels and retries fields to JobDef, propagate
through into_workflow().

Sample workflow updated with labels (linux, aws) and retries
to demonstrate the new schema.

Log collection API already wired in Phase 3:
- POST /api/jobs/{lease_id}/logs (worker pushes chunks)
- GET /api/workflows/{wf_id}/jobs/{job_id}/logs (historical)

22 tests passing.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([d3b283d](https://github.com/auser/workflow-graph/commit/d3b283d028247e3bc31af8e08169a0b347c5684a))
- Add Worker SDK with poll/execute/heartbeat/log streaming (Phase 4)

New crate: github-graph-worker-sdk

Worker struct with configurable:
- Poll interval, lease TTL, heartbeat interval
- Log batch interval, cancellation check interval
- Server URL, worker labels

Worker loop: register → poll/claim → execute with concurrent:
- Heartbeat sender (renews lease every TTL/3)
- Cancellation checker (polls server, kills child on cancel)
- Log streamer (batches stdout/stderr lines, pushes to server)

Executor: spawns sh -c, reads stdout/stderr incrementally via
AsyncBufReadExt, streams log chunks to server in batches.
Supports graceful cancellation via tokio CancellationToken.

Standalone binary: configurable via SERVER_URL and WORKER_LABELS
env vars. Can also be embedded as a library.

22 tests passing across workspace.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([7429ca4](https://github.com/auser/workflow-graph/commit/7429ca4b345a64e4651780400c841238041f2644))
- Wire queue system into server, add worker protocol API (Phase 3)

Replace inline orchestrator with queue-backed architecture:
- Delete orchestrator.rs and executor.rs (logic now in queue crate)
- Server creates InMemory* backends, spawns DagScheduler event loop
  and lease reaper background task
- Expose create_router() for library consumers to embed in their apps
- run_workflow now calls scheduler.start_workflow()

New API endpoints for worker protocol:
- POST /api/workers/register — register worker with labels
- POST /api/workers/{id}/heartbeat — worker heartbeat
- GET  /api/workers — list registered workers
- POST /api/jobs/claim — atomic job claim with lease TTL
- POST /api/jobs/{lease_id}/heartbeat — renew job lease
- POST /api/jobs/{lease_id}/complete — report success + outputs
- POST /api/jobs/{lease_id}/fail — report failure
- POST /api/jobs/{lease_id}/logs — push log chunks
- GET  /api/jobs/{wf_id}/{job_id}/cancelled — check cancellation
- POST /api/workflows/{id}/cancel — cancel workflow
- GET  /api/workflows/{wf_id}/jobs/{job_id}/logs — get job logs

22 tests passing across workspace.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([86521e0](https://github.com/auser/workflow-graph/commit/86521e0b0cf9787d969cf4f6d11e350454e4f6e4))
- Add DagScheduler with event-driven DAG cascade (Phase 2)

Event-driven scheduler that subscribes to JobQueue events:
- start_workflow(): resets jobs, enqueues roots (no deps)
- On Completed: finds downstream jobs with all deps satisfied,
  collects upstream outputs from ArtifactStore, enqueues them
- On Failed (non-retryable): marks transitive downstream as Skipped
- On LeaseExpired: marks job as Queued for retry
- On Cancelled: marks job as Cancelled
- Updates SharedState so frontend polling works unchanged

4 new tests (19 total): start enqueues roots, completed cascades
downstream, failure skips downstream, cancel marks all cancelled.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([799db5b](https://github.com/auser/workflow-graph/commit/799db5bd2c2446923819aeea79b3b48a481da8e5))
- Add queue crate with traits and in-memory implementations (Phase 1)

New crate: github-graph-queue with pluggable trait system:
- JobQueue: enqueue, atomic claim with lease TTL, renew, complete,
  fail with retry policy, cancel, reap expired leases, event subscribe
- ArtifactStore: put/get job outputs for downstream consumption
- LogSink: append/get log chunks with broadcast subscribe for SSE
- WorkerRegistry: register/deregister workers with labels, heartbeat

All traits designed for pg-boss (Postgres), Redis, or in-memory backends.
In-memory implementations included with 15 passing unit tests covering:
claim/complete/fail/retry, label-based routing, lease expiry reaping,
cancellation, artifact storage, log streaming, worker lifecycle.

Also scaffolds worker-sdk crate (placeholder for Phase 4).

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([6c81a30](https://github.com/auser/workflow-graph/commit/6c81a307df7b49bf4c10ff19ff7199f3eb464d18))
- Update sprint with pg-boss mapping and full implementation plan

Document how JobQueue trait maps to pg-boss operations
(SELECT FOR UPDATE SKIP LOCKED for atomic claiming, maintain()
for lease reaping). Reorganize phases with descriptions.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([5576a5e](https://github.com/auser/workflow-graph/commit/5576a5ed79383a3b6cb047986e801e617b89193c))
- Add web component library plan (002) and update sprint tracker

Covers: config API, event callbacks, pan/zoom, selection state,
programmatic control, NPM packaging, React adapter, client SDK,
accessibility, and documentation.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([1721d9d](https://github.com/auser/workflow-graph/commit/1721d9dae912847f00e4ba3e34c20b168326c03c))
- Add node click events and sprint specs for queue/worker system

- Add on_node_click JS callback to render_workflow() API
- Distinguish click from drag via 5px movement threshold
- Click fires callback with job ID; drag moves node as before
- Update running icon to glowing arc spinner (user's design)
- Add specs/SPRINT.md with full task checklist
- Add specs/plans/001-queue-worker-architecture.md with architecture plan

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([8ea7b3d](https://github.com/auser/workflow-graph/commit/8ea7b3d0213d97582d4fc02a5fa733fb9ed1dffb))
- Initial implementation of GitHub Actions workflow DAG visualizer

WASM + Axum workspace with three crates:
- shared: Job/Workflow types, YAML/JSON workflow parser
- web: Canvas-based DAG renderer with interactive drag-to-move,
  path highlighting on hover, and animated GitHub Octicon status icons
- server: Axum backend with shell command executor, DAG cascade
  orchestrator, and REST API for workflow management

Features:
- Pixel-perfect GitHub Octicon SVG icons via Canvas Path2D
- Animated orbiting dot spinner for running jobs
- Live elapsed timer counting up while jobs execute
- Automatic cascade: downstream jobs start when deps succeed,
  skip when deps fail
- Pluggable workflow definitions via YAML or JSON files
- Node drag-and-drop with boundary clamping
- Edge highlighting showing full upstream/downstream path on hover
- Auto port discovery if preferred port is taken

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>([f5c5050](https://github.com/auser/workflow-graph/commit/f5c505046a85d9b59ebad4a9c45cd6da332b3ba2))
- Initial commit([440669d](https://github.com/auser/workflow-graph/commit/440669dbbb82846994eaba7e7535bd98fcb19ecd))

