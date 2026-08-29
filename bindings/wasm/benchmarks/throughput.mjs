// Throughput benchmark for the wickra-terminal WASM binding.
//
// What this measures is the boundary, not the core. Every binding drives the
// same Rust terminal through one function -- a command JSON in, a frame JSON
// out -- so the number is the cost of crossing this boundary once per command.
// WASM is the interesting row: there is no FFI in the usual sense, but every
// string crossing is copied into and out of linear memory, which is a real cost
// the other bindings do not pay in the same shape.
//
// Build the nodejs target first:
//
//     wasm-pack build bindings/wasm --target nodejs --release --out-dir pkg-node
//
// then, from the repository root:
//
//     node bindings/wasm/benchmarks/throughput.mjs
//     node bindings/wasm/benchmarks/throughput.mjs --ticks 100000

import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { Terminal } = require('../pkg-node/wickra_terminal_wasm.js');

// Shared by all nine binding benchmarks, so the numbers compare.
const CONFIG = JSON.stringify({
  sources: [{ Synth: { seed: 1 } }],
  layout: {
    panels: [
      { kind: 'Chart', rect: { x: 0, y: 0, w: 100, h: 40 } },
      { kind: 'Book', rect: { x: 0, y: 40, w: 50, h: 30 } },
      { kind: 'Tape', rect: { x: 50, y: 40, w: 50, h: 30 } },
    ],
  },
});
const SUBSCRIBE = JSON.stringify({ type: 'Subscribe', source: 0, symbol: 'BTC/USDT' });
const TICK = JSON.stringify({ type: 'Tick' });
const LIST = JSON.stringify({ type: 'ListIndicators' });

// The catalogue response is ~30 kB, so a hundred of them is a noisy sample.
const CATALOGUE_REPS = 1000;

/** Median elapsed nanoseconds over a few repetitions, after one warmup pass. */
function medianNs(run, reps = 3) {
  run();
  const samples = [];
  for (let i = 0; i < reps; i++) {
    const start = process.hrtime.bigint();
    run();
    samples.push(Number(process.hrtime.bigint() - start));
  }
  samples.sort((a, b) => a - b);
  return samples[Math.floor(samples.length / 2)];
}

const flag = process.argv.indexOf('--ticks');
const parsed = flag === -1 ? NaN : Number(process.argv[flag + 1]);
const ticks = Number.isFinite(parsed) && parsed >= 100 ? parsed : 20000;

const term = new Terminal(CONFIG);
term.command(SUBSCRIBE);
const frameBytes = Buffer.byteLength(term.command(TICK));
const catalogueBytes = Buffer.byteLength(term.command(LIST));

const tickNs = medianNs(() => {
  for (let i = 0; i < ticks; i++) term.command(TICK);
});
const listNs = medianNs(() => {
  for (let i = 0; i < CATALOGUE_REPS; i++) term.command(LIST);
});

const n = (value) => value.toLocaleString('en-US');
console.log(`wickra-terminal WASM throughput - ${n(ticks)} commands (median of 3)\n`);
console.log('Command'.padEnd(18) + 'per second'.padStart(14) + 'us/command'.padStart(14) + 'payload'.padStart(12));
console.log('-'.repeat(58));
console.log(
  'Tick'.padEnd(18) +
    n(Math.round(ticks / (tickNs / 1e9))).padStart(14) +
    (tickNs / ticks / 1e3).toFixed(2).padStart(14) +
    `${n(frameBytes)}B`.padStart(12),
);
console.log(
  'ListIndicators'.padEnd(18) +
    n(Math.round(CATALOGUE_REPS / (listNs / 1e9))).padStart(14) +
    (listNs / CATALOGUE_REPS / 1e3).toFixed(2).padStart(14) +
    `${n(catalogueBytes)}B`.padStart(12),
);
console.log(
  '\nOne command crosses the boundary once. Higher is better, and the numbers\n' +
    'are machine-dependent -- compare bindings on one machine, never across two.',
);
