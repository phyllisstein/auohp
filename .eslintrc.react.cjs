module.exports = {
  extends: [
    'plugin:jsx-a11y/recommended',
    'plugin:react-hooks/recommended',
    'plugin:react/recommended',
    'plugin:import/react',
  ],
  parserOptions: {
    ecmaFeatures: {
      jsx: true,
    },
  },
  plugins: [
    'import',
    'jsx-a11y',
    'react',
    'react-hooks',
  ],
  rules: {
    'react/jsx-closing-bracket-location': ['warn', 'after-props'],
    'react/jsx-curly-newline': ['warn', 'consistent'],
    'react/jsx-curly-spacing': [
      'warn',
      {
        attributes: { when: 'always' },
        children: { when: 'always' },
        spacing: { objectLiterals: 'never' },
        when: 'always',
      },
    ],
    'react/jsx-sort-props': [
      'warn',
      {
        callbacksLast: true,
        ignoreCase: true,
        noSortAlphabetically: true,
        reservedFirst: true,
      },
    ],
    'react/jsx-tag-spacing': [
      'warn',
      {
        afterOpening: 'never',
        beforeClosing: 'never',
        beforeSelfClosing: 'always',
        closingSlash: 'never',
      },
    ],
    'react/no-unknown-property': [
      'warn',
      {
        ignore: ['css'],
      },
    ],
    'react/react-in-jsx-scope': 'off',
    'react/self-closing-comp': 'warn',
  },
  settings: {
    react: {
      version: 'detect',
    },
  },
}
