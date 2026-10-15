import { registerHooks } from "node:module";

// Node's type stripping needs explicit extensions for the frontend's bundler imports.
const frontendRoot = new URL("../", import.meta.url).href;
export function registerTypeScript() {
  registerHooks({
    resolve(specifier, context, nextResolve) {
      if (
        context.parentURL?.startsWith(frontendRoot) &&
        specifier.startsWith(".") &&
        !/\.[a-z]+$/.test(specifier)
      ) {
        return nextResolve(`${specifier}.ts`, context);
      }
      return nextResolve(specifier, context);
    },
  });
}
