/**
 * oxlint JS plugin: forbid `export * from "..."` re-exports.
 *
 * A star re-export forces the bundler up from binding-level to module-level
 * analysis: to know which names the star contributes it must load and evaluate
 * the re-exported module, so any module-level side effect there (a
 * `createGlobalStyle` call, a polyfill install) now runs whenever *anything*
 * from the barrel is imported. Explicit `export { X } from "./x"` re-exports
 * stay statically resolvable and individually tree-shakeable --- and the list
 * doubles as a written-down public API surface for the module.
 *
 * `export * as ns from "./x"` is left alone: the `as ns` binds a namespace
 * object with a local name, so it behaves like a normal named export and does
 * not flatten unknown names into the barrel.
 *
 * oxlint has no `no-restricted-syntax` (the core ESLint rule this would
 * otherwise be a one-line selector for), so it lives here as a jsPlugin.
 */

const noExportStar = {
    create (context) {
        return {
            ExportAllDeclaration (node) {
                // `exported` is the Identifier from `export * as ns`; null for
                // the bare `export *` form we want to forbid.
                if (node.exported) {
                    return;
                }

                const source = node.source?.value ?? "the module";
                context.report({
                    node,
                    message:
                        `Re-export named bindings explicitly (export { A, B } from "${ source }"). `
                        + "export * defeats per-binding tree-shaking and runs the re-exported "
                        + "module's side effects on any import from this barrel.",
                });
            },
        };
    },
};

const plugin = {
    meta: {
        name: "no-export-star",
    },
    rules: {
        "no-export-star": noExportStar,
    },
};

export default plugin;
