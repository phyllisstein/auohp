# LabeledValue

A LabeledValue displays a non-editable value with a label. It formats numbers,
dates, times, and lists according to the user's locale.

```tsx
import {LabeledValue} from '@react-spectrum/s2/LabeledValue';

<LabeledValue />
```

## Value

In addition to a string as shown above, a `LabeledValue` accepts numbers, dates, times, and lists of strings in the `value` prop.

### Numbers

When the `value` prop is set to a number, `LabeledValue` formats it according to the user's locale. Custom `formatOptions` can also be provided to format the value as a percentage, unit, currency, etc. This prop is compatible
with the option parameter of [Intl.NumberFormat](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/NumberFormat).

```tsx
import {LabeledValue} from '@react-spectrum/s2';

<LabeledValue />
```

An object with `start` and `end` properties may also be provided to format a numeric range.

```tsx
"use client"
import {LabeledValue} from '@react-spectrum/s2/LabeledValue';

<LabeledValue 
  label="Price range"
  value={{start: 150, end: 400}}
  formatOptions={{style: 'currency', currency: 'USD', minimumFractionDigits: 0}} />
```

### Dates and times

The `value` prop may be set to a JavaScript [Date](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date) object, or one of the types from [@internationalized/date](react-aria:internationalized/date/.md) to display a date or time, which is formatted according to the user's locale. Custom `formatOptions` can also be provided control the exact date format. This prop is compatible
with the option parameter of [Intl.DateTimeFormat](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/DateTimeFormat/DateTimeFormat).

```tsx
import {LabeledValue} from '@react-spectrum/s2';

<LabeledValue />
```

By default, the formatting depends on the type of value provided. Above, a [CalendarDate](react-aria:internationalized/date/CalendarDate.md) is provided, so only the date is displayed. To display a time, pass a [Time](react-aria:internationalized/date/Time.md) object. You can also provide a [CalendarDateTime](react-aria:internationalized/date/CalendarDateTime.md) or [ZonedDateTime](react-aria:internationalized/date/ZonedDateTime.md) to display a date with a time.

```tsx
"use client"
import {LabeledValue} from '@react-spectrum/s2/LabeledValue';
import {CalendarDateTime} from '@internationalized/date';

<LabeledValue label="Page load time" value={new CalendarDateTime(2022, 7, 5, 14, 30)} />
```

An object with `start` and `end` properties may also be provided to format a date or time range.

```tsx
"use client"
import {LabeledValue} from '@react-spectrum/s2/LabeledValue';
import {Time} from '@internationalized/date';

<LabeledValue label="Business hours" value={{start: new Time(8, 30), end: new Time(18)}} />
```

### Lists

When the `value` prop is set to an array of strings, they are rendered as a comma-separated list according to the user's locale.

By default, the list is displayed as a conjunction (an "and"-based grouping of items). This may be changed to a disjunction (an "or"-based grouping), or a pure comma-separated list using the `formatOptions` prop. This is compatible with the option parameter of [Intl.ListFormat](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/ListFormat/ListFormat).

```tsx
import {LabeledValue} from '@react-spectrum/s2';

<LabeledValue />
```

### Components

The value can be a component and will be rendered as provided. Components cannot be editable.

```tsx
"use client"
import {LabeledValue} from '@react-spectrum/s2/LabeledValue';
import {Link} from '@react-spectrum/s2/Link';

<LabeledValue label="Website" value={<Link href="https://www.adobe.com/">Adobe.com</Link>} />
```

## Form

Use `LabeledValue` within a `Form` to display non-editable information alongside editable fields.

```tsx
import {Button} from '@react-spectrum/s2/Button';
import {ComboBox, ComboBoxItem} from '@react-spectrum/s2/ComboBox';
import {Form} from '@react-spectrum/s2/Form';
import {LabeledValue} from '@react-spectrum/s2/LabeledValue';
import {NumberField} from '@react-spectrum/s2/NumberField';
import {style} from '@react-spectrum/s2/style' with {type: 'macro'};

<Form>
  <div
    role="group"
    className={style({display: 'flex', alignItems: 'end', gap: 48})}>
    <ComboBox label="Duration type" styles={style({width: 120})}>
      <ComboBoxItem>Simple</ComboBoxItem>
      <ComboBoxItem>Calculated assignment</ComboBoxItem>
      <ComboBoxItem>Calculated work</ComboBoxItem>
      <ComboBoxItem>Effort driven</ComboBoxItem>
    </ComboBox>
    <NumberField
    label="Duration days"
    placeholder="# of days"
    defaultValue={1} 
    styles={style({width: 120})}/>
  </div>
  <div
    role="group"
    aria-labelledby="working hours"
    className={style({display: 'flex', alignItems: 'end', gap: 48, font: 'ui'})}>
    <NumberField
      label="Planned hours"
      placeholder="–"
      defaultValue={1} 
      styles={style({width: 120})}/>
    {/*- begin highlight -*/}
    <LabeledValue 
      label="Actual hours"
      value={0}
      styles={style({width: 120})}/>
    {/*- end highlight -*/}
  </div>
  <div
    role="group"
    aria-labelledby="working hours"
    className={style({display: 'flex', alignItems: 'end', gap: 48, font: 'ui'})}>
    <LabeledValue 
      label="Project duration"
      value={1}
      styles={style({width: 120})}/>
    <LabeledValue 
      label="Actual duration"
      value={0}
      styles={style({width: 120})}/>
  </div>
  <Button
    type="submit"
    variant="primary">
    Submit
  </Button>
</Form>
```

## API
