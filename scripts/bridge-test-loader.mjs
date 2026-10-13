import { registerHooks } from "node:module";

registerHooks({resolve(specifier,context,nextResolve) {
    try { return nextResolve(specifier,context); }
    catch (error) {
        if (error.code !== "ERR_MODULE_NOT_FOUND" || !context.parentURL?.endsWith(".ts") || !specifier.startsWith("./") || /\.[a-z]+$/i.test(specifier)) throw error;
        return nextResolve(`${specifier}.ts`,context);
    }
}});
