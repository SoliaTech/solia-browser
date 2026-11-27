# Solia Browser

> A native client for discovering, loading, and running Solia applications.
>
> **Solia Browser is not a general-purpose web browser.** It does not render HTML, CSS, or JavaScript. It loads Solia source files and Solia application packages, executes them in a sandboxed runtime, and presents them through the native Solia UI and rendering systems.

- **Organization:** [SoliaTech](https://github.com/SoliaTech)
- **Repository:** `SoliaTech/solia-browser`
- **Status:** Network source MVP / pre-alpha
- **Related project:** [`Solia`](https://github.com/SoliaTech/solia) — language, compiler, type system, and WASM backend
- **Initial platforms:** Windows, macOS, Linux

---

## Run the network source MVP

Cargo fetches the pinned Solia compiler from the private SoliaTech repository using your Git credentials. From this repository run:

```sh
cargo run --locked -- https://solia.home.rethinkos.com/counter.solia
```

The desktop client fetches the source, compiles it in memory, and renders Counter beneath the address bar. Click **Increment**, or use **Reload** to fetch the current source and reset the instance. Running without arguments opens the same test URL. The test domain must be reachable from your network.

To try nested layouts, checkbox and slider bindings, open the [settings source](https://solia.home.rethinkos.com/settings.solia) in the desktop client. Use Tab to reach the controls, Space to toggle Mute, and the arrow keys to adjust Volume.

For local development with the Solia examples checked out in `../solia`, serve them with `python3 -m http.server 8080 --bind 127.0.0.1 --directory ../solia/examples` and open `http://127.0.0.1:8080/counter.solia`. Run `cargo test --locked` for loopback integration tests and `cargo test --locked -- --ignored` for the live HTTPS Counter test.

The networking policy, lifecycle and repository interfaces are described in [RFC 0001](docs/RFC-0001-network-source.md).

## 1. Vision

Solia Browser provides a browser-like way to access applications over the network, without inheriting the legacy Web platform.

A user enters or opens a URL. Solia Browser resolves the resource, obtains a Solia source file or compiled application package, verifies its metadata and permissions, and runs it in the local Solia Runtime.

The user experience borrows familiar browser concepts—address bar, tabs, history, bookmarks, reload, and sharing—but the content model is different:

- A URL points to an executable Solia application, not an HTML document.
- The application UI is described with Solia's declarative UI model.
- Application logic is compiled to WebAssembly for the first release.
- The client renders UI and game content through native runtime systems.
- Each application runs with isolated state and explicitly granted capabilities.

The long-term goal is to make publishing and opening an interactive application as simple as publishing and opening a link.

## 2. Non-goals

Solia Browser intentionally does **not** aim to support the existing Web platform.

Out of scope:

- HTML parsing and rendering
- CSS layout, cascade, and styling compatibility
- JavaScript execution
- DOM, BOM, and browser compatibility APIs
- Chromium, Blink, Gecko, WebKit, or WebView embedding
- General-purpose browsing of traditional websites
- A compatibility mode for existing web applications

This boundary is deliberate. Solia Browser should remain a focused client for the Solia application platform rather than becoming another browser engine project.

## 3. Core user experience

A user can open an application in either of two ways:

```text
https://example.com/hello.solia
https://example.com/my-app.sapp
```

The first URL serves Solia source code. The client downloads the source, checks the language and runtime compatibility, compiles it locally, and launches it.

The second URL serves a precompiled Solia application package. The client downloads and verifies the package, checks its runtime requirements and permissions, then launches it.

The initial client should provide:

- Address bar and URL navigation
- Multiple application tabs
- Back / forward navigation where meaningful
- Reload and stop controls
- Recent applications and bookmarks
- Application loading and error states
- Permission prompts and a per-application permission page
- Local package cache and offline relaunch for cached applications
- Basic developer diagnostics

A tab represents an application instance or an application navigation context, not a traditional web page.

## 4. Supported application formats

### 4.1 Solia source: `.solia`

Purpose: development, previews, prototypes, and frictionless sharing.

Proposed loading flow:

1. Fetch the source over HTTPS (allow `http://localhost` for local development).
2. Enforce response-size and redirect limits.
3. Read the declared Solia language version and required runtime API version, if present.
4. Compile locally using the Solia compiler embedded in or provisioned to the client.
5. Run static type and capability checks.
6. Display the requested permissions to the user.
7. Instantiate the application in the WASM sandbox.
8. Render through the native Solia Runtime.

Source loading makes the source code available to the client. It is not a code-protection or production-distribution mechanism.

### 4.2 Solia application package: `.sapp`

Purpose: production distribution, application stores, and CDN delivery.

A package is a versioned archive containing a manifest, compiled WASM module, assets, and optional signature metadata.

Proposed logical structure:

```text
my-app.sapp
├── manifest.toml
├── app.wasm
├── assets/
│   ├── images/
│   ├── audio/
│   └── models/
├── shaders/
└── signature
```

The exact archive format, signing format, and manifest schema are to be specified separately. The package should declare at least:

- Application ID, name, version, and publisher
- Entry module
- Solia language and runtime compatibility range
- Required capabilities and optional capabilities
- Resource limits or resource hints
- Asset metadata and integrity hashes
- Signature and signing-key identity, when signed

### 4.3 Proposed media types

These media types are provisional and must be registered/documented by the Solia project before they are treated as stable:

| Resource | Extension | Proposed media type |
|---|---|---|
| Solia source | `.solia` | `application/vnd.solia.source` |
| Solia package | `.sapp` | `application/vnd.solia.package` |

The client should use a combination of URL, response media type, and validated package metadata. It must not execute an unknown file merely because its extension looks familiar.

## 5. High-level architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                       Solia Browser                          │
│                                                             │
│  Window / Tabs / Address Bar / History / Bookmarks           │
│                         │                                   │
│                  Navigation Controller                      │
│                         │                                   │
│                 Resource Resolver                           │
│                         │                                   │
│              Source Loader / Package Loader                  │
│                    │             │                           │
│              Solia Compiler  Package Verifier                │
│                    └──────┬──────┘                           │
│                           │                                  │
│                  Application Supervisor                      │
│                           │                                  │
│                 WASM Execution Sandbox                       │
│                           │                                  │
│                 Versioned Host Interfaces                    │
│                           │                                  │
│  ┌────────────────────────┴───────────────────────────────┐  │
│  │                  Solia Native Runtime                  │  │
│  │                                                       │  │
│  │  UI / Layout / Accessibility / Input / Game / Audio   │  │
│  │  GPU / Assets / Storage / Network / Capabilities     │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 5.1 Client shell

Owns the native window, tabs, address bar, navigation history, bookmarks, downloads, application status, and user-facing settings.

### 5.2 Navigation and resource resolver

Resolves a URL to a supported Solia resource. It handles HTTPS, redirects, caching metadata, media types, and error reporting. It does not implement HTML navigation or browser compatibility behavior.

### 5.3 Source compiler

For `.solia` development links, the client invokes a compatible Solia compiler. The compiler should be versioned independently from the client shell. Compilation should run off the UI thread and be subject to resource limits.

### 5.4 Package loader and verifier

Validates the package structure, manifest, hashes, signature (when present), runtime compatibility, and declared capabilities before execution.

### 5.5 Application supervisor

Creates and destroys application instances, tracks lifecycle, manages per-app storage namespaces, enforces quotas, and coordinates permissions and updates.

### 5.6 WASM execution sandbox

The first release uses WebAssembly as the application code target. The WASM runtime is an implementation detail of the platform, not a restriction on Solia's language design. Solia should retain a typed, backend-independent IR so that Native or other backends can be considered later.

### 5.7 Native Solia Runtime

Provides the host capabilities exposed to applications:

- Declarative UI and reactive state runtime
- Layout, text, accessibility semantics, focus, and input
- 2D / 3D rendering and game integration
- Audio and animation
- Asset loading and caching
- Scoped storage
- Permission-mediated network and device APIs

The client should reuse the shared Solia Runtime rather than implement a second UI or game engine.

## 6. Runtime and application isolation

Every remotely loaded application is untrusted by default, including source code served from a familiar domain.

The initial security model should include:

- Separate WASM instance and memory for each application instance
- Per-application storage namespace
- Explicit capability grants
- No ambient filesystem or process access
- Network access denied unless declared and granted
- Domain-scoped network permissions where practical
- Explicit camera and microphone permissions
- Resource limits for memory, execution time, tasks, and asset sizes
- Cancellation and termination controls
- Package integrity verification
- Optional publisher signatures, with clear trust indicators
- No access to another application's private storage by default

WASM isolation alone is not a complete security policy. The host must expose only narrow, validated interfaces and enforce quotas around those interfaces.

## 7. Capabilities and permissions

Applications declare required and optional capabilities in their manifest. The runtime checks every privileged host call against the application's granted capabilities.

Illustrative manifest fragment:

```toml
[permissions]
required = [
  "ui.window",
  "storage.app",
]
optional = [
  "network.connect:api.example.com",
  "audio.output",
  "ai.inference",
]
```

This is illustrative syntax, not a finalized schema.

Permission UX should explain what a capability enables, why the application requests it, whether it is required, and how to revoke it. Permissions should be scoped to the application and, where possible, to a specific origin or resource.

## 8. Caching, offline use, and updates

The client should cache verified application packages and their assets using content hashes and version metadata.

Initial behavior:

- Cache only after successful validation.
- Keep application data separate from package files.
- Allow offline relaunch of a previously cached compatible package.
- Check for updates when online, without silently replacing a running application.
- Verify new package contents before activation.
- Use atomic package activation so a failed update does not corrupt the previous version.
- Provide controls to clear an application's cache and local data separately.

A later release may support delta updates and shared content-addressed asset storage.

## 9. Application identity and URLs

The initial release should use ordinary HTTPS URLs rather than inventing a new transport protocol.

An application should have a stable application ID in its manifest, independent of the URL from which it is downloaded. The URL identifies a distribution location; the application ID identifies the application and its storage/security identity.

The design should eventually specify:

- Canonical application ID format
- Publisher identity and signing keys
- Redirect and cross-origin rules
- URL-to-application identity binding
- Application deep links and internal navigation
- Version pinning and update channels

The first milestone can support direct URLs to `.solia` and `.sapp` files. A directory-style application manifest and registry discovery can be added later.

## 10. Developer experience

Solia Browser should provide a developer mode without turning into a full IDE.

Initial developer features:

- Open a local development URL such as `http://localhost:PORT/app.solia`
- Show source compilation diagnostics
- Show package validation and compatibility errors
- Inspect application logs
- Inspect granted capabilities
- Reload the application
- Display runtime and frame-time diagnostics
- Export a diagnostic report

A separate Solia CLI and editor integration should remain responsible for authoring, formatting, testing, packaging, and publishing.

## 11. Proposed technology direction

| Area | Initial direction |
|---|---|
| Client and host runtime | Rust |
| Desktop windows and input | Cross-platform Rust windowing stack; evaluate winit |
| GPU rendering | `wgpu` |
| WASM execution | Wasmtime |
| WASM component interfaces | Evaluate WIT / Component Model; keep a versioned host ABI |
| Solia compilation | Solia compiler, shared with the language project |
| UI | Solia Native UI Runtime |
| Game integration | Shared Solia game runtime; evaluate Bevy ECS integration |
| Package format | Versioned `.sapp` archive |
| Transport | HTTPS for remote resources; localhost HTTP for development |

These are candidate choices, not final dependency commitments. The client should depend on stable interfaces from the language and runtime projects rather than duplicate their implementation.

## 12. MVP scope

### Must have

1. Native desktop shell for Windows, macOS, and Linux.
2. Address bar and single-tab navigation, followed by multiple tabs.
3. HTTPS loading of direct `.solia` and `.sapp` URLs.
4. Local compilation of `.solia` source using a compatible compiler.
5. Loading and execution of compiled WASM packages.
6. Integration with the Solia Native UI Runtime.
7. Basic app lifecycle and error handling.
8. Per-app sandbox and capability checks.
9. Permission prompt and permission management.
10. Verified local package and asset cache.
11. Basic reload, history, and developer diagnostics.

### Explicitly deferred

- HTML / CSS / JavaScript support
- WebView or Chromium embedding
- General-purpose website browsing
- Built-in application store or registry service
- Social features and user accounts
- Native application plug-ins from untrusted publishers
- Advanced delta updates
- Mobile clients
- Full 3D editor and game-authoring tools

## 13. Suggested development phases

### Phase 0 — Contracts and prototypes

- Define `.sapp` manifest and package layout.
- Define the URL loading contract and provisional media types.
- Define the versioned Runtime Host ABI.
- Prototype Wasmtime integration with a minimal Solia WASM module.
- Confirm how the client shares compiler and runtime versions with `solia`.

### Phase 1 — Local application client

- Native window and basic navigation shell.
- Open local `.solia` source and `.sapp` packages.
- Compile and execute a minimal application.
- Render basic declarative UI.
- Add logs, reload, and error display.

### Phase 2 — Remote loading

- HTTPS source and package loading.
- Redirect and content-type validation.
- Permission prompts and per-app isolation.
- Package and asset cache.
- Offline relaunch of cached apps.

### Phase 3 — Multi-application experience

- Multiple tabs and application lifecycle management.
- History and bookmarks.
- Update checks and atomic package activation.
- Developer diagnostics and performance tools.

### Phase 4 — Platform ecosystem

- Publisher identity and package signing workflow.
- Application registry and discovery.
- App links and sharing.
- Optional delta updates and distribution infrastructure.

## 14. Success criteria

The first meaningful milestone is not a feature-complete browser. It is a reliable end-to-end demonstration:

1. A developer writes a small `.solia` application.
2. The developer publishes it at an HTTPS URL.
3. A user opens that URL in Solia Browser.
4. The client compiles or loads the application.
5. The user reviews and grants its requested capabilities.
6. The application runs in a sandbox and renders through native Solia UI.
7. The client caches it and can relaunch it offline.

The same application should run on Windows, macOS, and Linux without HTML, CSS, JavaScript, a browser engine, or a per-app installation process.

## 15. Open design questions

The following decisions need RFCs before implementation becomes stable:

- Should `.sapp` use ZIP, a custom container, or a content-addressed directory format?
- Should the client always embed the Solia compiler, or download signed compiler toolchains on demand?
- What is the exact relationship between application ID, publisher identity, origin, and storage namespace?
- Should WASM Core Modules or the Component Model be the first execution format?
- What is the minimum stable host ABI for UI, input, storage, network, and rendering?
- How should long-running games and background tasks be scheduled and limited?
- What are the package signing, revocation, and trust-root policies?
- How should application updates interact with currently running instances?

These questions should be resolved incrementally through versioned RFCs. The initial implementation should avoid locking the language design to a specific WASM runtime or package archive format.

---

## Relationship to other Solia projects

| Project | Responsibility |
|---|---|
| `SoliaTech/solia` | Language syntax, type system, compiler, typed IR, WASM backend |
| `SoliaTech/solia-runtime` | Native UI, rendering, game, AI, resource, and capability runtime |
| `SoliaTech/solia-browser` | Desktop client, navigation, remote loading, app lifecycle, cache, permissions |
| `SoliaTech/solia-std` | Standard library and stable application-facing APIs |
| `SoliaTech/solia-registry` | Future application publishing, discovery, and distribution service |

The client should consume the language and runtime through versioned interfaces. It should not fork or duplicate their core functionality.

## License

License not selected yet. Choose a project-wide license before accepting external contributions or publishing stable packages.
