---
title: CAD Embedding
pageClass: marketing-page
---

<section class="market-hero">
  <p class="market-eyebrow">CAD connector strategy</p>
  <h1>Embedding Dry into CAD workflows</h1>
  <p class="market-lede">
    Dry should embed into CAD as the compiler, verification and report layer below host-specific geometry,
    document state and UI. The host owns design intent extraction. Dry owns IR, simulation, verification,
    reports and target output.
  </p>
  <div class="market-actions">
    <a href="#integration-patterns">Patterns</a>
    <a href="#host-prioritization">Hosts</a>
    <a href="#recommended-mvps">MVPs</a>
  </div>
</section>

## Ownership Boundary

<div class="market-grid">
  <article>
    <p class="card-label">Host CAD owns</p>
    <h3>Design context</h3>
    <ul>
      <li>geometry selection and feature context;</li>
      <li>native UI, document state and authentication;</li>
      <li>B-rep, mesh, sketch and curve extraction;</li>
      <li>workflow persistence and collaboration.</li>
    </ul>
  </article>
  <article>
    <p class="card-label">Dry owns</p>
    <h3>Compiler behavior</h3>
    <ul>
      <li>Dry IR and G-code import/export;</li>
      <li>toolpath lowering, verification and simulation;</li>
      <li>machine/material/process profiles;</li>
      <li>trace, compare, rewrite and report semantics.</li>
    </ul>
  </article>
  <article>
    <p class="card-label">Product boundary</p>
    <h3>Do not over-own CAD</h3>
    <ul>
      <li>avoid generic B-rep modeling;</li>
      <li>avoid mesh repair and full slicing as first scope;</li>
      <li>avoid native feature-tree ownership;</li>
      <li>keep connector UI thin.</li>
    </ul>
  </article>
</div>

## Integration Patterns

<div class="market-grid wide">
  <article>
    <h3>Local plugin calling Dry</h3>
    <p>Best for Fusion 360, Rhino/Grasshopper, Blender, FreeCAD and early SolidWorks prototypes.</p>
    <p><strong>Workflow:</strong> select sketch/curve/body -> convert supported intent -> call Dry CLI/SDK -> show report and G-code/IR.</p>
  </article>
  <article>
    <h3>Cloud app calling Dry service</h3>
    <p>Best for Onshape and enterprise/browser workflows.</p>
    <p><strong>Workflow:</strong> authenticate -> read document context -> run Dry cloud job -> return report/artifact link.</p>
  </article>
  <article>
    <h3>Post-processor hook</h3>
    <p>Best for low-friction pilots.</p>
    <p><strong>Workflow:</strong> host exports mesh/path/G-code -> Dry reviews, traces or rewrites -> accepted artifact is archived or uploaded.</p>
  </article>
  <article>
    <h3>Product SDK embed</h3>
    <p>Best for commercial CAD/CAM, slicer and manufacturing SaaS vendors.</p>
    <p><strong>Workflow:</strong> vendor product embeds Dry native/wasm/API service and exposes review/emit/report features.</p>
  </article>
</div>

## Host Prioritization

| Host | First motion | Why | Risk | Priority |
|---|---|---|---|---:|
| Fusion 360 | local add-in | fastest polished CAD MVP, strong maker/R&D overlap | packaging and OS support | 1 |
| Rhino/Grasshopper | components | best procedural toolpath fit | lower direct ACV | 2 |
| Onshape | cloud app + FeatureScript helper | strong cloud/platform story | auth, permissions and service complexity | 3 |
| FreeCAD | workbench/macro | open validation channel | weak revenue path | 4 |
| Blender | add-on | strong visual/demo surface | not manufacturing-native CAD | 5 |
| SOLIDWORKS | enterprise add-in/bridge | high-value industrial accounts | heavy Windows/enterprise burden | 6 |

## Host Playbooks

### Fusion 360

Autodesk's Fusion API supports scripts, add-ins and applications that customize Fusion. Dry's best first move is a
local add-in.

MVP:

1. Add command: `Compile with Dry`.
2. User selects sketches, curves, faces or a body.
3. Add-in converts supported entities to Dry design JSON or exports a supported artifact.
4. Add-in calls Dry CLI/Python binding.
5. Add-in shows metrics, verification findings and generated G-code/IR.

Commercial angle: best polished demo host for hardware teams, consultancies and advanced makers.

### Rhino / Grasshopper

Rhino/Grasshopper is the best procedural-toolpath host because users already build geometry and process logic as
graphs or scripts.

MVP:

1. Grasshopper components produce Dry path primitives or Dry IR.
2. Components call Dry to simulate, verify and emit.
3. Results render as preview curves, metrics and findings.
4. Example definitions cover custom infill, lattices and non-planar/robotic deposition.

Commercial angle: best early proof and research channel; package as support, workshops or advanced components.

### Onshape

Onshape is cloud-native, exposes REST APIs, and supports FeatureScript custom features. Dry's best first move is a
cloud integration with a small FeatureScript helper where useful.

MVP:

1. User selects a part, sketch, curve or Dry-specific custom feature.
2. Dry cloud integration reads document/element/version/workspace context through Onshape APIs.
3. Dry creates a review/compile job with machine/material profile selection.
4. Report, Dry IR and G-code artifacts are linked back to the document/app workflow.

FeatureScript role: express path primitives or manufacturing metadata close to the CAD model.

REST/API role: authentication, document context, export/import and artifact links.

Commercial angle: strongest cloud/enterprise story, but heavier product surface than Fusion or Grasshopper.

### SOLIDWORKS

SOLIDWORKS has a mature desktop API and high industrial relevance, but the delivery burden is higher.

MVP:

1. Windows add-in or bridge exports selected sketches, curves or bodies.
2. Dry runs as local CLI/service.
3. Report and generated artifacts are attached to the project/workflow before deep UI integration.

Commercial angle: high-value later target after proof from simpler hosts.

### Blender

Blender is useful for artistic and non-traditional deposition rather than classic CAD-first manufacturing.

MVP:

1. User selects curve, mesh or generated path object.
2. Add-on converts supported paths into Dry input.
3. Dry verifies, simulates and emits.
4. Blender shows preview and report metadata.

Commercial angle: demo channel for clay, concrete, food, silicone, composites and artistic FFF.

### FreeCAD

FreeCAD is a good open-source validation host because it uses Python heavily and supports workbenches/macros.

MVP:

1. Workbench selects sketches, wires or paths.
2. Workbench exports Dry-compatible path/design data.
3. Dry CLI/Python binding verifies and emits.
4. Workbench displays verification summary and generated artifacts.

Commercial angle: adoption and community proof more than immediate revenue.

## Recommended MVPs

<div class="market-grid">
  <article>
    <p class="card-label">MVP 1</p>
    <h3>Fusion "Compile with Dry"</h3>
    <p>Selected sketches/curves, one FFF profile, metrics, verification findings and G-code/IR export.</p>
    <p><strong>Exit:</strong> compile, verify and export in under 5 minutes.</p>
  </article>
  <article>
    <p class="card-label">MVP 2</p>
    <h3>Grasshopper components</h3>
    <p>Path primitives, profile selection, verify, simulate, emit and visual findings.</p>
    <p><strong>Exit:</strong> one definition generates a validated Dry toolpath without hand-written G-code.</p>
  </article>
  <article>
    <p class="card-label">MVP 3</p>
    <h3>Onshape cloud proof</h3>
    <p>Document context, FeatureScript metadata where useful, Dry cloud job and linked report/artifacts.</p>
    <p><strong>Exit:</strong> one Onshape document round-trips to Dry report and artifact links.</p>
  </article>
</div>

## Commercial Packaging

### CAD Connector Pilot

Buyer: hardware R&D team, advanced print lab or CAD/CAM integrator.

Price test: **$7,500-$25,000** for a 4-6 week connector pilot.

Deliverables:

- one host integration;
- one profile;
- one validated workflow;
- one report template;
- documented limitations and next-step backlog.

### Embed SDK for CAD/CAM Vendors

Buyer: CTO, VP Engineering or platform product owner.

Price test: **$15,000-$75,000/year** support/license, with custom OEM pricing if Dry ships inside a commercial
product.

Deliverables:

- native/wasm/CLI integration path;
- conformance fixture;
- report schema mapping;
- compatibility/versioning agreement.

## Source Notes

- [Autodesk Fusion API overview](https://aps.autodesk.com/developer/overview/autodesk-fusion-api)
- [Autodesk Fusion API documentation](https://help.autodesk.com/view/fusion360/ENU/?guid=GUID-A92A4B10-3781-4925-94C6-47DA85A4F65A)
- [Autodesk Platform Services API documentation](https://aps.autodesk.com/developer/documentation)
- [Onshape REST API introduction](https://onshape-public.github.io/docs/api-intro/)
- [Onshape FeatureScript documentation](https://cad.onshape.com/FsDoc/)
- [Onshape custom features](https://www.onshape.com/en/features/custom-features)
- [SOLIDWORKS API reference](https://help.solidworks.com/2026/english/api/sldworksapiprogguide/Welcome.htm)
- [Rhino and Grasshopper developer docs](https://developer.rhino3d.com/)
- [Blender Python API](https://docs.blender.org/api/current/index.html)
- [Blender add-on tutorial](https://docs.blender.org/manual/en/latest/advanced/scripting/addon_tutorial.html)
- [FreeCAD Python scripting tutorial](https://wiki.freecad.org/Python_scripting_tutorial)

The implementation playbook is maintained in the public repository at
`docs/marketing/cad-embedding-playbook.md`.
