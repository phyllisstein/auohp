import '@spectrum-css/button/dist/index-vars.css'
import '@spectrum-css/inlinealert/dist/index-vars.css'
import '@spectrum-css/tokens/dist/index.css'
import '@spectrum-css/vars/dist/spectrum-dark.css'
import '@spectrum-css/vars/dist/spectrum-global.css'
import '@spectrum-css/vars/dist/spectrum-medium.css'
import '@spectrum-css/well/dist/index-vars.css'

import { StyledComponentsRegistry } from 'styles/global'

export default function Layout({ children }: { children: React.ReactNode }) {
    return (
        <html className='spectrum spectrum--medium spectrum--dark' lang='en'>
            <body>
                <StyledComponentsRegistry>
                    { children }
                </StyledComponentsRegistry>
            </body>
        </html>
    )
}
