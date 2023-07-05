import '@spectrum-css/button/dist/index-vars.css'
import '@spectrum-css/inlinealert/dist/index-vars.css'
import '@spectrum-css/page/dist/index-vars.css'
import '@spectrum-css/tokens/dist/index.css'
import '@spectrum-css/typography/dist/index-vars.css'
import '@spectrum-css/vars/dist/spectrum-dark.css'
import '@spectrum-css/vars/dist/spectrum-global.css'
import '@spectrum-css/vars/dist/spectrum-medium.css'
import { AppProps } from 'next/app'
import Head from 'next/head'
import { RecoilRoot } from 'recoil'
import { ThemeProvider } from 'styled-components'
import { Preflight } from 'styled-preflight'

import { AdobeClean } from 'assets/fonts'
import { Body } from 'styles/global'
import { theme } from 'styles/theme'

function CaptionEditorApp ({ Component, pageProps }: AppProps) {
  return (
    <>
      <Head>
        <title>AUOHP</title>
        <meta content='initial-scale=1.0, width=device-width' name='viewport' />
        <meta content='IE=edge' httpEquiv='X-UA-Compatible' />
      </Head>

      <RecoilRoot>
        <ThemeProvider theme={ theme }>
          <Preflight />
          <AdobeClean />
          <Body />
          <Component { ...pageProps } />
        </ThemeProvider>
      </RecoilRoot>
    </>
  )
}

export default CaptionEditorApp
