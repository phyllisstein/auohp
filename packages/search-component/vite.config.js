import tsconfigPaths from 'vite-tsconfig-paths'

/** @type {import('vite').UserConfig} */
export default {
    publicDir: '../public',
    root: './src',
    server: {
        host: '0.0.0.0',
        port: 4040,
    },
    plugins: [tsconfigPaths()],
}
