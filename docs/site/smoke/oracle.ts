import * as sdk from '@dry/sdk';
import { compileSnippet } from '../.vitepress/theme/run-snippet';
import type { Dry } from '../.vitepress/theme/dry-engine';

const dry = sdk as unknown as Dry;

export function oracleGcode(src: string): string[] {
  const value = compileSnippet(src)(dry) as { gcode?: () => string[] };
  return typeof value?.gcode === 'function' ? value.gcode() : [];
}
