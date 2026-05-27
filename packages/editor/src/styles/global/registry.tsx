import { type ReactNode } from "react";
import { RecoilRoot } from "recoil";
import { ThemeProvider } from "styled-components";
import { Preflight } from "styled-preflight";

import { AdobeClean } from "styles/assets/fonts";
import { theme } from "styles/theme";

import { Body } from "./body";

export function StyledComponentsRegistry({ children }: { children: ReactNode }) {
    return (
        <RecoilRoot>
            <ThemeProvider theme={ theme }>
                <Preflight />
                <Body />
                <AdobeClean />
                { children }
            </ThemeProvider>
        </RecoilRoot>
    );
}
