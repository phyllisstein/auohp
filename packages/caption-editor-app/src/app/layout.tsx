import '@spectrum-css/button/dist/index-vars.css'
import '@spectrum-css/inlinealert/dist/index-vars.css'
import '@spectrum-css/page/dist/index-vars.css'
import '@spectrum-css/tokens/dist/index.css'
import '@spectrum-css/typography/dist/index-vars.css'
import '@spectrum-css/vars/dist/spectrum-dark.css'
import '@spectrum-css/vars/dist/spectrum-global.css'
import '@spectrum-css/vars/dist/spectrum-medium.css'

import { StyledComponentsRegistry } from 'styles/global'

export default function Layout({ children }: { children: React.ReactNode }) {
    return (
        <html className='spectrum spectrum--medium spectrum--dark' lang='en'>
            <body className='spectrum-Body spectrum-Body--sizeM'>
                <StyledComponentsRegistry>
                    { children }
                </StyledComponentsRegistry>
            </body>
        </html>
    )
}
