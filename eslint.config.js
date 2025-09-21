import stylistic from "@stylistic/eslint-plugin";
import stylisticTS from "@stylistic/eslint-plugin-ts";
import parserTS from "@typescript-eslint/parser";
import tseslint from "typescript-eslint";
import typescriptESLint from "@typescript-eslint/eslint-plugin";
import eslint from "@eslint/js";
import jsxA11y from "eslint-plugin-jsx-a11y";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import next from "@next/eslint-plugin-next";

export default [
    {
        ignores: ["dist", "node_modules"],
    },
    eslint.configs.recommended,
    stylistic.configs["recommended-flat"],
    react.configs.flat.recommended,
    react.configs.flat["jsx-runtime"],
    {
        languageOptions: {
            ecmaVersion: 2024,
            globals: {
                ...globals.browser,
                ...globals.es2020,
                ...globals.node,
                ...globals.worker,
            },
            sourceType: "module",
        },
        plugins: {
            "@next/next": next,
            "@stylistic": stylistic,
            "jsx-a11y": jsxA11y,
            react,
            "react-hooks": reactHooks,
        },
        rules: {
            ...next.configs.recommended.rules,
            ...reactHooks.configs.recommended.rules,
            ...jsxA11y.configs.recommended.rules,
            "@stylistic/arrow-parens": [
                "warn",
                "as-needed",
                {
                    requireForBlockBody: false,
                },
            ],
            "@stylistic/block-spacing": ["warn", "never"],
            "@stylistic/brace-style": [
                "warn",
                "1tbs",
                {
                    allowSingleLine: true,
                },
            ],
            "@stylistic/comma-dangle": [
                "warn",
                "always-multiline",
            ],
            "@stylistic/comma-spacing": [
                "warn",
                {
                    after: true,
                    before: false,
                },
            ],
            "@stylistic/eol-last": "warn",
            "@stylistic/indent": [
                "warn",
                4,
            ],
            "@stylistic/jsx-closing-bracket-location": ["warn", "after-props"],
            "@stylistic/jsx-curly-newline": ["warn", "consistent"],
            "@stylistic/jsx-curly-spacing": [
                "warn",
                {
                    attributes: { when: "always" },
                    children: { when: "always" },
                    spacing: { objectLiterals: "never" },
                    when: "always",
                },
            ],
            "@stylistic/jsx-indent": [
                "warn",
                4,
                {
                    checkAttributes: true,
                    indentLogicalExpressions: true,
                },
            ],
            "@stylistic/jsx-indent-props": [
                "warn",
                4,
            ],
            "@stylistic/jsx-one-expression-per-line": ["warn", { allow: "single-line" }],
            "@stylistic/jsx-quotes": [
                "warn",
                "prefer-double",
            ],
            "@stylistic/jsx-sort-props": [
                "warn",
                {
                    callbacksLast: true,
                    ignoreCase: true,
                    noSortAlphabetically: true,
                    reservedFirst: true,
                    shorthandFirst: true,
                },
            ],
            "@stylistic/jsx-tag-spacing": [
                "warn",
                {
                    afterOpening: "never",
                    beforeClosing: "never",
                    beforeSelfClosing: "always",
                    closingSlash: "never",
                },
            ],
            "@stylistic/member-delimiter-style": [
                "warn",
                {
                    multiline: {
                        delimiter: "semi",
                        requireLast: true,
                    },
                    singleline: {
                        delimiter: "semi",
                        requireLast: false,
                    },
                },
            ],
            "@stylistic/no-multiple-empty-lines": [
                "warn",
                {
                    max: 2,
                    maxBOF: 0,
                    maxEOF: 1,
                },
            ],
            "@stylistic/no-trailing-spaces": "warn",
            "@stylistic/object-curly-spacing": [
                "warn",
                "always",
            ],
            "@stylistic/operator-linebreak": "warn",
            "@stylistic/quote-props": ["warn", "consistent-as-needed"],
            "@stylistic/quotes": [
                "warn",
                "double",
                {
                    allowTemplateLiterals: true,
                    avoidEscape: true,
                },
            ],
            "@stylistic/semi": [
                "warn",
                "always",
                {
                    omitLastInOneLineBlock: true,
                    omitLastInOneLineClassBody: true,
                },
            ],
            "@stylistic/space-before-function-paren": "warn",
            "@stylistic/template-curly-spacing": ["warn", "always"],
            "no-unused-vars": "warn",
            "react/prop-types": "off",
            "react-hooks/exhaustive-deps": [
                "warn",
                {
                    additionalHooks: "(useRecoilCallback|useRecoilTransaction_UNSTABLE)",
                },
            ],
        },
        settings: {
            react: {
                version: "detect",
            },
        },
    },
    {
        files: [
            "*.ts",
            "*.tsx",
            "**/*.ts",
            "**/*.tsx",
            "*.d.ts",
            "**/*.d.ts",
        ],
        languageOptions: {
            parser: parserTS,
            parserOptions: {
                projectService: true,
                tsconfigRootDir: import.meta.dirname,
            },
        },
        plugins: {
            "@stylistic/ts": stylisticTS,
            "@typescript-eslint": typescriptESLint,
        },
        rules: {
            ...tseslint.configs.recommendedTypeChecked.rules,
            ...tseslint.configs.stylisticTypeChecked.rules,
            "@stylistic/block-spacing": "off",
            "@stylistic/object-curly-spacing": "off",
            "@stylistic/ts/block-spacing": ["warn", "always"],
            "@stylistic/ts/object-curly-spacing": [
                "warn",
                "always",
            ],
            "@typescript-eslint/no-unused-vars": [
                "warn",
                {
                    args: "after-used",
                    argsIgnorePattern: "^_",
                    destructuredArrayIgnorePattern: "^_",
                    ignoreRestSiblings: true,
                    varsIgnorePattern: "^_",
                },
            ],
        },
    },
];
