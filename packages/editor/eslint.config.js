import stylistic from "@stylistic/eslint-plugin";
import tseslint from "typescript-eslint";
import jsxA11y from "eslint-plugin-jsx-a11y";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import { defineConfig } from "eslint/config";
import js from "@eslint/js";
import globals from "globals";

export default defineConfig(
    js.configs.recommended,
    tseslint.configs.recommended,
    tseslint.configs.recommendedTypeChecked,
    stylistic.configs.recommended,
    {
        rules: {
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
            "@stylistic/jsx-indent-props": [
                "warn",
                4,
            ],
            "@stylistic/jsx-one-expression-per-line": ["warn", { allow: "single-line" }],
            "@stylistic/jsx-quotes": [
                "warn",
                "prefer-double",
            ],
            // FIXME: Use `sort-jsx-props` in eslint-plugin-perfectionist
            // "@stylistic/jsx-sort-props": [
            //     "warn",
            //     {
            //         callbacksLast: true,
            //         ignoreCase: true,
            //         noSortAlphabetically: true,
            //         reservedFirst: true,
            //         shorthandFirst: true,
            //     },
            // ],
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
                    allowTemplateLiterals: "always",
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
        }
    },
    {
        files: ['**/*.{js,jsx,mjs,cjs,ts,tsx}'],
        plugins: {
            react,
            "react-hooks": reactHooks,
            "jsx-a11y": jsxA11y,
        },
        settings: {
            react: {
                version: "detect",
            },
        },
        languageOptions: {
            ecmaVersion: 2024,
            globals: {
                ...globals.browser,
                ...globals.es2027,
                ...globals.worker,
            },
            sourceType: "module",
            parserOptions: {
                ecmaFeatures: {
                    jsx: true,
                },
            },
        },
        rules: {
            "react-hooks/exhaustive-deps": [
                "warn",
                {
                    additionalHooks: "(useRecoilCallback|useRecoilTransaction_UNSTABLE)",
                },
            ],
            ...react.configs["jsx-runtime"].rules,
            ...jsxA11y.configs.recommended.rules,
        },
    },
    {
        languageOptions: {
            parserOptions: {
                projectService: true,
                tsconfigRootDir: import.meta.dirname,
            },
        },
    },
);
