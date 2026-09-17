// chai's own `register-should.d.ts` augments `Object`, but primitive string
// values resolve their properties through the `String` interface, not
// `Object` -- so a string literal like `"foo"` has no typed `.should` even
// though chai's `should()` patches `Object.prototype` at runtime and the
// property genuinely exists. This fills that gap for the primitive wrapper
// interfaces `should()` actually needs to reach in tests.
declare global {
    interface String {
        should: Chai.Assertion;
    }
    interface Number {
        should: Chai.Assertion;
    }
    interface Boolean {
        should: Chai.Assertion;
    }
}

export {};
