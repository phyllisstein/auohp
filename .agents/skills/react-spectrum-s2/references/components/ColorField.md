# ColorField

A color field allows users to edit a hex color or individual color channel value.

```tsx
import {ColorField} from '@react-spectrum/s2/ColorField';

<ColorField />
```

## Value

Use the `value` or `defaultValue` prop to set the color value, and `onChange` to handle user input. The value may be a string or `Color` object, parsed using the `parseColor` function. `onChange` is always called with a `Color` object.

```tsx
import {ColorField, type Color} from '@react-spectrum/s2/ColorField';
import {ColorSwatch} from '@react-spectrum/s2/ColorSwatch';
import {useState} from 'react';
import {parseColor} from '@react-stately/color';

function Example() {
  let [value, setValue] = useState<Color | null>(parseColor('#e73623'));

  return (
    <ColorField
      label="Primary color"
      placeholder="Enter a color"
      value={value}
      onChange={setValue}
      prefix={<ColorSwatch size="XS" color={value ? value.toString('hex') : undefined} />} />
  );
}
```

## Channel

By default, ColorField displays a hex value. Set the `colorSpace` and `channel` props to display a specific color channel.

```tsx
import {ColorField, parseColor, type Color} from '@react-spectrum/s2/ColorField';
import {style} from '@react-spectrum/s2/style' with {type: 'macro'};
import {useState} from 'react';

function Example() {
  let [color, setColor] = useState<Color | null>(parseColor('#7f007f'));

  return (
    <div className={style({ display: 'flex', flexDirection: 'column', gap: 8 })}>
      <ColorField
        label="Hue"
        placeholder="Select a hue"
        value={color}
        onChange={setColor}
        /*- begin highlight -*/
        colorSpace="hsl"
        channel="hue" />
        {/*- end highlight -*/}
      <ColorField
        label="Saturation"
        placeholder="Select a saturation"
        value={color}
        onChange={setColor}
        colorSpace="hsl"
        channel="saturation" />
      <ColorField
        label="Lightness"
        placeholder="Select a lightness"
        value={color}
        onChange={setColor}
        colorSpace="hsl"
        channel="lightness" />
      <pre className={style({font: 'body'})}>Current value: {color?.toString('hex')}</pre>
    </div>
  );
}
```

## Forms

Use the `name` prop to submit the text value to the server. Set the `isRequired` prop to validate the value, or implement custom client or server-side validation. See the [Forms](forms.md) guide to learn more.

```tsx
import {ColorField} from '@react-spectrum/s2/ColorField';
import {Button} from '@react-spectrum/s2/Button';
import {Form} from '@react-spectrum/s2/Form';

function Example(props) {
  return (
    <Form>
      <ColorField
        {...props}
        label="Brand color"
        placeholder="Enter a color"
        name="brandColor"
        
      />
      <Button type="submit">Submit</Button>
    </Form>
  );
}
```

## API

| Name | Type | Default | Description |
|------|------|---------|-------------|
| `aria-describedby` | `string | undefined` | — | Identifies the element (or elements) that describes the object. |
| `aria-details` | `string | undefined` | — | Identifies the element (or elements) that provide a detailed, extended description for the object. |
| `aria-errormessage` | `string | undefined` | — | Identifies the element that provides an error message for the object. |
| `aria-label` | `string | undefined` | — | Defines a string value that labels the current element. |
| `aria-labelledby` | `string | undefined` | — | Identifies the element (or elements) that labels the current element. |
| `autoFocus` | `boolean | undefined` | — | Whether the element should receive focus on render. |
| `channel` | `ColorChannel | undefined` | — | The color channel that this field edits. If not provided, the color is edited as a hex value. |
| `colorSpace` | `ColorSpace | undefined` | — | The color space that the color field operates in if a `channel` prop is provided. If no `channel` is provided, the color field always displays the color as an RGB hex value. |
| `contextualHelp` | `ReactNode` | — | A ContextualHelp element to place next to the label. |
| `defaultValue` | `string | Color | null | undefined` | — | The default value (uncontrolled). |
| `description` | `ReactNode` | — | A description for the field. Provides a hint such as specific requirements for what to choose. |
| `errorMessage` | `((v: ValidationResult) => ReactNode) | ReactNode` | — | An error message for the field. |
| `excludeFromTabOrder` | `boolean | undefined` | — | Whether to exclude the element from the sequential tab order. If true, the element will not be focusable via the keyboard by tabbing. This should be avoided except in rare scenarios where an alternative means of accessing the element or its functionality via the keyboard is available. |
| `form` | `string | undefined` | — | The `<form>` element to associate the input with. The value of this attribute must be the id of a `<form>` in the same document. See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input#form). |
| `id` | `string | undefined` | — | The element's unique identifier. See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes/id). |
| `isDisabled` | `boolean | undefined` | — | Whether the input is disabled. |
| `isInvalid` | `boolean | undefined` | — | Whether the input value is invalid. |
| `isReadOnly` | `boolean | undefined` | — | Whether the input can be selected but not changed by the user. |
| `isRequired` | `boolean | undefined` | — | Whether user input is required on the input before form submission. |
| `isWheelDisabled` | `boolean | undefined` | — | Enables or disables changing the value with scroll. |
| `label` | `ReactNode` | — | The content to display as the label. |
| `labelAlign` | `Alignment | undefined` | 'start' | The label's horizontal alignment relative to the element it is labeling. |
| `labelPosition` | `LabelPosition | undefined` | 'top' | The label's overall position relative to the element it is labeling. |
| `name` | `string | undefined` | — | The name of the input element, used when submitting an HTML form. See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/input#htmlattrdefname). |
| `necessityIndicator` | `NecessityIndicator | undefined` | 'icon' | Whether the required state should be shown as an icon or text. |
| `onBeforeInput` | `FormEventHandler<HTMLInputElement> | undefined` | — | Handler that is called when the input value is about to be modified. See [MDN](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/beforeinput_event). |
| `onBlur` | `((e: FocusEvent<Element>) => void) | undefined` | — | Handler that is called when the element loses focus. |
| `onChange` | `((color: Color | null) => void) | undefined` | — | Handler that is called when the value changes. |
| `onCompositionEnd` | `CompositionEventHandler<HTMLInputElement> | undefined` | — | Handler that is called when a text composition system completes or cancels the current text composition session. See [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Element/compositionend_event). |
| `onCompositionStart` | `CompositionEventHandler<HTMLInputElement> | undefined` | — | Handler that is called when a text composition system starts a new text composition session. See [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Element/compositionstart_event). |
| `onCompositionUpdate` | `CompositionEventHandler<HTMLInputElement> | undefined` | — | Handler that is called when a new character is received in the current text composition session. See [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Element/compositionupdate_event). |
| `onCopy` | `ClipboardEventHandler<HTMLInputElement> | undefined` | — | Handler that is called when the user copies text. See [MDN](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/oncopy). |
| `onCut` | `ClipboardEventHandler<HTMLInputElement> | undefined` | — | Handler that is called when the user cuts text. See [MDN](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/oncut). |
| `onFocus` | `((e: FocusEvent<Element>) => void) | undefined` | — | Handler that is called when the element receives focus. |
| `onFocusChange` | `((isFocused: boolean) => void) | undefined` | — | Handler that is called when the element's focus status changes. |
| `onInput` | `FormEventHandler<HTMLInputElement> | undefined` | — | Handler that is called when the input value is modified. See [MDN](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/input_event). |
| `onKeyDown` | `((e: KeyboardEvent) => void) | undefined` | — | Handler that is called when a key is pressed. |
| `onKeyUp` | `((e: KeyboardEvent) => void) | undefined` | — | Handler that is called when a key is released. |
| `onPaste` | `ClipboardEventHandler<HTMLInputElement> | undefined` | — | Handler that is called when the user pastes text. See [MDN](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/onpaste). |
| `onSelect` | `ReactEventHandler<HTMLInputElement> | undefined` | — | Handler that is called when text in the input is selected. See [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Element/select_event). |
| `placeholder` | `string | undefined` | — | Temporary text that occupies the text input when it is empty. See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/placeholder). |
| `prefix` | `ReactNode` | — | The prefix to display in the ColorField. A non-interactive element that appears before the input. |
| `size` | `"L" | "M" | "S" | "XL" | undefined` | 'M' | The size of the color field. |
| `slot` | `string | null | undefined` | — | A slot name for the component. Slots allow the component to receive props from a parent component. An explicit `null` value indicates that the local props completely override all props received from a parent. |
| `styles` | `StylesProp | undefined` | — | Spectrum-defined styles, returned by the `style()` macro. |
| `UNSAFE_className` | `UnsafeClassName | undefined` | — | Sets the CSS [className](https://developer.mozilla.org/en-US/docs/Web/API/Element/className) for the element. Only use as a **last resort**. Use the `style` macro via the `styles` prop instead. |
| `UNSAFE_style` | `CSSProperties | undefined` | — | Sets inline [style](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/style) for the element. Only use as a **last resort**. Use the `style` macro via the `styles` prop instead. |
| `validate` | `((value: Color | null) => true | undefined) | ValidationError | null | undefined` | — | A function that returns an error message if a given value is invalid. Validation errors are displayed to the user when the form is submitted if `validationBehavior="native"`. For realtime validation, use the `isInvalid` prop instead. |
| `validationBehavior` | `"aria" | "native" | undefined` | 'native' | Whether to use native HTML form validation to prevent form submission when the value is missing or invalid, or mark the field as required or invalid via ARIA. |
| `value` | `string | Color | null | undefined` | — | The current value (controlled). |

## Related Types

### Color

`Color(props: IconProps & {size?: 'L' | 'S' | 'M'}): ReactNode`

| Name | Type | Description |
|------|------|-------------|
| `aria-describedby` | `string | undefined` | Identifies the element (or elements) that describes the object. |
| `aria-details` | `string | undefined` | Identifies the element (or elements) that provide a detailed, extended description for the object. |
| `aria-hidden` | `boolean | "true" | "false" | undefined` | — |
| `aria-label` | `string | undefined` | Defines a string value that labels the current element. |
| `aria-labelledby` | `string | undefined` | Identifies the element (or elements) that labels the current element. |
| `id` | `string | undefined` | The element's unique identifier. See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes/id). |
| `slot` | `string | null | undefined` | A slot name for the component. Slots allow the component to receive props from a parent component. An explicit `null` value indicates that the local props completely override all props received from a parent. |
| `styles` | `StyleString<AllowedOverrides> | undefined` | — |
| `UNSAFE_className` | `UnsafeClassName | undefined` | Sets the CSS [className](https://developer.mozilla.org/en-US/docs/Web/API/Element/className) for the element. Only use as a **last resort**. Use the `style` macro via the `styles` prop instead. |
| `UNSAFE_style` | `CSSProperties | undefined` | Sets inline [style](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/style) for the element. Only use as a **last resort**. Use the `style` macro via the `styles` prop instead. |

### parseColor

`parseColor(value: string): IColor`

Parses a color from a string value. Throws an error if the string could not be parsed.
