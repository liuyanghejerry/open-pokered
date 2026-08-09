#!/usr/bin/env node
/**
 * Generate stub scripts for maps that don't have them
 * Usage: node scripts/generate-map-scripts.js [--all]
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SCRIPTS_DIR = path.resolve(__dirname, '../../../../../scripts');
const MAPS_DIR = path.resolve(__dirname, '../../../crates/pokered-data/maps');
const TEXT_DIR = path.resolve(__dirname, '../../../../../text');

// Get all maps that need scripts
function getMapsWithoutScripts() {
  const allMaps = fs.readdirSync(MAPS_DIR, { withFileTypes: true })
    .filter(d => d.isDirectory())
    .map(d => d.name);

  const mapsWithScripts = new Set(
    fs.readdirSync(MAPS_DIR, { withFileTypes: true })
      .filter(d => d.isDirectory())
      .map(d => d.name)
      .filter(name => fs.existsSync(path.join(MAPS_DIR, name, 'script.js')))
  );

  return allMaps.filter(name => !mapsWithScripts.has(name));
}

// Parse text from text/MapName.asm
function parseTextFile(mapName) {
  const textPath = path.join(TEXT_DIR, `${mapName}.asm`);
  if (!fs.existsSync(textPath)) return {};

  const content = fs.readFileSync(textPath, 'utf-8');
  const texts = {};
  let currentKey = null;
  let currentLines = [];

  const lines = content.split('\n');
  for (const line of lines) {
    const textMatch = line.match(/^(\w+Text(?:_\d+)?)\s*:/);
    if (textMatch) {
      if (currentKey && currentLines.length > 0) {
        texts[currentKey] = currentLines;
      }
      currentKey = textMatch[1];
      currentLines = [];
      continue;
    }

    const textContent = line.match(/^\s*text\s+"(.+)"/);
    if (textContent && currentKey) {
      currentLines.push(textContent[1].replace(/\\n/g, '\n'));
    }

    const textFar = line.match(/^\s*text_far\s+_(\w+)/);
    if (textFar && currentKey) {
      // Reference to shared text, we'll need to look it up
      currentLines.push(`[ref: ${textFar[1]}]`);
    }
  }

  if (currentKey && currentLines.length > 0) {
    texts[currentKey] = currentLines;
  }

  return texts;
}

// Parse script file for NPC/sign handlers
function parseScriptFile(mapName) {
  const scriptPath = path.join(SCRIPTS_DIR, `${mapName}.asm`);
  if (!fs.existsSync(scriptPath)) return null;

  const content = fs.readFileSync(scriptPath, 'utf-8');
  const result = {
    hasComplexLogic: false,
    textPointers: [],
    mapScripts: [],
  };

  // Extract text pointers
  const textPointersMatch = content.match(/def_text_pointers\s+([\s\S]*?)(?=\w+:|$)/);
  if (textPointersMatch) {
    const pointerMatches = textPointersMatch[1].matchAll(/dw_const\s+(\w+),\s+TEXT_(\w+)/g);
    for (const match of pointerMatches) {
      result.textPointers.push({
        handler: match[1],
        textId: match[2],
      });
    }
  }

  // Check for complex script logic
  if (content.includes('CheckEvent') || content.includes('SetEvent') ||
      content.includes('MoveSprite') || content.includes('GiveItem') ||
      content.includes('EngageTrainer')) {
    result.hasComplexLogic = true;
  }

  // Extract script pointers
  const scriptPointersMatch = content.match(/def_script_pointers\s+([\s\S]*?)(?=\w+:|$)/);
  if (scriptPointersMatch) {
    const scriptMatches = scriptPointersMatch[1].matchAll(/dw_const\s+(\w+),\s+SCRIPT_(\w+)/g);
    for (const match of scriptMatches) {
      result.mapScripts.push({
        handler: match[1],
        state: match[2],
      });
    }
  }

  return result;
}

// Generate script.js content
function generateScript(mapName, scriptInfo, mapJson) {
  const lines = [];
  lines.push(`// ${mapName}.js — ${mapName} map script`);
  lines.push('');

  // Add event constants if needed
  if (scriptInfo?.hasComplexLogic) {
    lines.push('const EVENT = {');
    lines.push('  // Add event flags as needed');
    lines.push('};');
    lines.push('');
  }

  // Add map script stubs
  if (scriptInfo?.mapScripts?.length > 0) {
    lines.push('// Map Scripts');
    lines.push('');
    for (const script of scriptInfo.mapScripts) {
      lines.push(`export async function ${toCamelCase(script.handler)}() {`);
      lines.push('  // TODO: Implement script logic');
      lines.push('}');
      lines.push('');
    }
  }

  // Add default map script
  lines.push('export async function enterMap() {');
  lines.push('  // Called when entering the map');
  lines.push('}');
  lines.push('');

  // Track generated NPC function names
  const npcFunctions = [];
  
  // Generate NPC handlers from map.json
  if (mapJson.npcs?.length > 0) {
    lines.push('// NPC Handlers');
    lines.push('');
    const usedNames = new Set();
    mapJson.npcs.forEach((npc, i) => {
      let baseName = npc.spriteName || `Npc`;
      let funcName = `talk${baseName}`;
      let counter = 1;
      while (usedNames.has(funcName)) {
        funcName = `talk${baseName}${counter}`;
        counter++;
      }
      usedNames.add(funcName);
      npcFunctions.push({ textId: npc.textId, funcName });
      lines.push(`export async function ${funcName}() {`);
      lines.push(`  await game.showText("TODO: ${npc.spriteName || 'NPC'} dialog");`);
      lines.push('}');
      lines.push('');
    });
  }

  // Track generated sign function names
  const signFunctions = [];
  
  // Generate sign handlers from map.json
  if (mapJson.signs?.length > 0) {
    lines.push('// Sign Handlers');
    lines.push('');
    mapJson.signs.forEach((sign, i) => {
      const funcName = `sign${sign.textId}`;
      signFunctions.push({ textId: sign.textId, funcName });
      lines.push(`export async function ${funcName}() {`);
      lines.push(`  await game.showText("TODO: Sign text");`);
      lines.push('}');
      lines.push('');
    });
  }

  return { content: lines.join('\n'), npcFunctions, signFunctions };
}

// Generate script_config.json content
function generateScriptConfig(mapName, scriptInfo, mapJson, npcFunctions, signFunctions) {
  const config = {
    mapScripts: ['enterMap'],
    npcs: [],
    signs: [],
    coordEvents: [],
  };

  // Add NPC bindings
  for (const { textId, funcName } of npcFunctions) {
    config.npcs.push({ id: textId, talk: funcName });
  }

  // Add sign bindings
  for (const { textId, funcName } of signFunctions) {
    config.signs.push({ id: textId, talk: funcName });
  }

  return JSON.stringify(config, null, 2);
}

function toCamelCase(str) {
  return str
    .toLowerCase()
    .split('_')
    .map((word, i) => i === 0 ? word : word.charAt(0).toUpperCase() + word.slice(1))
    .join('');
}

// Main
const generateAll = process.argv.includes('--all');
const mapsWithoutScripts = getMapsWithoutScripts();

console.log(`Found ${mapsWithoutScripts.length} maps without scripts`);

let generated = 0;
for (const mapName of mapsWithoutScripts) {
  const mapJsonPath = path.join(MAPS_DIR, mapName, 'map.json');
  if (!fs.existsSync(mapJsonPath)) continue;

  const mapJson = JSON.parse(fs.readFileSync(mapJsonPath, 'utf-8'));
  const scriptInfo = parseScriptFile(mapName);

  const { content: scriptContent, npcFunctions, signFunctions } = generateScript(mapName, scriptInfo, mapJson);
  const configContent = generateScriptConfig(mapName, scriptInfo, mapJson, npcFunctions, signFunctions);

  const scriptPath = path.join(MAPS_DIR, mapName, 'script.js');
  const configPath = path.join(MAPS_DIR, mapName, 'script_config.json');

  if (generateAll) {
    fs.writeFileSync(scriptPath, scriptContent + '\n');
    fs.writeFileSync(configPath, configContent + '\n');
    generated++;
    console.log(`Generated: ${mapName}`);
  } else {
    console.log(`Would generate: ${mapName}`);
  }
}

console.log(`\n${generateAll ? 'Generated' : 'Would generate'} ${generated} scripts`);
console.log('Run with --all to actually generate files');