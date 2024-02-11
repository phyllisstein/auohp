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
        '../../.eslintrc.common.cjs',
        '../../.eslintrc.react.cjs',
    ],
    overrides: [
        {
            files: ['**/*.ts', '**/*.tsx'],
            parserOptions: {
                project: './tsconfig.json',
                tsconfigRootDir: __dirname,
            },
        },
    ],
    settings: {
        'import/internal-regex': `^(${ moduleDirectories })`,
    },
}
