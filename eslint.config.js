import eslint from '@eslint/js'
import stylistic from '@stylistic/eslint-plugin'
import stylisticTS from '@stylistic/eslint-plugin-ts'
import typescriptESLint from '@typescript-eslint/eslint-plugin'
import parserTS from '@typescript-eslint/parser'
import jsxA11y from 'eslint-plugin-jsx-a11y'
import react from 'eslint-plugin-react'
import globals from 'globals'
import tseslint from 'typescript-eslint'

export default [
  {
    ignores: ['**/dist', '**/node_modules', '**/public', '**/.next'],
  },
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
      '@stylistic/ts': stylisticTS,
      'jsx-a11y': jsxA11y,
      react,
    },
    rules: {
      '@stylistic/arrow-parens': [
        'warn',
        'as-needed',
        {
          requireForBlockBody: false,
        },
      ],
      '@stylistic/block-spacing': 'off',
      '@stylistic/ts/block-spacing': ['warn', 'always'],
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
        2,
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
        2,
        {
          checkAttributes: true,
          indentLogicalExpressions: true,
        },
      ],
      '@stylistic/jsx-indent-props': [
        'warn',
        2,
      ],
      '@stylistic/jsx-one-expression-per-line': 'off',
      '@stylistic/jsx-quotes': [
        'warn',
        'prefer-single',
      ],
      '@stylistic/jsx-sort-props': [
        'warn',
        {
          callbacksLast: true,
          ignoreCase: true,
          noSortAlphabetically: true,
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
      '@stylistic/object-curly-spacing': 'off',
      '@stylistic/ts/object-curly-spacing': [
        'warn',
        'always',
        { objectsInObjects: false },
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
      'react/jsx-sort-props': [
        'warn',
        {
          callbacksLast: true,
          ignoreCase: true,
          noSortAlphabetically: true,
          reservedFirst: true,
          shorthandFirst: true,
          multiline: 'ignore',
        },
      ],
    },
  },
  {
    files: [
      '*.ts',
      '*.tsx',
      '**/*.ts',
      '**/*.tsx',
      '*.d.ts',
      '**/*.d.ts',
    ],
    languageOptions: {
      parser: parserTS,
      parserOptions: {
        project: './tsconfig.json',
      },
    },
    plugins: {
      '@typescript-eslint': typescriptESLint,
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
    },
  },
]
