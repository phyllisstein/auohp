const fs = require('fs')
const path = require('path')

const sourceRoot = path.resolve('./src')
const dirEntries = fs.readdirSync(sourceRoot, { withFileTypes: true })
const moduleDirectories = dirEntries
  .filter(dirent => dirent.isDirectory())
  .map(dirent => dirent.name)
  .join('|')

  module.exports = {
    extends: [
      '../../.eslintrc.cjs',
      'plugin:ramda/recommended',
    ],
    overrides: [
      {
        files: ['**/*.ts', '**/*.tsx', '**/*.js', '**/*.jsx', '**/*.svelte'],
        parserOptions: {
          project: './tsconfig.json',
          tsconfigRootDir: __dirname,
        },
      },
    ],
    plugins: ['ramda'],
    settings: {
      'import/internal-regex': `^(${ moduleDirectories })`,
    },
  }
