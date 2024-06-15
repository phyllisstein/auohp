import stylistic from '@stylistic/eslint-plugin'
import parserTS from '@typescript-eslint/parser'
import tsSortKeys from 'eslint-plugin-typescript-sort-keys'
import tseslint from 'typescript-eslint'
import typescriptESLint from '@typescript-eslint/eslint-plugin'
import sortDestructureKeys from 'eslint-plugin-sort-destructure-keys'
import sortKeysFix from 'eslint-plugin-sort-keys-fix'
import eslint from '@eslint/js'
import jsxA11y from 'eslint-plugin-jsx-a11y'
import react from 'eslint-plugin-react'
import globals from 'globals'

export default [
    eslint.configs.recommended,
    stylistic.configs['recommended-flat'],
    {
        languageOptions: {
            ecmaVersion: 2024,
            globals: {
                ...globals.browser,
                ...globals.es2020,
                ...globals.node,
                ...globals.worker,
            },
            sourceType: 'module',
        },
        plugins: {
            '@stylistic': stylistic,
            'jsx-a11y': jsxA11y,
            react,
            'sort-destructure-keys': sortDestructureKeys,
            'sort-keys-fix': sortKeysFix,
        },
        rules: {
            '@stylistic/brace-style': [
                'warn',
                '1tbs',
                {
                    allowSingleLine: true,
                },
            ],
            '@stylistic/comma-dangle': [
                'warn',
                'always-multiline',
            ],
            '@stylistic/comma-spacing': [
                'warn',
                {
                    after: true,
                    before: false,
                },
            ],
            '@stylistic/eol-last': 'warn',
            '@stylistic/indent': [
                'warn',
                4,
                {
                    ignoredNodes: [
                        'TSTypeParameterInstantiation',
                        'FunctionExpression > .params[decorators.length > 0]',
                        'FunctionExpression > .params > :matches(Decorator, :not(:first-child))',
                        'ClassBody.body > PropertyDefinition[decorators.length > 0] > .key',
                    ],
                },
            ],
            '@stylistic/jsx-closing-bracket-location': ['warn', 'after-props'],
            '@stylistic/jsx-curly-newline': ['warn', 'consistent'],
            '@stylistic/jsx-curly-spacing': [
                'warn',
                {
                    attributes: { when: 'always' },
                    children: { when: 'always' },
                    spacing: { objectLiterals: 'never' },
                    when: 'always',
                },
            ],
            '@stylistic/jsx-indent': [
                'warn',
                4,
                {
                    checkAttributes: true,
                    indentLogicalExpressions: true,
                },
            ],
            '@stylistic/jsx-quotes': [
                'warn',
                'prefer-single',
            ],
            '@stylistic/jsx-sort-props': [
                'warn',
                {
                    callbacksLast: true,
                    ignoreCase: true,
                    noSortAlphabetically: false,
                    reservedFirst: true,
                    shorthandFirst: true,
                },
            ],
            '@stylistic/jsx-tag-spacing': [
                'warn',
                {
                    afterOpening: 'never',
                    beforeClosing: 'never',
                    beforeSelfClosing: 'always',
                    closingSlash: 'never',
                },
            ],
            '@stylistic/member-delimiter-style': [
                'warn',
                {
                    multiline: {
                        delimiter: 'none',
                    },
                    singleline: {
                        delimiter: 'comma',
                        requireLast: false,
                    },
                },
            ],
            '@stylistic/no-multiple-empty-lines': [
                'warn',
                {
                    max: 2,
                    maxBOF: 0,
                    maxEOF: 1,
                },
            ],
            '@stylistic/no-trailing-spaces': 'warn',
            '@stylistic/object-curly-spacing': [
                'warn',
                'always',
            ],
            '@stylistic/operator-linebreak': 'warn',
            '@stylistic/quote-props': ['warn', 'consistent-as-needed'],
            '@stylistic/quotes': [
                'warn',
                'single',
                {
                    allowTemplateLiterals: true,
                    avoidEscape: true,
                },
            ],
            '@stylistic/semi': [
                'warn',
                'never',
                {
                    beforeStatementContinuationChars: 'always',
                },
            ],
            '@stylistic/space-before-function-paren': 'warn',
            '@stylistic/template-curly-spacing': ['warn', 'always'],
            'sort-destructure-keys/sort-destructure-keys': [
                'warn',
                {
                    caseSensitive: false,
                },
            ],
            'sort-keys-fix/sort-keys-fix': [
                'warn',
                'asc',
                {
                    caseSensitive: false,
                    natural: true,
                },
            ],
        },
    },
    {
        files: ['**/*.ts', '**/*.tsx'],
        languageOptions: {
            parser: parserTS,
            parserOptions: {
                project: './tsconfig.json',
            },
        },
        plugins: {
            '@typescript-eslint': typescriptESLint,
            'typescript-sort-keys': tsSortKeys,
        },
        rules: {
            ...tseslint.configs.recommendedTypeChecked.rules,
            ...tseslint.configs.stylisticTypeChecked.rules,
            '@typescript-eslint/no-unused-vars': [
                'warn',
                {
                    args: 'after-used',
                    argsIgnorePattern: '^_',
                    destructuredArrayIgnorePattern: '^_',
                    ignoreRestSiblings: true,
                    varsIgnorePattern: '^_',
                },
            ],
            'no-undef': 'off',
            'no-unused-vars': 'off',
            'typescript-sort-keys/interface': 'warn',
            'typescript-sort-keys/string-enum': 'warn',
        },
    },
]
