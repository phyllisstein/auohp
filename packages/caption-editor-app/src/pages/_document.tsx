import { Html, Head, Main, NextScript } from 'next/document'

export default function Document () {
    return (
        <Html className='spectrum spectrum--medium spectrum--dark'>
            <Head />
            <body className='spectrum-Body spectrum-Body--sizeM'>
                <Main />
                <NextScript />
            </body>
        </Html>
    )
}
