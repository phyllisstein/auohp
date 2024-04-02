import path from 'path'
import { fileURLToPath } from 'url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

/**
  * @type {import('next').NextConfig}
  */
export default {
    compiler: {
        styledComponents: true,
    },
    webpack(config, { dev }) {
        config.resolve.enforceExtension = false
        config.resolve.modules = [
            path.resolve(__dirname, 'src'),
            path.resolve(__dirname, 'vendor'),
            'node_modules',
            ...config.resolve.modules,
        ]

        config.module.rules.push({
            test: /\.(cql|cypher)$/,
            type: 'asset/source',
        })

        return config
    },
}
