/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_NEO4J_URI: string
  readonly VITE_NEO4J_USERNAME: string
  readonly VITE_NEO4J_PASSWORD: string
  readonly VITE_OPENAI_API_KEY: string
  // more env variables...
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
