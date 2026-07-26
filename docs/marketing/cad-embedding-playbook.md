# CAD Embedding Playbook: Dry

Research date: 2026-07-02

## Thesis

Dry can be embedded into CAD and CAD-adjacent tools as a **toolpath compiler and verification layer**, not as a
replacement for each CAD host's geometry kernel, sketch system, UI framework or document model.

The host should own:

- geometry selection and feature context;
- native UI, document state and authentication;
- B-rep, mesh, sketch and curve extraction;
- user workflow and persistence.

Dry should own:

- conversion into Dry IR or G-code import;
- toolpath lowering, verification, simulation and reports;
- profile-aware constraints;
- target G-code emission or review;
- conformance and deterministic behavior across CLI, SDK and wasm surfaces.

## Integration Patterns

### 1. Local plugin calling Dry CLI or SDK

Best for: Fusion 360, Rhino/Grasshopper, Blender, FreeCAD, early SolidWorks prototypes.

Workflow:

```text
selected sketch / curve / face / body
  -> host plugin extracts geometry intent
  -> plugin creates Dry design input or intermediate JSON
  -> dry verify / simulate / emit
  -> host shows preview + report + exported G-code
```

Why it is attractive:

- fastest MVP path;
- uses the existing Dry CLI/Python/TypeScript surfaces;
- avoids cloud auth complexity;
- good fit for researchers, makers, and early integrator pilots.

Main risk:

- packaging and installation differ per CAD host.

### 2. Cloud app calling Dry service

Best for: Onshape and enterprise/browser workflows.

Workflow:

```text
Onshape document / cloud CAD data
  -> public-facing app/API integration (authenticated requests)
  -> Dry service compiles or reviews selected geometry/toolpaths
  -> report + artifact linked back to the document/workflow
```

Why it is attractive:

- maps to cloud-native CAD workflows;
- preserves authentication and document-permission checks;
- centralizes Dry versioning and profiles;
- easier for teams that want a shared compliance/report layer.

Main risk:

- requires auth, document permissions, server-side state, and stronger support expectations.

### 3. Post-processor or export hook

Best for: low-friction pilots and existing CAD/CAM shops.

Workflow:

```text
host exports mesh/path/G-code
  -> Dry import/review/verify/trace/rewrite
  -> accepted artifact is archived, uploaded or returned to host
```

Why it is attractive:

- minimal CAD API dependency;
- can prove value before deep geometry integration;
- fits current Dry post-slicer review strengths.

Main risk:

- loses upstream CAD intent; harder to create rich authored toolpaths.

### 4. Product SDK embed

Best for: commercial CAD/CAM, slicer, print-management and manufacturing SaaS vendors.

Workflow:

```text
vendor product
  -> embeds Dry native/wasm/API service
  -> exposes review, compare, trace, report and emit as product features
```

Why it is attractive:

- higher ACV;
- turns Dry into infrastructure;
- does not require Dry to own end-user UI.

Main risk:

- requires stable API, versioning and support contract.

## Host-by-Host Analysis

### Onshape

Official surface:

- Onshape is cloud-native and exposes REST APIs for client/system integration.
- FeatureScript supports custom features and reusable company/community feature libraries.
- Onshape's integration positioning explicitly includes API integration, FeatureScript and third-party connectivity.

Best Dry integration:

- **Cloud app + optional FeatureScript helper.**

MVP:

1. User selects a part, sketch, curve or configured custom feature.
2. Dry cloud integration reads relevant document/element/version/workspace context through Onshape APIs.
3. Dry creates a review/compile job with machine/material profile selection.
4. User receives report, Dry IR and G-code artifact linked back to the document or app workflow.

FeatureScript role:

- define path primitives or manufacturability metadata inside Onshape;
- expose custom feature parameters that Dry can interpret;
- keep design intent close to the CAD model.

REST/API role:

- authenticate, read document structure, export geometry/data where allowed, and manage artifacts/reports.

Commercial angle:

- strongest for cloud-first engineering teams and platform integrators.
- likely sold as Embed SDK / Dry Cloud integration, not as a local desktop plugin.

Risks:

- server-side auth and permissions add product complexity;
- extracting exact manufacturing intent from generic CAD geometry remains non-trivial;
- FeatureScript is powerful but should not contain Dry's compiler logic.

### Autodesk Fusion 360

Official surface:

- Autodesk describes the Fusion API as a way to build scripts, add-ins and applications that customize Fusion.
- Autodesk Platform Services exposes cloud APIs for design/make data and automation.

Best Dry integration:

- **Local Fusion add-in first; APS/data integration later.**

MVP:

1. Fusion add-in command: `Compile with Dry`.
2. User selects sketch curves, construction geometry, faces or a body.
3. Add-in converts supported entities to Dry path/design JSON or exports a mesh/path artifact.
4. Add-in calls Dry CLI/Python binding locally.
5. Add-in shows metrics, verification findings and output path to G-code/IR.

Why Fusion first can work:

- approachable scripting/add-in surface;
- strong overlap with maker/prototyping and small manufacturing users;
- local plugin can use existing Dry binaries without needing a cloud product first.

Commercial angle:

- good demo host for "CAD-connected toolpath compiler";
- useful for paid pilots with hardware teams and consultancies.

Risks:

- Fusion's native CAM already covers conventional manufacturing paths;
- Dry must focus on custom/algorithmic FFF, non-planar, experimental deposition, and verification gaps;
- add-in packaging and OS differences need support work.

### SOLIDWORKS

Official surface:

- SOLIDWORKS publishes a desktop API for automating and customizing the application.

Best Dry integration:

- **Later-stage enterprise add-in or external bridge.**

MVP:

1. Windows add-in or external automation tool exports selected sketches/curves/bodies.
2. Dry runs as local CLI/service to compile/review selected artifacts.
3. Results are attached as generated files/reports rather than deeply embedded UI at first.

Commercial angle:

- valuable for industrial users and enterprise workflows;
- stronger buyer potential but heavier delivery cost.

Risks:

- Windows/.NET packaging and enterprise IT support burden;
- CAD data complexity and customer expectations are higher;
- should follow proof from Fusion/Onshape rather than lead GTM.

### Rhino / Grasshopper

Official surface:

- McNeel provides official developer resources for Rhino and Grasshopper; Rhino developer tools are royalty-free.
- Rhino/Grasshopper is friendly to Python, C# and procedural geometry workflows.

Best Dry integration:

- **Grasshopper components + optional Rhino command plugin.**

MVP:

1. Grasshopper components produce Dry path primitives or Dry IR.
2. Component calls Dry SDK/CLI to simulate, verify and emit.
3. Results render back into Grasshopper/Rhino as preview curves, metrics and findings.

Why this is high-fit:

- Grasshopper users already think in node graphs and procedural geometry;
- strong fit for custom infill, lattice, non-planar paths and robotic extrusion experiments;
- easier to position as research/prototyping package.

Commercial angle:

- best early community/proof channel for custom toolpath authoring.
- can generate examples and demos that support higher-value platform sales.

Risks:

- research/prototyping users may have lower recurring willingness to pay;
- many users expect hackable/open tooling;
- package as support/workshops or advanced components, not only SaaS subscription.

### Blender

Official surface:

- Blender exposes a Python API and supports Python add-ons.

Best Dry integration:

- **Experimental add-on for artistic/robotic deposition and mesh-to-path workflows.**

MVP:

1. User selects mesh, curve or generated path object.
2. Blender add-on converts supported curves/paths into Dry input.
3. Dry verifies, simulates and emits G-code/IR.
4. Blender shows path preview and report metadata.

Commercial angle:

- strong demo surface for non-traditional deposition: clay, concrete, food, silicone, composites, artistic FFF.
- not first enterprise GTM target.

Risks:

- Blender meshes are not the same as manufacturing intent;
- many workflows would require custom slicing/path extraction before Dry adds value;
- better as experimental proof than core revenue driver.

### FreeCAD

Official surface:

- FreeCAD makes extensive use of Python; modules add workbenches, commands and object types.

Best Dry integration:

- **Open-source workbench or macro-driven prototype.**

MVP:

1. FreeCAD workbench lets user select sketches, wires or paths.
2. Workbench exports Dry-compatible path/design data.
3. Dry CLI/Python binding verifies and emits artifacts.
4. Workbench displays verification summary and generated files.

Commercial angle:

- good open-source validation path and early adopter channel;
- useful for users who need an inspectable toolchain.

Risks:

- weaker direct revenue channel than Fusion/Onshape/SOLIDWORKS;
- distribution/support expectations vary;
- use primarily for adoption, demos and community validation.

## Prioritization

| Host | First motion | Why | Risk | Priority |
|---|---|---|---|---:|
| Fusion 360 | local add-in | fastest polished CAD MVP, good maker/R&D overlap | packaging/support | 1 |
| Rhino/Grasshopper | components | best procedural toolpath fit | lower direct ACV | 2 |
| Onshape | cloud app + FeatureScript helper | strong cloud/platform story | auth/API/product complexity | 3 |
| FreeCAD | workbench/macro | open validation channel | weak revenue | 4 |
| Blender | add-on | strong visual/demo surface | not CAD/manufacturing-native | 5 |
| SOLIDWORKS | enterprise add-in/bridge | high-value industrial accounts | heavy Windows/enterprise burden | 6 |

## Recommended MVPs

### MVP 1: Fusion 360 "Compile with Dry" add-in

Scope:

- selected sketches/curves only;
- one FFF profile;
- preview metrics + verification findings;
- export Dry IR and G-code;
- local Dry CLI invocation.

Why:

- quickest host-integrated demo that looks like a real product.

Exit criteria:

- one user can select a sketch, compile, verify, inspect findings and export G-code in under 5 minutes.

### MVP 2: Grasshopper Dry components

Scope:

- components for path primitives, profile selection, verify, simulate, emit;
- visual preview of findings;
- generated examples for lattice/non-planar/custom infill.

Why:

- best match for algorithmic toolpath authors and researchers.

Exit criteria:

- one Grasshopper definition generates a validated Dry toolpath without hand-written G-code.

### MVP 3: Onshape cloud proof

Scope:

- app reads document context and selected parameters;
- FeatureScript helper defines Dry path intent or metadata;
- Dry cloud job returns report and artifact links.

Why:

- validates enterprise/cloud integration story.

Exit criteria:

- one Onshape document round-trips to a Dry report and artifact without local desktop tooling.

## Commercial Packaging

### CAD Connector Pilot

Buyer: hardware R&D team, advanced print lab, CAD/CAM integrator.

Price test:

- $7,500-$25,000 for a 4-6 week connector pilot.

Deliverables:

- one host integration;
- one profile;
- one validated workflow;
- one report template;
- documented limitations and next-step backlog.

### Embed SDK for CAD/CAM Vendors

Buyer: CTO, VP Engineering, platform product owner.

Price test:

- $15,000-$75,000/year support/license;
- custom OEM pricing if Dry ships inside a commercial product.

Deliverables:

- native/wasm/CLI integration path;
- conformance fixture;
- report schema mapping;
- compatibility/versioning agreement.

## Product Boundaries

Dry should not own:

- generic B-rep modeling;
- mesh repair;
- support generation;
- native CAD feature trees;
- host-specific document/version control;
- UI state beyond the connector.

Dry should own:

- explicit path/design intent once extracted;
- verification contracts;
- profile-aware machine/material/process checks;
- trace, compare and rewrite semantics;
- reproducible reports and artifacts.

## Source Notes

- Autodesk: [Fusion API overview](https://aps.autodesk.com/developer/overview/autodesk-fusion-api) and [Fusion API documentation](https://help.autodesk.com/view/fusion360/ENU/?guid=GUID-A92A4B10-3781-4925-94C6-47DA85A4F65A).
- Autodesk Platform Services: [API and SDK documentation](https://aps.autodesk.com/developer/documentation) and [Data Management API](https://aps.autodesk.com/developer/overview/data-management-api).
- Onshape: [REST API introduction](https://onshape-public.github.io/docs/api-intro/), [FeatureScript documentation](https://cad.onshape.com/FsDoc/), and [custom features](https://www.onshape.com/en/features/custom-features).
- SOLIDWORKS: [API online reference](https://help.solidworks.com/2026/english/api/sldworksapiprogguide/Welcome.htm).
- Rhino/Grasshopper: [official developer documentation](https://developer.rhino3d.com/).
- Blender: [Python API documentation](https://docs.blender.org/api/current/index.html) and [add-on tutorial](https://docs.blender.org/manual/en/latest/advanced/scripting/addon_tutorial.html).
- FreeCAD: [Python scripting tutorial](https://wiki.freecad.org/Python_scripting_tutorial).
