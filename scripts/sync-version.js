#!/usr/bin/env node
// Sync version from git tag (e.g. v0.5.0) into tauri.conf.json and Cargo.toml
// Usage: npm run version [-- <version>]
//   - With arg: set version to the given string (strips leading 'v')
//   - Without arg: read from latest git tag

const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');

function getVersion() {
    const arg = process.argv[2];
    if (arg) return arg.replace(/^v/, '');
    try {
        const tag = require('child_process').execSync('git describe --tags --abbrev=0', { encoding: 'utf8' }).trim();
        return tag.replace(/^v/, '');
    } catch {
        console.error('No git tag found. Pass version as argument: npm run version -- 0.5.0');
        process.exit(1);
    }
}

const version = getVersion();
console.log(`Syncing version: ${version}`);

// Update tauri.conf.json
const confPath = path.join(root, 'src-tauri', 'tauri.conf.json');
const conf = JSON.parse(fs.readFileSync(confPath, 'utf8'));
conf.version = version;
fs.writeFileSync(confPath, JSON.stringify(conf, null, 2) + '\n');
console.log(`  Updated ${confPath}`);

// Update Cargo.toml
const cargoPath = path.join(root, 'src-tauri', 'Cargo.toml');
let cargo = fs.readFileSync(cargoPath, 'utf8');
cargo = cargo.replace(/^version\s*=\s*"[^"]*"/m, `version = "${version}"`);
fs.writeFileSync(cargoPath, cargo);
console.log(`  Updated ${cargoPath}`);

console.log('Done.');
