const fs = require('fs')
const path = require('path')

const sourceRoot = path.resolve(__dirname, './src')
const dirEntries = fs.readdirSync(sourceRoot, { withFileTypes: true })
const moduleDirectories = dirEntries
  .filter(dirent => dirent.isDirectory())
  .map(dirent => dirent.name)
  .join('|')

module.exports = {
  extends: [
    '../../.eslintrc.common.cjs',
  ],
  settings: {
    'import/internal-regex': `^(${ moduleDirectories })\b`,
  },
}
