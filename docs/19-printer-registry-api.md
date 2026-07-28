# Printer Registry API

Dry's hosted printer capability graph is maintained as the independently licensed public
[`dry-printer-registry`](https://github.com/dmytro-yemelianov/dry-printer-registry) repository. Dry
consumes its GraphQL API through the CLI; the registry repository owns the schemas, packs, Worker,
and standalone TypeScript client.

## Data flow

```text
versioned printer capability pack
  -> JSON Schema and Dry profile validation
  -> D1 query index + R2 immutable artifacts
  -> read-only GraphQL API
  -> Dry CLI, SDKs and website explorer
```

D1 stores fields and relationships that users filter or traverse. R2 stores canonical manifests,
capability documents, resolved `dry-profile-v1` files, macro sources and proofs. A graph response always
identifies the exact pack version and artifact hash.

The v1 capability graph includes:

- printer identity, version and trust/support status;
- firmware and machine limits;
- installed hardware and physical drivers;
- materials and filament products;
- process presets and slicer mappings;
- macro definitions, implementations and printer bindings;
- calibration observations and compatibility claims;
- proofs and provenance.

`dry-profile-v1` remains the flattened runtime contract consumed by review, verify, rewrite and upload.

## Query surface

The production endpoint is
[`https://api.dry.yemelianov.dev/graphql`](https://api.dry.yemelianov.dev/graphql); its schema is
available at
[`/schema.graphql`](https://api.dry.yemelianov.dev/schema.graphql). GraphQL requests use `POST` with
the standard `{ "query": "...", "variables": { ... } }` JSON body.

```graphql
query {
  printers(
    where: {
      firmware: [KLIPPER]
      kinematics: [COREXY]
      minimumBuildVolume: { xMm: 300, yMm: 300, zMm: 250 }
      material: ["ABS"]
      nozzleDiameterMm: 0.4
      providesMacros: ["dry:macro/print-start"]
      hardwareCategories: ["stepper-driver"]
    }
  ) {
    totalCount
    nodes {
      id
      name
      versions {
        version
        packSha256
        capabilities {
          machine { kinematics maxAccelerationMmS2 }
          hardware { role component { category manufacturer model } }
          materials { family filaments { manufacturer name diameterMm } }
          macroBindings { configuredName definition { id purpose } }
        }
        profiles(materialId: "dry:material/abs", nozzleDiameterMm: 0.4) {
          id
          profileUrl
          sha256
        }
      }
    }
  }
}
```

Queries are paginated and bounded by document length, field count and depth. The public schema has no
mutations. Pack publishing remains a reviewed operator/CI action.

## Artifacts

The Worker exposes immutable R2-backed downloads:

```text
GET /v1/packs/{printer-id}/{version}
GET /v1/profiles/{printer-id}/{version}/{profile-id}
GET /v1/objects/{encoded-object-key}
```

Printer identifiers and versions are explicit. Production and CI consumers should pin a version and
verify the returned SHA-256 rather than relying on an unversioned latest value.

## CLI

The CLI searches the graph, inspects the complete versioned printer document, and resolves a
`dry-profile-v1` for existing review, verify, rewrite, and upload commands:

```sh
dry printer search voron \
  --firmware klipper --kinematics corexy \
  --material ABS --nozzle 0.4 \
  --build-x 300 --build-y 300 --build-z 250

dry printer inspect voron-2.4-350-klipper --version 0.1.0

dry printer resolve voron-2.4-350-klipper \
  --version 0.1.0 \
  --material dry:material/abs \
  --nozzle 0.4 \
  --out voron-abs-0.4.json
```

`resolve` verifies the artifact SHA-256 before writing it. `--source` can override the default
production origin with a local or private compatible registry.

## TypeScript

The public registry repository provides the dependency-free
`@dry/printer-registry-client` package:

```ts
import { PrinterRegistry } from '@dry/printer-registry-client';

const registry = new PrinterRegistry();
const matches = await registry.search({
  firmware: ['KLIPPER'],
  kinematics: ['COREXY'],
  material: ['ABS'],
  nozzleDiameterMm: 0.4,
});

const resolved = await registry.resolveProfile(matches.nodes[0].id, {
  version: '0.1.0',
  materialId: 'dry:material/abs',
  nozzleDiameterMm: 0.4,
});
if (!resolved) throw new Error('No compatible profile');

const profile = await registry.downloadProfile(resolved); // SHA-256 verified
```

## Development and deployment

See the registry repository's
[`service/README.md`](https://github.com/dmytro-yemelianov/dry-printer-registry/blob/main/service/README.md)
for local migrations, example publishing, tests, and production deployment. Its initial Voron pack
is illustrative schema/API data, not a production guarantee for a physical printer.
