# ADR-014: Static HTML Graph Visualization Export

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 3 Implementation

## Context
Operators and systems engineers diagnosing complex multi-hop wake chains and block I/O bottlenecks benefit from visual inspection of the causal graph. However, traditional visualization tools introduce web server runtimes (e.g. Node.js, Express, Python Flask), external CDN dependencies (e.g. Google Fonts, Tailwind CDN, D3.js), or bloated JavaScript frameworks, violating Katana's single-binary, bounded-overhead, air-gapped production constraints.

## Alternatives Considered
1. **Interactive Web Server (`katana serve`):** Requires running an HTTP listener, daemon management, socket permissions, and attack-surface overhead on production hosts.
2. **External Graphviz / DOT export (`--dot`):** Requires user to install `dot` or upload sensitive system trace graphs to external web tools.
3. **Heavy JS visualization library (D3.js / Cytoscape):** Depends on external CDNs or bundling megabytes of minified JavaScript.

## Decision
Implement a pure, self-contained static HTML exporter (`katana::html_export::render_html_report`).
- **Monochrome Aesthetic:** Complete black and white only (obsidian `#050505` background, hairline `#1e1e1e` borders, crisp `#ffffff` text, pure monochrome palette).
- **Anti-Emoji Requirement:** Strictly zero emojis across the entire document.
- **Zero External Dependencies:** Self-contained inline CSS and scalable vector graphics (SVG) for the causal graph topology. Renders identically offline and in air-gapped data centers.
- **CLI Flag:** `katana explain <PID> --html [path]` and `katana watch <PID> --html [path]`.

## Consequences
- Fast single-file reports easily shareable as post-incident artifacts or attached to ticketing systems.
- Zero network activity or runtime dependencies.
