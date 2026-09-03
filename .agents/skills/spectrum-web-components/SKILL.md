---
name: 'spectrum-web-components'
description: "Build UIs with Spectrum Web Components (SWC), Adobe's Spectrum 1 web component library. Use when developers are working with @spectrum-web-components/* packages or sp-* custom elements. Includes component API references, usage examples, accessibility guidance, and integration guides."
license: 'Apache-2.0'
metadata:
  author: 'Adobe'
  website: 'https://opensource.adobe.com/spectrum-web-components/'
---

# Spectrum Web Components (Spectrum 1)

Spectrum Web Components (SWC) is Adobe's Spectrum 1 design system implemented as
framework-agnostic web components. Elements are prefixed with `sp-` and each ships
as an independent npm package under `@spectrum-web-components`.

> **Upgrading to Spectrum 2?** Use the `migrate-swc-gen1-to-gen2` skill instead,
> which includes per-component migration guides and a full coexistence walkthrough.

## Quick start

Install the components you need (each component is its own package):

```bash
yarn add @spectrum-web-components/theme \
         @spectrum-web-components/button \
         @spectrum-web-components/badge
```

All `sp-*` components must be wrapped in `<sp-theme>` to receive Spectrum CSS tokens:

```html
<script type="module">
  import '@spectrum-web-components/theme/sp-theme.js';
  import '@spectrum-web-components/theme/scale-medium.js';
  import '@spectrum-web-components/theme/theme-light.js';
  import '@spectrum-web-components/button/sp-button.js';
</script>

<sp-theme system="spectrum" scale="medium" color="light">
  <sp-button variant="accent">Save</sp-button>
</sp-theme>
```

## Key concepts

- **`<sp-theme>`** is required — it provides all design tokens (color, scale, spacing,
  typography) to descendant `sp-*` elements via CSS custom properties. See
  [What is a theme?](references/guides/what-is-a-theme.md) for a full explanation of
  `system`, `color`, `scale`, `direction`, and `language` attributes.

- **Per-package install** — each component (`@spectrum-web-components/button`,
  `@spectrum-web-components/badge`, etc.) is imported separately so your bundle only
  includes what you use.

- **Side-effectful import** — importing the `.js` entry registers the custom element:

  ```ts
  import '@spectrum-web-components/button/sp-button.js';
  ```

  Import the class directly for extension or typing:

  ```ts
  import { Button } from '@spectrum-web-components/button';
  ```

- **Spectrum 2 theme bridge** — apply `system="spectrum-two"` to `<sp-theme>` to
  adopt Spectrum 2 visual tokens on Gen 1 components. This is useful when running
  Gen 1 and Gen 2 side-by-side during a gradual migration.

- **React** — use `@swc-react/*` wrapper packages for first-class React event
  handling. See [Using SWC with React](references/guides/using-swc-react.md).

- **Accessibility** — `sp-*` components expose correct ARIA roles, labels, and
  keyboard navigation. Labels are typically provided via the `label` attribute or
  the default slot; see each component's API reference for specifics.

## Documentation structure

The `references/` directory contains guides and one Markdown file per component.
Read the component file for its API, slots, events, CSS custom properties, and usage examples.

### Guides

- [Getting started](references/guides/getting-started.md): Set up a new project and start using Spectrum Web Components.
- [What is a theme?](references/guides/what-is-a-theme.md): Understand sp-theme: system, color, scale, direction, and language.
- [Using SWC with React](references/guides/using-swc-react.md): Use @swc-react/* wrapper components to integrate sp-* elements into React apps.
- [Support and compatibility](references/guides/support-and-compatibility.md): Browser support, versioning policy, and SLA for Spectrum Web Components.
- [Registry conflicts](references/guides/registry-conflicts.md): Diagnose and resolve custom element registry conflicts.
- [Dev mode](references/guides/dev-mode.md): Enable dev mode for additional warnings and debugging information.
- [Deprecation](references/guides/deprecation.md): What is deprecated in Spectrum 1 and what to use instead.
- [Migrating to Spectrum 2 (sp-theme bridge)](references/guides/migrating-to-spectrum2.md): Apply the spectrum-two theme to sp-* components as a visual bridge while migrating.

### Components

One file per component in `references/components/` (e.g. `references/components/sp-badge.md`).
Read the file for props, slots, events, CSS custom properties, and usage examples.

Available components: `sp-accordion`, `sp-action-bar`, `sp-action-button`, `sp-action-group`, `sp-action-menu`, `sp-alert-banner`, `sp-alert-dialog`, `sp-asset`, `sp-avatar`, `sp-badge`, `sp-breadcrumbs`, `sp-button`, `sp-button-group`, `sp-card`, `sp-checkbox`, `sp-coachmark`, `sp-color-area`, `sp-color-field`, `sp-color-handle`, `sp-color-loupe`, `sp-color-slider`, `sp-color-wheel`, `sp-combobox`, `sp-contextual-help`, `sp-dialog`, `sp-divider`, `sp-dropzone`, `sp-field-group`, `sp-field-label`, `sp-help-text`, `sp-icon`, `sp-icons`, `sp-icons-ui`, `sp-icons-workflow`, `sp-iconset`, `sp-illustrated-message`, `sp-infield-button`, `sp-link`, `sp-menu`, `sp-meter`, `sp-number-field`, `sp-overlay`, `sp-picker`, `sp-picker-button`, `sp-popover`, `sp-progress-bar`, `sp-progress-circle`, `sp-radio`, `sp-search`, `sp-sidenav`, `sp-slider`, `sp-split-view`, `sp-status-light`, `sp-swatch`, `sp-switch`, `sp-table`, `sp-tabs`, `sp-tags`, `sp-textfield`, `sp-thumbnail`, `sp-toast`, `sp-tooltip`, `sp-top-nav`, `sp-tray`, `sp-underlay`.

- [sp-accordion](references/components/sp-accordion.md): The sp-accordion element contains a list of items that can be expanded or collapsed to reveal additional content or information associated with each item.
- [sp-action-bar](references/components/sp-action-bar.md): A sp-action-bar delivers a floating action bar that is a convenient way to deliver stateful actions in cases like selection mode.
- [sp-action-button](references/components/sp-action-button.md): An sp-action-button represents an action a user can take.
- [sp-action-group](references/components/sp-action-group.md)
- [sp-action-menu](references/components/sp-action-menu.md): An sp-action-menu is an action button that triggers an overlay with sp-menu-items for activation.
- [sp-alert-banner](references/components/sp-alert-banner.md): The sp-alert-banner displays pressing and high-signal messages, such as system alerts.
- [sp-alert-dialog](references/components/sp-alert-dialog.md): sp-alert-dialog displays important information that users need to acknowledge.
- [sp-asset](references/components/sp-asset.md)
- [sp-avatar](references/components/sp-avatar.md): An sp-avatar is a thumbnail representation of an entity, such as a user or an organization.
- [sp-badge](references/components/sp-badge.md): sp-badge elements display a small amount of color-categorized metadata.
- [sp-breadcrumbs](references/components/sp-breadcrumbs.md): An sp-breadcrumbs shows hierarchy and navigational context for a user's location within an app.
- [sp-button](references/components/sp-button.md): An sp-button represents an action a user can take.
- [sp-button-group](references/components/sp-button-group.md): sp-button-group delivers a set of buttons in horizontal or vertical orientation while ensuring the appropriate spacing between those buttons.
- [sp-card](references/components/sp-card.md): An sp-card represents a rectangular card that contains a variety of text and image layouts.
- [sp-checkbox](references/components/sp-checkbox.md): sp-checkbox allow users to select multiple items from a list of independent options, or to mark an individual option as selected.
- [sp-coachmark](references/components/sp-coachmark.md): sp-coachmark is a temporary message that educates users through new or unfamiliar product experiences.
- [sp-color-area](references/components/sp-color-area.md): An sp-color-area allows users to visually select two properties of a color simultaneously.
- [sp-color-field](references/components/sp-color-field.md): sp-color-field elements are textfields that allow users to input custom color values.
- [sp-color-handle](references/components/sp-color-handle.md): The sp-color-handle is used to select a color on an sp-color-area , sp-color-slider , or sp-color-wheel .
- [sp-color-loupe](references/components/sp-color-loupe.md): An sp-color-loupe shows the output color that would otherwise be covered by a cursor, stylus, or finger during color selection.
- [sp-color-slider](references/components/sp-color-slider.md): An sp-color-slider lets users visually change an individual channel of a color.
- [sp-color-wheel](references/components/sp-color-wheel.md): An sp-color-wheel allows users to visually select the hue of a color on a circular track.
- [sp-combobox](references/components/sp-combobox.md): An sp-combobox allows users to filter lists to only the options matching a query.
- [sp-contextual-help](references/components/sp-contextual-help.md): An sp-contextual-help shows a user extra information about the state of either an adjacent component or an entire view.
- [sp-dialog](references/components/sp-dialog.md): sp-dialog displays important information that users need to acknowledge.
- [sp-divider](references/components/sp-divider.md): sp-divider brings clarity to a layout by grouping and dividing content that exists in close proximity.
- [sp-dropzone](references/components/sp-dropzone.md): A sp-dropzone is an area on the screen into which an object can be dragged and dropped to accomplish a task.
- [sp-field-group](references/components/sp-field-group.md): An sp-field-group element is used to layout a group of fields, usually sp-checkbox elements.
- [sp-field-label](references/components/sp-field-label.md): An sp-field-label provides accessible labelling for form elements.
- [sp-help-text](references/components/sp-help-text.md): An sp-help-text provides either an informative description or an error message that gives more context about what a user needs to input.
- [sp-icon](references/components/sp-icon.md): sp-icon renders an icon to the page.
- [sp-icons](references/components/sp-icons.md): The sp-icons-medium and sp-icons-large elements included in this package supply your application with the Spectrum CSS medium and large icons for use in the sp-icon element.
- [sp-icons-ui](references/components/sp-icons-ui.md): Deliver Spectrum UI Icons as either: - Registered custom elements (sp-icon-arrow75 ) - Unregistered class definitions (I
- [sp-icons-workflow](references/components/sp-icons-workflow.md): Deliver Spectrum Workflow Icons as either: - Registered custom elements (sp-icon-abc ) - Unregistered class definitions 
- [sp-iconset](references/components/sp-iconset.md): Extend either the Iconset or IconsetSVG exports of this package to supply your application with a custom icon set to power the use of sp-icon elements throughout.
- [sp-illustrated-message](references/components/sp-illustrated-message.md): An sp-illustrated-message displays an outline illustration and a message, usually in an empty state or on an error page.
- [sp-infield-button](references/components/sp-infield-button.md)
- [sp-link](references/components/sp-link.md): An sp-link allows users to navigate to a different location.
- [sp-menu](references/components/sp-menu.md): An sp-menu is used for creating a menu list.
- [sp-meter](references/components/sp-meter.md): An sp-meter is a visual representation of a quantity or achievement.
- [sp-number-field](references/components/sp-number-field.md): sp-number-field elements are used for numeric inputs.
- [sp-overlay](references/components/sp-overlay.md): An sp-overlay element is used to decorate content that you would like to present to your visitors as "overlaid" on the rest of the application.
- [sp-picker](references/components/sp-picker.md): An sp-picker is an alternative to HTML's select element.
- [sp-picker-button](references/components/sp-picker-button.md): An sp-picker-button is used as a sub-component of patterns like the sp-combobox (release pending) to pair a button interface with a text input.
- [sp-popover](references/components/sp-popover.md): An sp-popover is used to display transient content (menus, options, additional actions etc.) and appears when clicking/tapping on a source (tools, buttons, etc.) It stands out via its visual style (stroke and drop shadow) and floats on top of the rest of the interface.
- [sp-progress-bar](references/components/sp-progress-bar.md): An sp-progress-bar is used to visually show the progression of a system operation such as downloading, uploading, processing, etc.
- [sp-progress-circle](references/components/sp-progress-circle.md): An sp-progress-circle shows the progression of a system operation such as downloading, uploading, processing, etc.
- [sp-radio](references/components/sp-radio.md): sp-radio and sp-radio-group allow users to select a single option from a list of mutually exclusive options.
- [sp-search](references/components/sp-search.md): The sp-search element is used for searching and filtering items.
- [sp-sidenav](references/components/sp-sidenav.md): Side navigation allows users to locate information and features within the UI.
- [sp-slider](references/components/sp-slider.md): sp-slider allows users to quickly select a value within a range.
- [sp-split-view](references/components/sp-split-view.md): An sp-split-view element displays its first two direct child elements side by side (horizontal) or stacked (vertical with vertical attribute).
- [sp-status-light](references/components/sp-status-light.md): An sp-status-light is a great way to convey semantic meaning, such as statuses and categories.
- [sp-swatch](references/components/sp-swatch.md): An sp-swatch shows a small sample of a fill — such as a color, gradient, texture, or material — that is intended to be applied to an object.
- [sp-switch](references/components/sp-switch.md): An sp-switch is used to turn an option on or off.
- [sp-table](references/components/sp-table.md): An sp-table is used to create a container for displaying information.
- [sp-tabs](references/components/sp-tabs.md): The sp-tabs displays a list of sp-tab element children as role="tablist".
- [sp-tags](references/components/sp-tags.md): sp-tags elements contain a collection of sp-tag elements and allow users to categorize content.
- [sp-textfield](references/components/sp-textfield.md): sp-textfield components are text boxes that allow users to input custom text entries with a keyboard.
- [sp-thumbnail](references/components/sp-thumbnail.md): An sp-thumbnail can be used in a variety of locations as a way to display a preview of an image, layer, or effect.
- [sp-toast](references/components/sp-toast.md): sp-toast elements display brief, temporary notifications.
- [sp-tooltip](references/components/sp-tooltip.md): sp-tooltip allow users to get contextual help or information about specific components when hovering or focusing on them.
- [sp-top-nav](references/components/sp-top-nav.md): sp-top-nav delivers site navigation, particularly for when that navigation will change the majority of the page's content and/or the page's URL when selected.
- [sp-tray](references/components/sp-tray.md): sp-tray elements are typically used to portray information on mobile device or smaller screens.
- [sp-underlay](references/components/sp-underlay.md): An sp-underlay provides a visual layer between overlay content and the rest of your application.
