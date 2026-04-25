#!/usr/bin/env node
// plumb-tailwind loader.
//
// Reads a Tailwind config file path from argv and writes the
// resolved `theme` object as JSON to stdout. Diagnostic messages go to
// stderr — never stdout, since stdout is parsed as JSON by the caller.
//
// Behaviour:
//   1. If the config path ends in `.ts` or `.mts` / `.cts`, attempt to
//      register a TypeScript loader (`tsx/cjs`, then `ts-node/register`,
//      then `esbuild-register`). If none are available, exit with
//      structured error code "TS_LOADER_MISSING".
//   2. Try to `require` `tailwindcss/resolveConfig` from the config
//      file's directory (so the user project's installed version wins).
//      If it succeeds, run it on the loaded user config and emit
//      `result.theme`.
//   3. Fall back to a small in-process resolver that merges
//      `theme.extend` into `theme` for the keys we care about. This
//      preserves the contract for projects that haven't `npm install`ed
//      Tailwind, and is the path exercised by Plumb's own tests.

'use strict';

const fs = require('node:fs');
const path = require('node:path');

const KEYS = [
  'colors',
  'spacing',
  'fontSize',
  'fontWeight',
  'fontFamily',
  'borderRadius',
];

function emitError(code, message) {
  // Write a single-line JSON object so the Rust side can pattern-match
  // on `code` without parsing free-form text.
  const payload = JSON.stringify({ plumbTailwindError: { code, message } });
  process.stdout.write(payload + '\n');
  process.exit(2);
}

function tryRegisterTsLoader(configDir) {
  const candidates = ['tsx/cjs', 'ts-node/register/transpile-only', 'esbuild-register'];
  for (const candidate of candidates) {
    try {
      // Resolve from the user project so we pick up its own dev deps.
      const resolved = require.resolve(candidate, { paths: [configDir] });
      // eslint-disable-next-line import/no-dynamic-require, global-require
      require(resolved);
      return candidate;
    } catch (_err) {
      // try the next candidate
    }
  }
  return null;
}

function loadUserConfig(configPath) {
  const ext = path.extname(configPath).toLowerCase();
  const isTs = ext === '.ts' || ext === '.mts' || ext === '.cts';
  if (isTs) {
    const loader = tryRegisterTsLoader(path.dirname(configPath));
    if (!loader) {
      emitError(
        'TS_LOADER_MISSING',
        'No TypeScript loader found. Install one of `tsx`, `ts-node`, or `esbuild-register` in your project, or convert the config to .js/.mjs/.cjs.',
      );
    }
  }
  let mod;
  try {
    // eslint-disable-next-line import/no-dynamic-require, global-require
    mod = require(configPath);
  } catch (err) {
    emitError('CONFIG_REQUIRE_FAILED', `Failed to load Tailwind config: ${err && err.message ? err.message : String(err)}`);
  }
  // ESM default-export interop.
  if (mod && typeof mod === 'object' && 'default' in mod && Object.keys(mod).length === 1) {
    return mod.default;
  }
  return mod;
}

function tryResolveWithTailwind(userConfig, configDir) {
  try {
    const resolved = require.resolve('tailwindcss/resolveConfig', {
      paths: [configDir],
    });
    // eslint-disable-next-line import/no-dynamic-require, global-require
    const resolveConfig = require(resolved);
    const result = resolveConfig(userConfig);
    return result && result.theme ? result.theme : null;
  } catch (_err) {
    return null;
  }
}

function fallbackResolveTheme(userConfig) {
  // Minimal Tailwind-shaped resolver. Sufficient for tests and for
  // projects that author tokens directly in their config without
  // depending on Tailwind's own preset machinery.
  const theme = (userConfig && userConfig.theme) || {};
  const extend = theme.extend || {};
  const out = {};
  for (const key of KEYS) {
    const base = theme[key];
    const ext = extend[key];
    if (base === undefined && ext === undefined) continue;
    if (typeof base !== 'object' || base === null) {
      out[key] = ext === undefined ? base : ext;
      continue;
    }
    out[key] = Object.assign({}, base, ext || {});
  }
  return out;
}

function pickTheme(theme) {
  // Restrict the emitted JSON to the keys Plumb merges. This keeps the
  // payload small (matters for cache files) and avoids leaking arbitrary
  // user-defined theme keys.
  const out = {};
  for (const key of KEYS) {
    if (theme && Object.prototype.hasOwnProperty.call(theme, key)) {
      out[key] = theme[key];
    }
  }
  return out;
}

function main() {
  // When invoked as `node -e <script> -- <path>`, Node strips both `-e`
  // and the `--` separator, so the first script argument is `argv[1]`.
  // When invoked as `node script.js <path>`, the path also lands in
  // `argv[1]`. Skip the `--` separator if a caller forwarded it.
  let configPath = process.argv[1];
  if (configPath === '--') {
    configPath = process.argv[2];
  }
  if (!configPath) {
    emitError('USAGE', 'Usage: loader.js <tailwind-config-path>');
  }
  if (!fs.existsSync(configPath)) {
    emitError('CONFIG_NOT_FOUND', `Tailwind config not found: ${configPath}`);
  }
  const absolute = path.resolve(configPath);
  const configDir = path.dirname(absolute);
  const userConfig = loadUserConfig(absolute);
  if (!userConfig || typeof userConfig !== 'object') {
    emitError('CONFIG_NOT_OBJECT', 'Tailwind config did not export an object.');
  }
  const resolved = tryResolveWithTailwind(userConfig, configDir);
  const theme = resolved !== null ? resolved : fallbackResolveTheme(userConfig);
  const picked = pickTheme(theme);
  process.stdout.write(JSON.stringify(picked));
  process.stdout.write('\n');
}

main();
