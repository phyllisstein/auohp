module.exports = {
  extends: [
    'eslint:recommended',
    'plugin:import/recommended',
    'plugin:import/typescript',
    'plugin:@next/next/recommended',
    'plugin:react-hooks/recommended',
    'plugin:react/recommended',
    'plugin:ramda/recommended',
  ],
  ignorePatterns: [
    '**/.chorus',
    '**/node_modules',
    '**/__generated__',
    '**/dist/**',
  ],
  overrides: [
    {
      extends: [
        'plugin:@typescript-eslint/recommended',
        'plugin:@typescript-eslint/recommended-requiring-type-checking',
      ],
      files: ['**/*.ts', '**/*.tsx'],
      parser: '@typescript-eslint/parser',
      parserOptions: {
        project: './tsconfig.json',
        tsconfigRootDir: __dirname,
      },
      plugins: [
        '@typescript-eslint',
      ],
      rules: {
        '@typescript-eslint/indent': [
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
        '@typescript-eslint/member-delimiter-style': [
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
        // Getting lots of false positives for no-unsafe rules.
        '@typescript-eslint/no-unsafe-argument': 'off',
        '@typescript-eslint/no-unsafe-assignment': 'off',
        '@typescript-eslint/no-unsafe-call': 'off',
        '@typescript-eslint/no-unsafe-member-access': 'off',
        '@typescript-eslint/no-unsafe-return': 'off',
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
        '@typescript-eslint/object-curly-spacing': [
          'warn',
          'always',
        ],
        '@typescript-eslint/quotes': [
          'warn',
          'single',
          {
            allowTemplateLiterals: true,
            avoidEscape: true,
          },
        ],
        '@typescript-eslint/semi': [
          'warn',
          'never',
          {
            beforeStatementContinuationChars: 'always',
          },
        ],
        'object-curly-spacing': 'off',
        'quotes': 'off',
        'semi': 'off',
      },
    },
  ],
  parserOptions: {
    ecmaFeatures: {
      jsx: true,
    },
    ecmaVersion: 2022,
    sourceType: 'module',
  },
  plugins: [
    'import',
    '@next/eslint-plugin-next',
    'ramda',
  ],
  root: true,
  rules: {
    'arrow-parens': [
      'warn',
      'as-needed',
    ],
    'comma-dangle': ['warn', 'always-multiline'],
    'eqeqeq': ['error', 'smart'],
    'import/default': 'off',
    'import/export': 'warn',
    'import/no-named-as-default-member': 'off',
    'import/order': [
      'warn',
      {
        'alphabetize': {
          caseInsensitive: true,
          order: 'asc',
        },
        'groups': [
          'unknown',
          'builtin',
          'external',
          'internal',
          'parent',
          'sibling',
        ],
        'newlines-between': 'always',
        'warnOnUnassignedImports': true,
      },
    ],
    'indent': 'off',
    'jsx-quotes': [
      'warn',
      'prefer-single',
    ],
    'no-inner-declarations': 'off',
    'no-multiple-empty-lines': [
      'warn',
      {
        max: 1,
        maxBOF: 0,
        maxEOF: 1,
      },
    ],
    'no-unused-vars': 'off',
    'object-curly-spacing': [
      'warn',
      'always',
    ],
    'quote-props': ['warn', 'consistent-as-needed'],
    'quotes': [
      'warn',
      'single',
      {
        allowTemplateLiterals: true,
        avoidEscape: true,
      },
    ],
    'semi': [
      'warn',
      'never',
      {
        beforeStatementContinuationChars: 'always',
      },
    ],
    'sort-imports': 'off',
    'space-before-function-paren': [
      'warn',
      'always',
    ],
    'template-curly-spacing': ['warn', 'always'],
  },
  settings: {
    'environments': {
      'browser': true,
      'commonjs': true,
      'es2022': true,
      'node': true,
      'shared-node-browser': true,
    },
    'import/cache': {
      lifetime: '∞',
    },
    'import/parsers': {
      '@babel/parser': ['.js', '.jsx', '.cjs', '.mjs'],
      '@typescript-eslint/parser': ['.ts', '.tsx'],
    },
    'import/resolver': {
      node: true,
      typescript: true,
    },
  },
}
