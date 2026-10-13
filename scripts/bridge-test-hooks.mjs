import { readFile } from "node:fs/promises";
import { stripTypeScriptTypes } from "node:module";
export async function resolve(specifier,context,nextResolve) {
    try {return await nextResolve(specifier,context);}
    catch(error) {
        if(error.code!=="ERR_MODULE_NOT_FOUND" || !context.parentURL?.endsWith(".ts") || !specifier.startsWith("./") || /\.[a-z]+$/i.test(specifier)) throw error;
        return nextResolve(`${specifier}.ts`,context);
    }
}
export async function load(url,context,nextLoad) {
    if(!url.startsWith("file:") || !url.endsWith(".ts")) return nextLoad(url,context);
    return {format:"module",shortCircuit:true,source:stripTypeScriptTypes(await readFile(new URL(url),"utf8"),{mode:"strip",sourceUrl:url})};
}
