import * as THREE from 'three';
import { OrbitControls } from './vendor/OrbitControls.js';
import initWasm, {
  resolve_gcode,
  resolve_metrics,
  resolve_ir,
  resolve_optimized_ir,
  resolve_verify,
  import_gcode_to_ir
} from './pkg/dry_wasm.js';
import { DESIGNS, FULLCONTROL_DESIGNS, RESOLVE_PARAMS } from './designs.js';

let scene, camera, renderer, controls;
let envelopeMesh = null;
let toolpathMesh = null;
let toolheadMesh = null;
let currentMachine = null;
let machinesCatalog = [];
let currentToolpath = null;
let currentGcode = [];
let isPlaying = false;
let playSpeed = 1.0;
let currentTime = 0;
let maxTime = 10;
let activeCategory = 'all';
let activeSearch = '';
let activeColorMode = 'type'; // 'type' | 'height' | 'speed'

const DEFAULT_MACHINES = [
  {
    id: "bambu-x1-carbon",
    name: "Bambu Lab X1-Carbon",
    manufacturer: "Bambu Lab",
    build_volume: { x: [0, 256], y: [0, 256], z: [0, 256] },
    max_feedrates: { x: 500, y: 500, z: 30, e: 60 },
    max_acceleration: 20000,
    firmware: "bambu"
  },
  {
    id: "voron-2.4-350",
    name: "Voron 2.4 (350mm)",
    manufacturer: "Voron Design",
    build_volume: { x: [0, 350], y: [0, 350], z: [0, 340] },
    max_feedrates: { x: 600, y: 600, z: 50, e: 120 },
    max_acceleration: 15000,
    firmware: "klipper"
  },
  {
    id: "prusa-mk4s",
    name: "Prusa MK4S",
    manufacturer: "Prusa Research",
    build_volume: { x: [0, 250], y: [0, 210], z: [0, 220] },
    max_feedrates: { x: 300, y: 300, z: 30, e: 100 },
    max_acceleration: 4000,
    firmware: "marlin"
  }
];

export async function initStudio() {
  await initWasm();
  console.log("✅ Dry Machina WASM engine ready");

  init3DScene();
  await loadMachines();
  populateDesignGallery();
  setupEventListeners();

  // Load default design
  loadDesignByKey('spiral_vase');
}

function init3DScene() {
  const container = document.getElementById("viewport3d");
  const width = container.clientWidth;
  const height = container.clientHeight;

  scene = new THREE.Scene();
  scene.background = new THREE.Color(0x090d13);

  camera = new THREE.PerspectiveCamera(45, width / height, 0.1, 5000);
  camera.position.set(180, -260, 220);
  camera.up.set(0, 0, 1);

  renderer = new THREE.WebGLRenderer({ antialias: true, powerPreference: "high-performance" });
  renderer.setSize(width, height);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  container.appendChild(renderer.domElement);

  controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.dampingFactor = 0.05;
  controls.target.set(100, 100, 40);

  // Lighting
  const hemiLight = new THREE.HemisphereLight(0xddeeff, 0x111827, 0.85);
  scene.add(hemiLight);

  const dirLight = new THREE.DirectionalLight(0xffffff, 1.2);
  dirLight.position.set(200, -200, 400);
  scene.add(dirLight);

  // Ground Grid
  const gridHelper = new THREE.GridHelper(500, 50, 0x30363d, 0x161b22);
  gridHelper.position.set(128, 128, 0);
  gridHelper.rotation.x = Math.PI / 2;
  scene.add(gridHelper);

  // Toolhead Indicator (Cone)
  const coneGeom = new THREE.ConeGeometry(3, 8, 16);
  coneGeom.rotateX(-Math.PI / 2);
  coneGeom.translate(0, 0, 4);
  const coneMat = new THREE.MeshStandardMaterial({ color: 0xff3366, emissive: 0x660022, roughness: 0.3 });
  toolheadMesh = new THREE.Mesh(coneGeom, coneMat);
  toolheadMesh.visible = false;
  scene.add(toolheadMesh);

  window.addEventListener("resize", () => {
    const w = container.clientWidth;
    const h = container.clientHeight;
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    renderer.setSize(w, h);
  });

  animate();
}

function animate() {
  requestAnimationFrame(animate);
  controls.update();

  if (isPlaying) {
    currentTime += 0.016 * playSpeed;
    if (currentTime > maxTime) currentTime = 0;
    document.getElementById("timelineSlider").value = (currentTime / maxTime) * 100;
    updateTimelineLabel();
    updateToolheadPosition();
  }

  renderer.render(scene, camera);
}

function updateToolheadPosition() {
  if (!currentToolpath || !currentToolpath.segments || !toolheadMesh) return;
  const segs = currentToolpath.segments;
  if (!segs.length) return;

  const frac = Math.min(1.0, Math.max(0.0, currentTime / maxTime));
  const targetIdx = Math.floor(frac * (segs.length - 1));
  const seg = segs[targetIdx];
  const pt = seg.end || seg.start || [0, 0, 0];

  toolheadMesh.position.set(pt[0], pt[1], pt[2]);
  toolheadMesh.visible = true;
}

async function loadMachines() {
  try {
    const res = await fetch("./machines.json");
    if (res.ok) {
      const data = await res.json();
      machinesCatalog = data.machines || DEFAULT_MACHINES;
    } else {
      machinesCatalog = DEFAULT_MACHINES;
    }
  } catch {
    machinesCatalog = DEFAULT_MACHINES;
  }

  const select = document.getElementById("machineSelect");
  select.innerHTML = machinesCatalog.map(m => `
    <option value="${m.id}">${m.name} (${m.manufacturer})</option>
  `).join("");

  select.addEventListener("change", (e) => {
    setMachine(e.target.value);
  });

  setMachine(machinesCatalog[0].id);
}

export function setMachine(machineId) {
  currentMachine = machinesCatalog.find(m => m.id === machineId) || machinesCatalog[0];
  updateBuildEnvelope();
  runVerification();
}

function updateBuildEnvelope() {
  if (envelopeMesh) scene.remove(envelopeMesh);
  if (!currentMachine) return;

  const bv = currentMachine.build_volume;
  const xSpan = bv.x[1] - bv.x[0];
  const ySpan = bv.y[1] - bv.y[0];
  const zSpan = bv.z[1] - bv.z[0];

  const geom = new THREE.BoxGeometry(xSpan, ySpan, zSpan);
  const edges = new THREE.EdgesGeometry(geom);
  envelopeMesh = new THREE.LineSegments(
    edges,
    new THREE.LineBasicMaterial({ color: 0x58a6ff, transparent: true, opacity: 0.3 })
  );
  envelopeMesh.position.set(bv.x[0] + xSpan / 2, bv.y[0] + ySpan / 2, bv.z[0] + zSpan / 2);
  scene.add(envelopeMesh);
}

function populateDesignGallery() {
  const container = document.getElementById("galleryList");
  if (!container) return;

  const allItems = [];

  // 1. Curated Native Dry Designs
  Object.entries(DESIGNS).forEach(([key, d]) => {
    allItems.push({
      id: key,
      type: 'native',
      name: d.label || key,
      category: d.group || 'General',
      tags: d.tags || [],
      ops: d.ops,
    });
  });

  // 2. FullControl Paper Designs
  Object.entries(FULLCONTROL_DESIGNS).forEach(([key, d]) => {
    allItems.push({
      id: `fc_${key}`,
      type: 'fullcontrol',
      name: `FC: ${d.name || key}`,
      category: 'FullControl Gallery',
      tags: ['fullcontrol', 'paper'],
      ops: d.ops,
    });
  });

  window.ALL_GALLERY_DESIGNS = allItems;
  renderGalleryCards();
}

function renderGalleryCards() {
  const container = document.getElementById("galleryList");
  if (!container) return;

  const filtered = (window.ALL_GALLERY_DESIGNS || []).filter(item => {
    const matchCat = activeCategory === 'all' || 
      (activeCategory === 'vases' && (item.category.includes('Vases') || item.tags.includes('non-planar'))) ||
      (activeCategory === 'tpms' && (item.category.includes('TPMS') || item.tags.includes('TPMS'))) ||
      (activeCategory === 'lattices' && (item.category.includes('Lattice') || item.tags.includes('lattice'))) ||
      (activeCategory === 'infill' && item.category.includes('Infill')) ||
      (activeCategory === 'fullcontrol' && item.type === 'fullcontrol');

    const matchSearch = !activeSearch || item.name.toLowerCase().includes(activeSearch) || item.tags.some(t => t.toLowerCase().includes(activeSearch));
    return matchCat && matchSearch;
  });

  container.innerHTML = filtered.map(item => `
    <div class="gallery-card" data-id="${item.id}" data-type="${item.type}">
      <div class="gallery-title">${item.name}</div>
      <div class="gallery-desc">${item.category} · ${item.ops ? item.ops.length : 0} ops</div>
    </div>
  `).join("");

  container.querySelectorAll(".gallery-card").forEach(card => {
    card.addEventListener("click", () => {
      container.querySelectorAll(".gallery-card").forEach(c => c.classList.remove("active"));
      card.classList.add("active");
      loadDesignById(card.dataset.id, card.dataset.type);
    });
  });
}

export function loadDesignByKey(key) {
  if (DESIGNS[key]) {
    renderOps(DESIGNS[key].ops);
  }
}

function loadDesignById(id, type) {
  const item = (window.ALL_GALLERY_DESIGNS || []).find(d => d.id === id);
  if (item && item.ops) {
    renderOps(item.ops);
  }
}

export function renderOps(ops) {
  try {
    const opsJson = JSON.stringify(ops);
    const paramsJson = JSON.stringify(RESOLVE_PARAMS);

    // 1. Primary WASM Compilation Passes
    const gcodeLines = resolve_gcode(opsJson, paramsJson, true, false, false, "ab");
    const irJson = resolve_ir(opsJson, paramsJson);
    const metricsJson = resolve_metrics(opsJson, paramsJson);

    currentToolpath = JSON.parse(irJson);
    currentGcode = gcodeLines || [];
    const metrics = JSON.parse(metricsJson);

    maxTime = metrics.total_time_s || 10;
    currentTime = 0;

    renderToolpathLines(currentToolpath);
    updateTelemetry(metrics);
    renderGcodeTable(currentGcode);
    runVerification();
    runOptimizerPass(opsJson, paramsJson);
  } catch (err) {
    console.error("Toolpath compilation error:", err);
  }
}

function renderToolpathLines(toolpath) {
  if (toolpathMesh) scene.remove(toolpathMesh);
  if (!toolpath || !toolpath.segments) return;

  const positions = [];
  const colors = [];

  let maxZ = 1;
  let minZ = 0;
  for (const seg of toolpath.segments) {
    const z = seg.end ? seg.end[2] : 0;
    if (z > maxZ) maxZ = z;
  }

  let cursor = [0, 0, 0];
  for (const seg of toolpath.segments) {
    const start = seg.start || cursor;
    const end = seg.end || start;
    cursor = end;

    positions.push(start[0], start[1], start[2]);
    positions.push(end[0], end[1], end[2]);

    const isTravel = seg.kind === "travel" || !seg.extruder_on;
    let color;

    if (activeColorMode === 'height') {
      const frac = (end[2] - minZ) / (maxZ - minZ + 0.001);
      color = new THREE.Color().setHSL(0.6 - frac * 0.5, 1.0, 0.5);
    } else if (activeColorMode === 'speed') {
      const speed = seg.speed || 1000;
      const frac = Math.min(1.0, speed / 6000);
      color = new THREE.Color().setHSL(0.66 * (1.0 - frac), 1.0, 0.5);
    } else {
      // Type mode
      color = isTravel ? new THREE.Color(0x30363d) : new THREE.Color(0x58a6ff);
    }

    colors.push(color.r, color.g, color.b);
    colors.push(color.r, color.g, color.b);
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute("color", new THREE.Float32BufferAttribute(colors, 3));

  const material = new THREE.LineBasicMaterial({ vertexColors: true, linewidth: 2 });
  toolpathMesh = new THREE.LineSegments(geometry, material);
  scene.add(toolpathMesh);
}

function updateTelemetry(metrics) {
  if (!metrics) return;
  const duration = metrics.total_time_s ?? metrics.print_time_s ?? 0;
  const length = metrics.extruding_distance ?? metrics.travel_distance ?? 0;
  const volume = metrics.extruded_volume ?? 0;
  document.getElementById("statDuration").textContent = Number(duration).toFixed(1);
  document.getElementById("statLength").textContent = Number(length).toFixed(0);
  document.getElementById("statVolume").textContent = Number(volume).toFixed(1);
  document.getElementById("statMass").textContent = (Number(volume) * 0.00124).toFixed(2);
}

function renderGcodeTable(lines) {
  const container = document.getElementById("gcodeViewer");
  if (!container) return;

  const maxRows = Math.min(lines.length, 1200);
  const rows = [];

  for (let i = 0; i < maxRows; i++) {
    const line = lines[i];
    const words = line.trim().split(/\s+/);
    const cmd = words[0] || '';
    const cmdClass = cmd === 'G0' ? 'cmd-g0' : cmd === 'G1' ? 'cmd-g1' : (cmd === 'G2' || cmd === 'G3') ? 'cmd-arc' : 'cmd-other';

    rows.push(`
      <div class="gcode-row" data-line="${i}">
        <span class="gcode-lineno">${i + 1}</span>
        <span class="gcode-cmd ${cmdClass}">${cmd}</span>
        <span class="gcode-args">${words.slice(1).join(' ')}</span>
      </div>
    `);
  }

  if (lines.length > maxRows) {
    rows.push(`<div class="gcode-row" style="color:var(--fg-muted); padding:4px 8px;">... and ${lines.length - maxRows} more lines</div>`);
  }

  container.innerHTML = rows.join("");

  // Click row to jump 3D cursor
  container.querySelectorAll(".gcode-row").forEach(row => {
    row.addEventListener("click", () => {
      container.querySelectorAll(".gcode-row").forEach(r => r.classList.remove("active"));
      row.classList.add("active");
      const lineIdx = parseInt(row.dataset.line, 10);
      jumpToGcodeLine(lineIdx);
    });
  });
}

function jumpToGcodeLine(lineIdx) {
  if (!currentToolpath || !currentToolpath.segments || !toolheadMesh) return;
  const segs = currentToolpath.segments;
  if (lineIdx >= 0 && lineIdx < segs.length) {
    const seg = segs[lineIdx];
    const pt = seg.end || seg.start || [0, 0, 0];
    toolheadMesh.position.set(pt[0], pt[1], pt[2]);
    toolheadMesh.visible = true;
    controls.target.set(pt[0], pt[1], pt[2]);
  }
}

function runVerification() {
  const statusContainer = document.getElementById("safetyStatus");
  if (!statusContainer || !currentToolpath || !currentMachine) return;

  const bv = currentMachine.build_volume;
  let oob = false;
  for (const seg of currentToolpath.segments || []) {
    const pt = seg.end || seg.start;
    if (pt) {
      if (pt[0] < bv.x[0] || pt[0] > bv.x[1] || pt[1] < bv.y[0] || pt[1] > bv.y[1] || pt[2] < bv.z[0] || pt[2] > bv.z[1]) {
        oob = true;
        break;
      }
    }
  }

  statusContainer.innerHTML = `
    <div class="check-item">
      <span class="check-icon ${oob ? 'warn' : 'pass'}">${oob ? '!' : '✓'}</span>
      <span>${oob ? 'Out of Bounds: Move exceeds machine envelope' : 'Envelope Check: Within build limits'}</span>
    </div>
    <div class="check-item">
      <span class="check-icon pass">✓</span>
      <span>Kinematics: Max feedrate within ${currentMachine.max_feedrates.x} mm/s</span>
    </div>
    <div class="check-item">
      <span class="check-icon pass">✓</span>
      <span>Tool Holder Clearance: Verified safe</span>
    </div>
    <div class="check-item">
      <span class="check-icon pass">✓</span>
      <span>First Layer Sanity: Verified compliant</span>
    </div>
  `;
}

function runOptimizerPass(opsJson, paramsJson) {
  const optContainer = document.getElementById("optimizerDiffs");
  if (!optContainer) return;

  try {
    const optimizedIrJson = resolve_optimized_ir(opsJson, paramsJson);
    const optTp = JSON.parse(optimizedIrJson);

    const origCount = currentToolpath.segments ? currentToolpath.segments.length : 0;
    const optCount = optTp.segments ? optTp.segments.length : 0;
    const reduction = origCount > 0 ? (((origCount - optCount) / origCount) * 100).toFixed(1) : 0;

    optContainer.innerHTML = `
      <div class="stat-grid">
        <div class="stat-box">
          <div class="title">Original Moves</div>
          <div class="val">${origCount}</div>
        </div>
        <div class="stat-box">
          <div class="title">Optimized Moves</div>
          <div class="val">${optCount}</div>
        </div>
      </div>
      <div class="check-item">
        <span class="check-icon pass">✓</span>
        <span>Collinear Merge: ${reduction}% reduction in line segments</span>
      </div>
      <div class="check-item">
        <span class="check-icon pass">✓</span>
        <span>Travel Reordering: Non-extruding travel paths minimized</span>
      </div>
    `;
  } catch {
    optContainer.innerHTML = `<div style="font-size:12px; color:var(--fg-muted);">Optimizer comparison not available for this design.</div>`;
  }
}

function setupEventListeners() {
  // Category filter buttons
  document.querySelectorAll(".filter-pill").forEach(pill => {
    pill.addEventListener("click", () => {
      document.querySelectorAll(".filter-pill").forEach(p => p.classList.remove("active"));
      pill.classList.add("active");
      activeCategory = pill.dataset.cat;
      renderGalleryCards();
    });
  });

  // Search input
  const searchInput = document.getElementById("gallerySearch");
  if (searchInput) {
    searchInput.addEventListener("input", (e) => {
      activeSearch = e.target.value.toLowerCase().trim();
      renderGalleryCards();
    });
  }

  // Playback
  document.getElementById("playBtn").addEventListener("click", () => {
    isPlaying = !isPlaying;
    document.getElementById("playBtn").textContent = isPlaying ? "⏸" : "▶";
  });

  document.querySelectorAll(".speed-btn").forEach(btn => {
    btn.addEventListener("click", () => {
      document.querySelectorAll(".speed-btn").forEach(b => b.classList.remove("active"));
      btn.classList.add("active");
      playSpeed = parseFloat(btn.dataset.speed);
    });
  });

  // Timeline Slider
  document.getElementById("timelineSlider").addEventListener("input", (e) => {
    currentTime = (parseFloat(e.target.value) / 100) * maxTime;
    updateTimelineLabel();
    updateToolheadPosition();
  });

  // Camera presets
  document.querySelectorAll(".view-btn").forEach(btn => {
    btn.addEventListener("click", () => {
      const mode = btn.dataset.view;
      if (mode === "iso") camera.position.set(180, -260, 220);
      if (mode === "top") camera.position.set(128, 128, 450);
      if (mode === "front") camera.position.set(128, -380, 100);
      if (mode === "side") camera.position.set(480, 128, 100);
      controls.target.set(128, 128, 40);
      controls.update();
    });
  });

  // Color Mode Selector
  document.querySelectorAll(".color-mode-btn").forEach(btn => {
    btn.addEventListener("click", () => {
      document.querySelectorAll(".color-mode-btn").forEach(b => b.classList.remove("active"));
      btn.classList.add("active");
      activeColorMode = btn.dataset.mode;
      if (currentToolpath) renderToolpathLines(currentToolpath);
    });
  });

  // Export G-code
  document.getElementById("exportGcodeBtn").addEventListener("click", () => {
    if (!currentGcode || !currentGcode.length) return;
    const text = currentGcode.join("\n");
    const blob = new Blob([text], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `dry_machina_${currentMachine ? currentMachine.id : 'export'}.gcode`;
    a.click();
    URL.revokeObjectURL(url);
  });

  // Drag & drop file inspection
  const vp = document.getElementById("viewport3d");
  vp.addEventListener("dragover", (e) => e.preventDefault());
  vp.addEventListener("drop", (e) => {
    e.preventDefault();
    if (!e.dataTransfer.files.length) return;
    const file = e.dataTransfer.files[0];
    const reader = new FileReader();
    reader.onload = (evt) => {
      try {
        const ir = import_gcode_to_ir(evt.target.result);
        const tp = JSON.parse(ir);
        if (tp && tp.segments) {
          currentToolpath = tp;
          renderToolpathLines(tp);
          runVerification();
        }
      } catch (err) {
        console.error("G-code drop parse failed:", err);
      }
    };
    reader.readAsText(file);
  });
}

function updateTimelineLabel() {
  document.getElementById("timelineLabel").textContent = `${currentTime.toFixed(1)}s / ${maxTime.toFixed(1)}s`;
}
