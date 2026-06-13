/// <reference types="react" />

// Minimal JSX type declarations for Spectrum Web Components used in this app.
// These are custom elements — not React components — so TypeScript doesn't know
// their prop shapes without explicit declarations.
declare namespace React {
    namespace JSX {
        interface IntrinsicElements {
            "sp-badge": React.DetailedHTMLProps<
                React.HTMLAttributes<HTMLElement> & {
                    variant?: string;
                },
                HTMLElement
            >;
            "sp-button": React.DetailedHTMLProps<
                React.HTMLAttributes<HTMLElement> & {
                    disabled?: boolean;
                    href?: string;
                    quiet?: boolean;
                    size?: "s" | "m" | "l" | "xl";
                    target?: string;
                    treatment?: "fill" | "outline";
                    variant?: "primary" | "secondary" | "negative" | "white" | "black";
                },
                HTMLElement
            >;
            "sp-theme": React.DetailedHTMLProps<
                React.HTMLAttributes<HTMLElement> & {
                    color?: "light" | "dark" | "lightest" | "darkest";
                    scale?: "medium" | "large";
                    system?: "spectrum" | "express";
                },
                HTMLElement
            >;
        }
    }
}
