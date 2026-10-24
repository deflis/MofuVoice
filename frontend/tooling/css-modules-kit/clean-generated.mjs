import { readdir, rm } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const sourceDirectory = fileURLToPath(new URL("../../src/", import.meta.url));
const oldOutputDirectory = fileURLToPath(new URL("../../generated/", import.meta.url));

async function removeGeneratedDeclarations(directory) {
  const entries = await readdir(directory, { withFileTypes: true });

  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      await removeGeneratedDeclarations(path);
    } else if (entry.isFile() && entry.name.endsWith(".module.css.d.ts")) {
      await rm(path);
    }
  }
}

await removeGeneratedDeclarations(sourceDirectory);
await rm(oldOutputDirectory, { recursive: true, force: true });
