import bezier from 'bezier-easing'

export const materialAcceleration = bezier(0.4, 0, 1, 1)
export const materialDeceleration = bezier(0, 0, 0.2, 0.1)
export const materialSharp = bezier(0.4, 0, 0.6, 1)
export const materialStandard = bezier(0.4, 0, 0.2, 1)
export const swiftIn = bezier(0.55, 0, 0.55, 0.2)
export const swiftInOut = bezier(0.35, 0, 0.25, 1)
export const swiftOut = bezier(0.25, 0.8, 0.25, 1)
