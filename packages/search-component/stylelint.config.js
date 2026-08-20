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
            customSyntax: "postcss-styled-syntax",
            files: ["./src/**/*.ts", "./src/**/*.tsx"],
        },
    ],
    plugins: [
        "@stylistic/stylelint-plugin",
        "stylelint-order",
        "stylelint-config-rational-order/plugin",
    ],
    rules: {
        "@stylistic/no-extra-semicolons": null,
        "@stylistic/block-closing-brace-newline-before": null,
        "@stylistic/block-opening-brace-newline-before": null,
        "@stylistic/block-opening-brace-space-before": null,
        "@stylistic/color-hex-case": "upper",
        "@stylistic/declaration-block-trailing-semicolon": null,
        "@stylistic/indentation": null,
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

        "media-feature-range-notation": "prefix",
    },
};

export default config;
