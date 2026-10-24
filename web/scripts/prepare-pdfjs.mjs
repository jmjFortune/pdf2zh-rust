import { cp, mkdir, copyFile } from 'node:fs/promises';
const source = new URL('../node_modules/pdfjs-dist/', import.meta.url);
const output = new URL('../public/pdfjs/', import.meta.url);
await mkdir(output, { recursive: true });
await copyFile(new URL('build/pdf.worker.min.mjs', source), new URL('pdf.worker.min.mjs', output));
await copyFile(new URL('LICENSE', source), new URL('LICENSE', output));
for (const directory of ['cmaps', 'standard_fonts', 'wasm']) {
  await cp(new URL(directory, source), new URL(directory, output), { recursive: true });
}
