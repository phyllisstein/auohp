import { Html, Head, Main, NextScript } from 'next/document'

export default function Document () {
  return (
    <Html className='spectrum spectrum--large spectrum--dark'>
      <Head />
      <body className='spectrum-Body spectrum-Body--sizeL'>
        <Main />
        <NextScript />
      </body>
    </Html>
  )
}
