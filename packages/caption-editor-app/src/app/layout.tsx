import '@spectrum-css/button'
import '@spectrum-css/inlinealert'
import '@spectrum-css/tokens/dist/index.css'
import '@spectrum-css/vars/dist/spectrum-dark.css'
import '@spectrum-css/vars/dist/spectrum-global.css'
import '@spectrum-css/vars/dist/spectrum-medium.css'
import '@spectrum-css/well'

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
