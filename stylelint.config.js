const config = {
    extends: [
        '@stylistic/stylelint-config',
        'stylelint-config-recommended',
    ],
    overrides: [
        {
            customSyntax: 'postcss-scss',
            files: ['**/*.scss'],
        },
        {
            customSyntax: '@stylelint/postcss-css-in-js',
            files: ['**/*.ts', '**/*.tsx'],
        },
    ],
    plugins: [
        '@stylistic/stylelint-plugin',
        'stylelint-order',
        'stylelint-config-rational-order/plugin',
    ],
    rules: {
        '@stylistic/block-closing-brace-newline-before': null,
        '@stylistic/block-opening-brace-newline-before': null,
        '@stylistic/block-opening-brace-space-before': null,
        '@stylistic/color-hex-case': 'upper',
        '@stylistic/declaration-block-trailing-semicolon': null,
        '@stylistic/indentation': [
            4,
            {
                baseIndentLevel: 1,
            },
        ],
        '@stylistic/max-empty-lines': 2,
        '@stylistic/named-grid-areas-alignment': [
            true,
            {
                alignQuotes: true,
            },
        ],
        '@stylistic/no-empty-first-line': null,
        '@stylistic/no-eol-whitespace': null,
        '@stylistic/no-missing-end-of-source-newline': null,
        '@stylistic/selector-max-empty-lines': 2,
        '@stylistic/string-quotes': 'single',

        'order/properties-order': [
            [],
            {
                emptyLineMinimumPropertyThreshold: 4,
                unspecified: 'bottom',
            },
        ],

        'plugin/rational-order': [
            true,
            {
                'empty-line-before-unspecified': 'always',
                'empty-line-between-groups': 'threshold',
                'empty-line-property-threshold': 3,
                'empty-lines-within-groups': false,
                'unspecified': 'bottomAlphabetical',
            },
        ],
    },
}

export default config
