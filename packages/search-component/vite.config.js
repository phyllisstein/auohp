import tsconfigPaths from 'vite-tsconfig-paths'

/** @type {import('vite').UserConfig} */
export default {
    appType: 'spa',
    plugins: [tsconfigPaths()],
    publicDir: '../public',
    root: './src',
    server: {
        host: '0.0.0.0',
        port: 4040,
    },
}
