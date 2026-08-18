import { crypto } from 'node:crypto';

export interface MachineFilter {
  vendor?: string;
  category?: string;
  firmware?: string;
  kinematics?: string;
  min_volume_x?: number;
  min_volume_y?: number;
  min_volume_z?: number;
  limit?: number;
}

export interface MachineRecord {
  id: string;
  name: string;
  vendor: string;
  category: string;
  firmware_flavor: string;
  kinematics_type: string;
  bounds_min_x: number;
  bounds_max_x: number;
  bounds_min_y: number;
  bounds_max_y: number;
  bounds_min_z: number;
  bounds_max_z: number;
  max_feedrate: number;
  max_accel?: number;
  is_official: number;
  profile_json: string;
  sha256_hash: string;
  created_at: string;
  updated_at: string;
}

/**
 * Search the machine catalog with flexible filters.
 */
export async function searchMachines(db: D1Database, filter: MachineFilter): Promise<MachineRecord[]> {
  let query = 'SELECT * FROM machine_catalog WHERE 1=1';
  const params: any[] = [];

  if (filter.vendor) {
    query += ' AND vendor LIKE ?';
    params.push(`%${filter.vendor}%`);
  }
  if (filter.category) {
    query += ' AND category = ?';
    params.push(filter.category);
  }
  if (filter.firmware) {
    query += ' AND firmware_flavor = ?';
    params.push(filter.firmware);
  }
  if (filter.kinematics) {
    query += ' AND kinematics_type = ?';
    params.push(filter.kinematics);
  }
  if (filter.min_volume_x !== undefined) {
    query += ' AND (bounds_max_x - bounds_min_x) >= ?';
    params.push(filter.min_volume_x);
  }
  if (filter.min_volume_y !== undefined) {
    query += ' AND (bounds_max_y - bounds_min_y) >= ?';
    params.push(filter.min_volume_y);
  }
  if (filter.min_volume_z !== undefined) {
    query += ' AND (bounds_max_z - bounds_min_z) >= ?';
    params.push(filter.min_volume_z);
  }

  query += ' ORDER BY is_official DESC, name ASC LIMIT ?';
  params.push(filter.limit || 50);

  const stmt = db.prepare(query).bind(...params);
  const { results } = await stmt.all<MachineRecord>();
  return results || [];
}

/**
 * Retrieve a specific machine record by its unique ID.
 */
export async function getMachineById(db: D1Database, id: string): Promise<MachineRecord | null> {
  const stmt = db.prepare('SELECT * FROM machine_catalog WHERE id = ?').bind(id);
  const result = await stmt.first<MachineRecord>();
  return result || null;
}

/**
 * Register or update a machine profile in the catalog.
 */
export async function registerMachine(
  db: D1Database,
  profile: Record<string, any>,
  isOfficial = false
): Promise<MachineRecord> {
  const id = profile.id || profile.name.toLowerCase().replace(/[^a-z0-9]+/g, '-');
  const name = profile.name || id;
  const vendor = profile.vendor || 'Custom';
  const category = profile.category || '3d_printer';
  const firmware_flavor = profile.firmware?.flavor || 'klipper';
  const kinematics_type = profile.kinematics?.type || 'cartesian';

  const bounds = profile.envelope?.bounds || profile.machine?.bounds || [0, 200, 0, 200, 0, 200];
  const max_feedrate = profile.kinematics?.max_feedrate_mm_min?.x || profile.machine?.max_feedrate_mm_s * 60 || 12000;
  const max_accel = profile.kinematics?.max_acceleration_mm_s2?.x || profile.machine?.max_acceleration_mm_s2 || 3000;

  const profile_json = JSON.stringify(profile);
  
  // Calculate SHA256 integrity hash
  const encoder = new TextEncoder();
  const data = encoder.encode(profile_json);
  const hashBuffer = await crypto.subtle.digest('SHA-256', data);
  const sha256_hash = Array.from(new Uint8Array(hashBuffer))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');

  const query = `
    INSERT INTO machine_catalog (
      id, name, vendor, category, firmware_flavor, kinematics_type,
      bounds_min_x, bounds_max_x, bounds_min_y, bounds_max_y, bounds_min_z, bounds_max_z,
      max_feedrate, max_accel, is_official, profile_json, sha256_hash, updated_at
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
    ON CONFLICT(id) DO UPDATE SET
      name=excluded.name,
      vendor=excluded.vendor,
      category=excluded.category,
      firmware_flavor=excluded.firmware_flavor,
      kinematics_type=excluded.kinematics_type,
      bounds_min_x=excluded.bounds_min_x,
      bounds_max_x=excluded.bounds_max_x,
      bounds_min_y=excluded.bounds_min_y,
      bounds_max_y=excluded.bounds_max_y,
      bounds_min_z=excluded.bounds_min_z,
      bounds_max_z=excluded.bounds_max_z,
      max_feedrate=excluded.max_feedrate,
      max_accel=excluded.max_accel,
      profile_json=excluded.profile_json,
      sha256_hash=excluded.sha256_hash,
      updated_at=datetime('now')
  `;

  await db
    .prepare(query)
    .bind(
      id,
      name,
      vendor,
      category,
      firmware_flavor,
      kinematics_type,
      bounds[0],
      bounds[1],
      bounds[2],
      bounds[3],
      bounds[4],
      bounds[5],
      max_feedrate,
      max_accel,
      isOfficial ? 1 : 0,
      profile_json,
      sha256_hash
    )
    .run();

  const registered = await getMachineById(db, id);
  return registered!;
}
