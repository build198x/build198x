// Run each line of cases.bas on the genuine 48K ROM and record what the ROM
// does with it, as results.tsv. See README.md.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROM_SHA1 = '5ea7c2b824672e914525d1d5c419d71b84a426a2';
const PACKAGE_VERSION = '0.4.0';
const here = path.dirname(fileURLToPath(import.meta.url));

const pkg = process.env.EMU198X_ZX_SPECTRUM_PKG;
const romPath = process.env.SPECTRUM_48K_ROM;
const tokeniser = process.env.TOKENISE_LISTING;
if (!pkg || !romPath || !tokeniser) {
  throw new Error('set EMU198X_ZX_SPECTRUM_PKG, SPECTRUM_48K_ROM and TOKENISE_LISTING');
}
const rom = fs.readFileSync(romPath);
const sha1 = createHash('sha1').update(rom).digest('hex');
if (sha1 !== ROM_SHA1) throw new Error(`${romPath} has SHA1 ${sha1}; the capture needs ${ROM_SHA1}`);
const version = JSON.parse(fs.readFileSync(path.join(pkg, 'package.json'), 'utf8')).version;
if (version !== PACKAGE_VERSION) throw new Error(`${pkg} is ${version}; README.md records ${PACKAGE_VERSION}`);
const emu = await import(path.join(pkg, 'emu198x_spectrum_web.js'));
emu.initSync({ module: fs.readFileSync(path.join(pkg, 'emu198x_spectrum_web_bg.wasm')) });

const block = (flag, data) => {
  const body = [flag, ...data];
  body.push(body.reduce((x, b) => x ^ b, 0));
  return [body.length & 0xff, body.length >> 8, ...body];
};
// A TAP of the program; line 0x8000 means LOAD "" does not auto-run it.
const tap = (bytes, line) => {
  const header = [0, ...Buffer.from('probe     '), bytes.length & 0xff, bytes.length >> 8,
    line & 0xff, line >> 8, bytes.length & 0xff, bytes.length >> 8];
  return new Uint8Array([...block(0x00, header), ...block(0xff, [...bytes])]);
};
const boot = (image) => {
  const s = emu.Spectrum.createHeadless(new Uint8Array(rom));
  const run = (ms) => { for (let t = 0; t < ms; t += 20) s.tick(20); };
  run(3000);
  s.load(s.mediaSlots()[0], 'tape', image);
  s.autoload(400);
  for (let g = 0; JSON.parse(s.query('tape.playing')) && g < 20000; g++) s.tick(20);
  run(1500);
  const chord = (codes) => {
    for (const c of codes) s.keyDown(c);
    run(100);
    for (const c of [...codes].reverse()) s.keyUp(c);
    run(200);
  };
  const peek16 = (a) => { const [lo, hi] = s.readMemory(a, 2); return lo | (hi << 8); };
  const rows = () => JSON.parse(s.query('screen.text.lines'));
  return { run, chord, peek16, rows };
};

const rowsOut = [];
for (const source of fs.readFileSync(path.join(here, 'cases.bas'), 'utf8').split('\n').filter(Boolean)) {
  const file = path.join(here, '.case.bas');
  fs.writeFileSync(file, `${source}\n`);
  const tok = spawnSync(tokeniser, [file]);
  fs.rmSync(file);
  if (tok.status !== 0) throw new Error(`tokeniser refused ${source}: ${tok.stderr}`);
  const bytes = new Uint8Array(tok.stdout);
  const number = (bytes[0] << 8) | bytes[1];

  // The editor: EDIT (Caps Shift+1) brings the line down, ENTER makes the ROM
  // syntax-check it. A refused line stays in the edit area (E_LINE to WORKSP
  // holds more than its closing 0x0D 0x80).
  const m = boot(tap(bytes, 0x8000));
  m.chord(['ShiftLeft', 'Digit1']);
  m.run(500);
  m.chord(['Enter']);
  m.run(800);
  const editor = m.peek16(23649) - m.peek16(23641) > 2 ? 'refused' : 'accepted';

  // RUN: the same bytes auto-run from a fresh LOAD; the report after 3 s.
  const r = boot(tap(bytes, number));
  r.run(3000);
  const report = r.rows()[23].trim() || '(running)';
  rowsOut.push([editor, report, source].join('\t'));
}
fs.writeFileSync(path.join(here, 'results.tsv'),
  `# editor\trun\tsource  (genuine 48K ROM SHA1 ${ROM_SHA1}, @emu198x/zx-spectrum ${version})\n${rowsOut.join('\n')}\n`);
console.log(`captured ${rowsOut.length} lines`);
