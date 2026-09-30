const config = {
    extends: [
        "@stylistic/stylelint-config",
        "stylelint-config-standard",
        "stylelint-config-clean-order",
    ],
    overrides: [
        {
            customSyntax: "postcss-scss",
            files: ["./src/**/*.scss"],
        },
        {
            customSyntax: "postcss-html",
            files: ["./src/**/*.svelte", "./src/**/*.html"],
        },
    ],
    plugins: [
        "@stylistic/stylelint-plugin",
        "stylelint-order",
        "stylelint-config-rational-order/plugin",
    ],
    rules: {
        "@stylistic/color-hex-case": "upper",
        "@stylistic/indentation": 4,
        "@stylistic/max-empty-lines": 2,
        "@stylistic/named-grid-areas-alignment": [
            true,
            {
                alignQuotes: true,
            },
        ],
        "@stylistic/no-empty-first-line": null,
        "@stylistic/no-eol-whitespace": null,
        "@stylistic/no-missing-end-of-source-newline": null,
        "@stylistic/selector-max-empty-lines": 2,
        "@stylistic/string-quotes": "double",

        "selector-class-pattern": null,
        "selector-pseudo-class-no-unknown": [
            true,
            {
                ignorePseudoClasses: ["global"],
            },
        ],
    },
};

export default config;
