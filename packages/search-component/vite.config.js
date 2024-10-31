import { nodeResolve } from '@rollup/plugin-node-resolve'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import path from 'path'
import { fileURLToPath } from 'url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

export default defineConfig({
  appType: 'spa',
  plugins: [
    react(),
    nodeResolve({
      extensions: ['.tsx', '.ts', '.js'],
      moduleDirectories: ['src', 'node_modules'],
    }),
  ],
  publicDir: '../public',
  root: './src',
  envDir: __dirname,
  server: {
    host: '0.0.0.0',
    port: 4040,
  },
  preview: {
    host: '0.0.0.0',
    port: 4040,
  },
})
