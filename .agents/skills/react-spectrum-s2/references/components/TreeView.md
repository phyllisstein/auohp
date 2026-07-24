# TreeView

A tree view provides users with a way to navigate nested hierarchical information.

```tsx
import {TreeView, TreeViewItem, TreeViewItemContent} from '@react-spectrum/s2/TreeView';
import {style} from '@react-spectrum/s2/style' with {type: 'macro'};

<TreeView
  aria-label="Files"
  
  styles={style({width: 'full'})}
  defaultExpandedKeys={['documents']}>
  <TreeViewItem id="documents" textValue="Documents">
    <TreeViewItemContent>Documents</TreeViewItemContent>
    <TreeViewItem id="project-a" textValue="Project A">
      <TreeViewItemContent>Project A</TreeViewItemContent>
      <TreeViewItem id="report" textValue="Weekly Report">
        <TreeViewItemContent>Weekly Report</TreeViewItemContent>
      </TreeViewItem>
    </TreeViewItem>
    <TreeViewItem id="readme" textValue="README">
      <TreeViewItemContent>README</TreeViewItemContent>
    </TreeViewItem>
  </TreeViewItem>
</TreeView>
```

## Content

`TreeView` follows the [Collection Components API](collections.md?component=TreeView), accepting both static and dynamic collections. This example shows a dynamic collection, passing a list of objects to the `items` prop, and a recursive function to render the children.

```tsx
import {TreeView, TreeViewItem, TreeViewItemContent, Collection} from '@react-spectrum/s2/TreeView';
import {style} from '@react-spectrum/s2/style' with {type: 'macro'};

let items = [
  {id: 1, title: 'Documents', type: 'directory', children: [
    {id: 2, title: 'Project', type: 'directory', children: [
      {id: 3, title: 'Weekly Report', type: 'file', children: []},
      {id: 4, title: 'Budget', type: 'file', children: []}
    ]}
  ]},
  {id: 5, title: 'Photos', type: 'directory', children: [
    {id: 6, title: 'Image 1', type: 'file', children: []},
    {id: 7, title: 'Image 2', type: 'file', children: []}
  ]}
];

<TreeView
  aria-label="Files"
  styles={style({width: 'full'})}
  defaultExpandedKeys={[1, 4]}
  items={items}
  selectionMode="multiple">
  {function renderItem(item) {
    return (
      <TreeViewItem textValue={item.title}>
        <TreeViewItemContent>{item.title}</TreeViewItemContent>
        {/*- begin highlight -*/}
        {/* recursively render children */}
        <Collection items={item.children}>
          {renderItem}
        </Collection>
        {/*- end highlight -*/}
      </TreeViewItem>
    );
  }}
</TreeView>
```

### Slots

`TreeViewItemContent` supports icons, `Text`, [ActionMenu](ActionMenu.md), and [ActionButtonGroup](ActionButtonGroup.md) as children.

```tsx
import {TreeView, TreeViewItem, TreeViewItemContent, Collection, Text} from '@react-spectrum/s2/TreeView';
import {ActionMenu, MenuItem} from '@react-spectrum/s2/ActionMenu';
import Folder from '@react-spectrum/s2/icons/Folder';
import File from '@react-spectrum/s2/icons/File';
import Edit from '@react-spectrum/s2/icons/Edit';
import Delete from '@react-spectrum/s2/icons/Delete';
import {style} from '@react-spectrum/s2/style' with {type: 'macro'};

let items = [
  {id: 1, title: 'Documents', type: 'directory', children: [
    {id: 2, title: 'Project', type: 'directory', children: [
      {id: 3, title: 'Weekly Report', type: 'file', children: []},
      {id: 4, title: 'Budget', type: 'file', children: []}
    ]}
  ]},
  {id: 5, title: 'Photos', type: 'directory', children: [
    {id: 6, title: 'Image 1', type: 'file', children: []},
    {id: 7, title: 'Image 2', type: 'file', children: []}
  ]}
];

<TreeView
  aria-label="Files"
  styles={style({width: 'full'})}
  defaultExpandedKeys={[1, 4]}
  items={items}
  selectionMode="multiple">
  {function renderItem(item) {
    return (
      <TreeViewItem textValue={item.title}>
        <TreeViewItemContent>
          {/*- begin highlight -*/}
          {item.type === 'directory' ? <Folder /> : <File />}
          <Text>{item.title}</Text>
          <ActionMenu>
          {/*- end highlight -*/}
            <MenuItem>
              <Edit />
              <Text>Edit</Text>
            </MenuItem>
            <MenuItem>
              <Delete />
              <Text>Delete</Text>
            </MenuItem>
          </ActionMenu>
        </TreeViewItemContent>
        <Collection items={item.children}>
          {renderItem}
        </Collection>
      </TreeViewItem>
    );
  }}
</TreeView>
```

### Asynchronous loading

Use [renderEmptyState](#empty-state) to display a spinner during initial load. To enable infinite scrolling, render a `<TreeViewLoadMoreItem>` at the end of each `<TreeViewItem>`.

```tsx
import {TreeView, TreeViewItem, TreeViewItemContent, TreeViewLoadMoreItem, Collection} from '@react-spectrum/s2/TreeView';
import {useAsyncList} from '@react-spectrum/s2/useAsyncList';
import {style} from '@react-spectrum/s2/style' with {type: 'macro'};

interface Character {
  name: string
}

function AsyncLoadingExample() {
  let starWarsList = useAsyncList<Character>({
    async load({signal, cursor}) {
      if (cursor) {
        cursor = cursor.replace(/^http:\/\//i, 'https://');
      }

      let res = await fetch(cursor || 'https://swapi.py4e.com/api/people/?search=', {signal});
      let json = await res.json();

      return {
        items: json.results,
        cursor: json.next
      };
    }
  });

  let pokemonList = useAsyncList<Character>({
    async load({signal, cursor, filterText}) {
      let res = await fetch(
        cursor || `https://pokeapi.co/api/v2/pokemon`,
        {signal}
      );
      let json = await res.json();

      return {
        items: json.results,
        cursor: json.next
      };
    }
  });

  return (
    <TreeView
      aria-label="Async loading tree"
      styles={style({height: 300, width: 'full'})}>
      <TreeViewItem textValue="Pokemon">
        <TreeViewItemContent>Pokemon</TreeViewItemContent>
        <Collection items={pokemonList.items}>
          {(item) => (
            <TreeViewItem id={item.name} textValue={item.name}>
              <TreeViewItemContent>{item.name}</TreeViewItemContent>
            </TreeViewItem>
          )}
        </Collection>
        {/*- begin highlight -*/}
        <TreeViewLoadMoreItem
          onLoadMore={pokemonList.loadMore}
          loadingState={pokemonList.loadingState} />
        {/*- end highlight -*/}
      </TreeViewItem>
      <TreeViewItem textValue="Star Wars">
        <TreeViewItemContent>Star Wars</TreeViewItemContent>
        <Collection items={starWarsList.items}>
          {(item) => (
            <TreeViewItem id={item.name} textValue={item.name}>
              <TreeViewItemContent>{item.name}</TreeViewItemContent>
            </TreeViewItem>
          )}
        </Collection>
        {/*- begin highlight -*/}
        <TreeViewLoadMoreItem
          onLoadMore={starWarsList.loadMore}
          loadingState={starWarsList.loadingState} />
        {/*- end highlight -*/}
      </TreeViewItem>
    </TreeView>
  );
}
```

### Links

Use the `href` prop on a `<TreeItem>` to create a link. See the [getting started guide](getting-started.md) to learn how to integrate with your framework.

```tsx
import {TreeView, TreeViewItem, TreeViewItemContent} from '@react-spectrum/s2/TreeView';
import {style} from '@react-spectrum/s2/style' with {type: 'macro'};

<TreeView
  aria-label="TreeView with links"
  selectionMode="multiple"
  styles={style({width: 'full'})}
  defaultExpandedKeys={['bulbasaur', 'ivysaur']}>
  <TreeViewItem
    /*- begin highlight -*/
    href="https://pokemondb.net/pokedex/bulbasaur"
    target="_blank"
    /*- end highlight -*/
    id="bulbasaur"
    textValue="Bulbasaur">
    <TreeViewItemContent>Bulbasaur</TreeViewItemContent>
    <TreeViewItem
      id="ivysaur"
      href="https://pokemondb.net/pokedex/ivysaur"
      target="_blank"
      textValue="Ivysaur">
      <TreeViewItemContent>Ivysaur</TreeViewItemContent>
      <TreeViewItem
        id="venusaur"
        textValue="Venusaur"
        href="https://pokemondb.net/pokedex/venusaur"
        target="_blank">
        <TreeViewItemContent>Venusaur</TreeViewItemContent>
      </TreeViewItem>
    </TreeViewItem>
  </TreeViewItem>
</TreeView>
```

### Empty state

Use `renderEmptyState` to render placeholder content when the tree is empty.

```tsx
import {TreeView} from '@react-spectrum/s2/TreeView';
import {IllustratedMessage, Heading, Content} from '@react-spectrum/s2/IllustratedMessage';
import {Link} from '@react-spectrum/s2/Link';
import FolderOpen from '@react-spectrum/s2/illustrations/linear/FolderOpen';
import {style} from '@react-spectrum/s2/style' with {type: 'macro'};

<TreeView
  aria-label="Search results"
  styles={style({width: 'full'})}
  /*- begin highlight -*/
  renderEmptyState={() => (
    <IllustratedMessage>
      <FolderOpen />
      <Heading>No results</Heading>
      <Content>Press <Link href="https://adobe.com">here</Link> for more info.</Content>
    </IllustratedMessage>
  )}>
  {/*- end highlight -*/}
  {[]}
</TreeView>
```

## Selection and actions

Use the `selectionMode` prop to enable single or multiple selection. The selected items can be controlled via the `selectedKeys` prop, matching the `id` prop of the items. Return an [ActionBar](ActionBar.md) from `renderActionBar` to handle bulk actions, and use `onAction` for item navigation. Items can be disabled with the `isDisabled` prop. See the [selection guide](selection.md?component=Tree) for more details.

```tsx
import {TreeView, TreeViewItem, TreeViewItemContent, Text, type Selection} from '@react-spectrum/s2/TreeView';
import {ActionBar, ActionButton} from '@react-spectrum/s2/ActionBar';
import Edit from '@react-spectrum/s2/icons/Edit';
import Copy from '@react-spectrum/s2/icons/Copy';
import Delete from '@react-spectrum/s2/icons/Delete';
import {style} from '@react-spectrum/s2/style' with {type: 'macro'};
import {useState} from 'react';

function Example(props) {
  let [selected, setSelected] = useState<Selection>(new Set());

  return (
    <div className={style({width: 'full'})}>
      <TreeView
        {...props}
        aria-label="Pokemon evolution"
        styles={style({height: 250, width: 'full', maxWidth: 400})}
        
        selectedKeys={selected}
        onSelectionChange={setSelected}
        onAction={key => alert(`Clicked ${key}`)}
        renderActionBar={(selectedKeys) => {
          let selection = selectedKeys === 'all' ? 'all' : [...selectedKeys].join(', ');
          return (
            <ActionBar>
              <ActionButton onPress={() => alert(`Edit ${selection}`)}>
                <Edit />
                <Text>Edit</Text>
              </ActionButton>
              <ActionButton onPress={() => alert(`Copy ${selection}`)}>
                <Copy />
                <Text>Copy</Text>
              </ActionButton>
              <ActionButton onPress={() => alert(`Delete ${selection}`)}>
                <Delete />
                <Text>Delete</Text>
              </ActionButton>
            </ActionBar>
          );
        }}>
        <TreeViewItem id="bulbasaur" textValue="Bulbasaur">
          <TreeViewItemContent>Bulbasaur</TreeViewItemContent>
          <TreeViewItem id="ivysaur" textValue="Ivysaur">
            <TreeViewItemContent>Ivysaur</TreeViewItemContent>
            <TreeViewItem id="venusaur" isDisabled textValue="Venusaur">
              <TreeViewItemContent>Venusaur</TreeViewItemContent>
            </TreeViewItem>
          </TreeViewItem>
        </TreeViewItem>
        <TreeViewItem id="charmander" textValue="Charmander">
          <TreeViewItemContent>Charmander</TreeViewItemContent>
          <TreeViewItem id="charmeleon" textValue="Charmeleon">
            <TreeViewItemContent>Charmeleon</TreeViewItemContent>
            <TreeViewItem id="charizard" textValue="Charizard">
              <TreeViewItemContent>Charizard</TreeViewItemContent>
            </TreeViewItem>
          </TreeViewItem>
        </TreeViewItem>
        <TreeViewItem id="squirtle" textValue="Squirtle">
          <TreeViewItemContent>Squirtle</TreeViewItemContent>
          <TreeViewItem id="wartortle" textValue="Wartortle">
            <TreeViewItemContent>Wartortle</TreeViewItemContent>
            <TreeViewItem id="blastoise" textValue="Blastoise">
              <TreeViewItemContent>Blastoise</TreeViewItemContent>
            </TreeViewItem>
          </TreeViewItem>
        </TreeViewItem>
      </TreeView>
      <p>Current selection: {selected === 'all' ? 'all' : [...selected].join(', ')}</p>
    </div>
  );
}
```

## Keyboard navigation

By default, TreeView uses arrow key navigation to move focus into rows. Set `keyboardNavigationBehavior="tab"` to have <Keyboard>Tab</Keyboard> move focus in and out of a row.
Use this when rows contain interactive elements such as text fields, where arrow keys and typing in the field should not trigger grid navigation or selection.

```tsx
import {Text} from '@react-spectrum/s2';
import {TreeView, TreeViewItem, TreeViewItemContent} from '@react-spectrum/s2/TreeView';
import {TextField} from '@react-spectrum/s2/TextField';
import {style} from '@react-spectrum/s2/style' with {type: 'macro'};


<TreeView
  /*- begin highlight -*/
  keyboardNavigationBehavior="tab"
  /*- end highlight -*/
  selectionMode="multiple"
  defaultExpandedKeys={['documents', 'photos']}
  aria-label="Shared files"
  styles={style({height: 250, width: 'full', maxWidth: 400})}>
  <TreeViewItem id="documents" textValue="Documents">
    <TreeViewItemContent>
      <Text>
        <TextField styles={style({margin: 4})} aria-label="title" defaultValue="Documents" />
      </Text>
    </TreeViewItemContent>
    <TreeViewItem id="weekly" textValue="Weekly Report.pdf">
      <TreeViewItemContent>
        <Text>
          <TextField styles={style({margin: 4})} aria-label="title" defaultValue="Weekly Report.pdf" />
        </Text>
      </TreeViewItemContent>
    </TreeViewItem>
    <TreeViewItem id="budget" textValue="Budget.xlsx">
      <TreeViewItemContent>
        <Text>
          <TextField styles={style({margin: 4})} aria-label="title" defaultValue="Budget.xlsx" />
        </Text>
      </TreeViewItemContent>
    </TreeViewItem>
  </TreeViewItem>
  <TreeViewItem id="photos" textValue="Photos">
    <TreeViewItemContent>
      <Text>
        <TextField styles={style({margin: 4})} aria-label="title" defaultValue="Photos" />
      </Text>
    </TreeViewItemContent>
    <TreeViewItem id="sunset" textValue="Sunset.jpg">
      <TreeViewItemContent>
        <Text>
          <TextField styles={style({margin: 4})} aria-label="title" defaultValue="Sunset.jpg" />
        </Text>
      </TreeViewItemContent>
    </TreeViewItem>
  </TreeViewItem>
</TreeView>
```

## Drag and drop

TreeView supports drag and drop interactions when the `dragAndDropHooks` prop is provided using the `useDragAndDrop` hook. Users can drop data on the tree as a whole, on individual items, insert new items between existing ones, or reorder items. React Spectrum supports drag and drop via mouse, touch, keyboard, and screen reader interactions. See the [drag and drop guide](dnd.md?component=TreeView) to learn more.

You can customize the drag preview by passing `DragPreview` or your own custom drag preview element to `useDragAndDrop`'s `renderDragPreview`. `DragPreview` supports leading icons and `Text` with `label` and `description` slots.

```tsx
import {Text} from '@react-spectrum/s2';
import {TreeView, TreeViewItem, TreeViewItemContent} from '@react-spectrum/s2/TreeView';
import {style} from '@react-spectrum/s2/style' with {type: 'macro'};
import {DragPreview, useDragAndDrop} from '@react-spectrum/s2/useDragAndDrop';
import {useTreeData} from '@react-spectrum/s2/useTreeData';
import {Collection} from '@react-spectrum/s2';
import Folder from '@react-spectrum/s2/icons/Folder';
import File from '@react-spectrum/s2/icons/File';

function ReorderableTree() {
  let tree = useTreeData({
    initialItems: [
      {id: '1', title: 'Documents', children: [
        {id: '2', title: 'Project', children: [
          {id: '3', title: 'Weekly Report', children: []},
          {id: '4', title: 'Budget', children: []}
        ]}
      ]},
      {id: '5', title: 'Photos', children: [
        {id: '6', title: 'Image 1', children: []},
        {id: '7', title: 'Image 2', children: []}
      ]}
    ]
  });

  let {dragAndDropHooks} = useDragAndDrop({
    getItems: (keys, items: typeof tree.items) => items.map(item => ({'text/plain': item.value.title, numChildren: (item.value.children.length).toString()})),
    onMove(e) {
      if (e.target.dropPosition === 'before') {
        tree.moveBefore(e.target.key, e.keys);
      } else if (e.target.dropPosition === 'after') {
        tree.moveAfter(e.target.key, e.keys);
      } else if (e.target.dropPosition === 'on') {
        let targetNode = tree.getItem(e.target.key);
        if (targetNode) {
          let targetIndex = targetNode.children ? targetNode.children.length : 0;
          let keyArray = Array.from(e.keys);
          for (let i = 0; i < keyArray.length; i++) {
            tree.move(keyArray[i], e.target.key, targetIndex + i);
          }
        }
      }
    },
    renderDragPreview: (items) => (
      <DragPreview items={items}>
        {items[0].numChildren !== '0' ? <Folder /> : <File />}
        <Text>{items[0]['text/plain']}</Text>
        {items[0].numChildren !== '0' && (
          <Text slot="description">{`${items[0].numChildren} item(s)`}</Text>
        )}
      </DragPreview>
    )
  });

  return (
    <TreeView
      aria-label="Tree with hierarchical drag and drop"
      selectionMode="multiple"
      items={tree.items}
      dragAndDropHooks={dragAndDropHooks}
      styles={style({width: 'full', maxWidth: 320})}>
      {function renderItem(item) {
        return (
          <TreeViewItem id={item.key} textValue={item.value.title}>
            <TreeViewItemContent>
              {item.children && item.children.length > 0 ? <Folder /> : <File />}
              <Text>{item.value.title}</Text>
            </TreeViewItemContent>
            {item.children && <Collection items={item.children}>
              {renderItem}
            </Collection>}
          </TreeViewItem>
        );
      }}
    </TreeView>
  );
}
```

## API

```tsx
<TreeView>
  <TreeViewItem>
    <TreeViewItemContent>
      <Icon />
      <Text />
      <ActionMenu /> or <ActionButtonGroup />
    </TreeViewItemContent>
    <TreeViewItem>
      {/* ... */}
    </TreeViewItem>
    <TreeViewLoadMoreItem />
  </TreeViewItem>
</TreeView>
```

### TreeView

| Name | Type | Default | Description |
|------|------|---------|-------------|
| `aria-describedby` | `string | undefined` | — | Identifies the element (or elements) that describes the object. |
| `aria-details` | `string | undefined` | — | Identifies the element (or elements) that provide a detailed, extended description for the object. |
| `aria-label` | `string | undefined` | — | Defines a string value that labels the current element. |
| `aria-labelledby` | `string | undefined` | — | Identifies the element (or elements) that labels the current element. |
| `autoFocus` | `boolean | FocusStrategy | undefined` | — | Whether to auto focus the gridlist or an option. |
| `children` | `((item: T) => ReactNode) | React.ReactNode` | — | The contents of the collection. |
| `defaultExpandedKeys` | `Iterable<Key> | undefined` | — | The initial expanded keys in the collection (uncontrolled). |
| `defaultSelectedKeys` | `"all" | Iterable<Key> | undefined` | — | The initial selected keys in the collection (uncontrolled). |
| `dependencies` | `readonly any[] | undefined` | — | Values that should invalidate the item cache when using dynamic collections. |
| `disabledBehavior` | `DisabledBehavior | undefined` | 'all' | Whether `disabledKeys` applies to all interactions, or only selection. |
| `disabledKeys` | `Iterable<Key> | undefined` | — | The item keys that are disabled. These items cannot be selected, focused, or otherwise interacted with. |
| `disallowEmptySelection` | `boolean | undefined` | — | Whether the collection allows empty selection. |
| `dragAndDropHooks` | `DragAndDropHooks<NoInfer<T>> | undefined` | — | The drag and drop hooks returned by `useDragAndDrop` used to enable drag and drop behavior for the Tree. |
| `escapeKeyBehavior` | `"clearSelection" | "none" | undefined` | 'clearSelection' | Whether pressing the escape key should clear selection in the grid list or not. Most experiences should not modify this option as it eliminates a keyboard user's ability to easily clear selection. Only use if the escape key is being handled externally or should not trigger selection clearing contextually. |
| `expandedKeys` | `Iterable<Key> | undefined` | — | The currently expanded keys in the collection (controlled). |
| `id` | `string | undefined` | — | The element's unique identifier. See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes/id). |
| `items` | `Iterable<T> | undefined` | — | Item objects in the collection. |
| `keyboardNavigationBehavior` | `"arrow" | "tab" | undefined` | 'arrow' | Whether keyboard navigation to focusable elements within grid list items is via the left/right arrow keys or the tab key. |
| `onAction` | `((key: Key) => void) | undefined` | — | Handler that is called when a user performs an action on an item. The exact user event depends on the collection's `selectionBehavior` prop and the interaction modality. |
| `onExpandedChange` | `((keys: Set<Key>) => any) | undefined` | — | Handler that is called when items are expanded or collapsed. |
| `onSelectionChange` | `((keys: Selection) => void) | undefined` | — | Handler that is called when the selection changes. |
| `renderActionBar` | `((selectedKeys: "all" | Set<Key>) => ReactElement) | undefined` | — | Provides the ActionBar to display when items are selected in the TreeView. |
| `renderEmptyState` | `((props: TreeEmptyStateRenderProps) => ReactNode) | undefined` | — | Provides content to display when there are no items in the list. |
| `selectedKeys` | `"all" | Iterable<Key> | undefined` | — | The currently selected keys in the collection (controlled). |
| `selectionMode` | `SelectionMode | undefined` | — | The type of selection that is allowed in the collection. |
| `selectionStyle` | `"checkbox" | "highlight" | undefined` | 'checkbox' | How selection should be displayed. For guidance on when to use which option, refer to the [Spectrum](https://spectrum.adobe.com/page/tree-view/#Checkbox-or-highlight-selection-style) page. |
| `shouldSelectOnPressUp` | `boolean | undefined` | — | Whether selection should occur on press up instead of press down. |
| `slot` | `string | null | undefined` | — | A slot name for the component. Slots allow the component to receive props from a parent component. An explicit `null` value indicates that the local props completely override all props received from a parent. |
| `styles` | `StylesPropWithHeight | undefined` | — | Spectrum-defined styles, returned by the `style()` macro. |
| `UNSAFE_className` | `UnsafeClassName | undefined` | — | Sets the CSS [className](https://developer.mozilla.org/en-US/docs/Web/API/Element/className) for the element. Only use as a **last resort**. Use the `style` macro via the `styles` prop instead. |
| `UNSAFE_style` | `React.CSSProperties | undefined` | — | Sets inline [style](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/style) for the element. Only use as a **last resort**. Use the `style` macro via the `styles` prop instead. |

### TreeViewItem

| Name | Type | Default | Description |
|------|------|---------|-------------|
| `aria-label` | `string | undefined` | — | An accessibility label for this tree item. |
| `children` | `React.ReactNode` | — | The content of the tree item along with any nested children. Supports static nested tree items or use of a Collection to dynamically render nested tree items. |
| `download` | `boolean | string | undefined` | — | Causes the browser to download the linked URL. A string may be provided to suggest a file name. See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/a#download). |
| `hasChildItems` | `boolean | undefined` | — | Whether this item has children, even if not loaded yet. |
| `href` | `string | undefined` | — | A URL to link to. See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/a#href). |
| `hrefLang` | `string | undefined` | — | Hints at the human language of the linked URL. See[MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/a#hreflang). |
| `id` | `Key | undefined` | — | The unique id of the tree row. |
| `isDisabled` | `boolean | undefined` | — | Whether the item is disabled. |
| `onAction` | `(() => void) | undefined` | — | Handler that is called when a user performs an action on this tree item. The exact user event depends on the collection's `selectionBehavior` prop and the interaction modality. |
| `onHoverChange` | `((isHovering: boolean) => void) | undefined` | — | Handler that is called when the hover state changes. |
| `onHoverEnd` | `((e: HoverEvent) => void) | undefined` | — | Handler that is called when a hover interaction ends. |
| `onHoverStart` | `((e: HoverEvent) => void) | undefined` | — | Handler that is called when a hover interaction starts. |
| `onPress` | `((e: PressEvent) => void) | undefined` | — | Handler that is called when the press is released over the target. |
| `onPressChange` | `((isPressed: boolean) => void) | undefined` | — | Handler that is called when the press state changes. |
| `onPressEnd` | `((e: PressEvent) => void) | undefined` | — | Handler that is called when a press interaction ends, either over the target or when the pointer leaves the target. |
| `onPressStart` | `((e: PressEvent) => void) | undefined` | — | Handler that is called when a press interaction starts. |
| `onPressUp` | `((e: PressEvent) => void) | undefined` | — | Handler that is called when a press is released over the target, regardless of whether it started on the target or not. |
| `ping` | `string | undefined` | — | A space-separated list of URLs to ping when the link is followed. See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/a#ping). |
| `referrerPolicy` | `React.HTMLAttributeReferrerPolicy | undefined` | — | How much of the referrer to send when following the link. See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/a#referrerpolicy). |
| `rel` | `string | undefined` | — | The relationship between the linked resource and the current page. See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Attributes/rel). |
| `routerOptions` | `undefined` | — | Options for the configured client side router. |
| `target` | `React.HTMLAttributeAnchorTarget | undefined` | — | The target window for the link. See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/a#target). |
| `textValue` | `string` | — | A string representation of the tree item's contents, used for features like typeahead. |
| `value` | `object | undefined` | — | The object value that this tree item represents. When using dynamic collections, this is set automatically. |

### TreeViewItemContent

| Name | Type | Default | Description |
|------|------|---------|-------------|
| `children` | `React.ReactNode` | — | Rendered contents of the tree item or child items. |

### TreeViewLoadMoreItem

| Name | Type | Default | Description |
|------|------|---------|-------------|
| `loadingState` | `LoadingState | undefined` | — | The current loading state of the TreeView or TreeView row. |
| `onLoadMore` | `(() => any) | undefined` | — | Handler that is called when more items should be loaded, e.g. while scrolling near the bottom. |

## Related Types

### useDragAndDrop

`useDragAndDrop(options: DragAndDropOptions<T>): DragAndDrop<T>`

Provides the hooks required to enable drag and drop behavior for a drag and drop compatible
collection component.

| Name | Type | Default | Description |
|------|------|---------|-------------|
| `acceptedDragTypes` | `"all" | (string | symbol)[] | undefined` | 'all' | The drag types that the droppable collection accepts. If the collection accepts directories, include `DIRECTORY_DRAG_TYPE` in your array of allowed types. |
| `dropTargetDelegate` | `DropTargetDelegate | undefined` | — | A custom delegate object that provides drop targets for pointer coordinates within the collection. |
| `getAllowedDropOperations` | `(() => DropOperation[]) | undefined` | — | Function that returns the drop operations that are allowed for the dragged items. If not provided, all drop operations are allowed. |
| `getDropOperation` | `((target: DropTarget, types: DragTypes, allowedOperations: DropOperation[]) => DropOperation) | undefined` | — | A function returning the drop operation to be performed when items matching the given types are dropped on the drop target. |
| `getItems` | `((keys: Set<Key>, items: T[]) => DragItem[]) | undefined` | () => \[] | A function that returns the items being dragged. If not specified, we assume that the collection is not draggable. |
| `isDisabled` | `boolean | undefined` | — | Whether the drag and drop events should be disabled. |
| `onDragEnd` | `((e: DraggableCollectionEndEvent) => void) | undefined` | — | Handler that is called when the drag operation is ended, either as a result of a drop or a cancellation. |
| `onDragMove` | `((e: DraggableCollectionMoveEvent) => void) | undefined` | — | Handler that is called when the drag is moved. |
| `onDragStart` | `((e: DraggableCollectionStartEvent) => void) | undefined` | — | Handler that is called when a drag operation is started. |
| `onDrop` | `((e: DroppableCollectionDropEvent) => void) | undefined` | — | Handler that is called when a valid drag is dropped on a drop target. When defined, this overrides other drop handlers such as `onInsert`, and `onItemDrop`. |
| `onDropActivate` | `((e: DroppableCollectionActivateEvent) => void) | undefined` | — | Handler that is called after a valid drag is held over a drop target for a period of time. |
| `onDropEnter` | `((e: DroppableCollectionEnterEvent) => void) | undefined` | — | Handler that is called when a valid drag enters a drop target. |
| `onDropExit` | `((e: DroppableCollectionExitEvent) => void) | undefined` | — | Handler that is called when a valid drag exits a drop target. |
| `onInsert` | `((e: DroppableCollectionInsertDropEvent) => void) | undefined` | — | Handler that is called when external items are dropped "between" items. |
| `onItemDrop` | `((e: DroppableCollectionOnItemDropEvent) => void) | undefined` | — | Handler that is called when items are dropped "on" an item. |
| `onMove` | `((e: DroppableCollectionReorderEvent) => void) | undefined` | — | Handler that is called when items are moved within the source collection. This handler allows dropping both on or between items, and items may be moved to a different parent item within a tree. |
| `onReorder` | `((e: DroppableCollectionReorderEvent) => void) | undefined` | — | Handler that is called when items are reordered within the collection. This handler only allows dropping between items, not on items. It does not allow moving items to a different parent item within a tree. |
| `onRootDrop` | `((e: DroppableCollectionRootDropEvent) => void) | undefined` | — | Handler that is called when external items are dropped on the droppable collection's root. |
| `renderDragPreview` | `((items: DragItem[]) => JSX.Element | { element: JSX.Element; x: number; y: number; }) | undefined` | — | A function that renders a drag preview, which is shown under the user's cursor while dragging. By default, a copy of the dragged element is rendered. |
| `shouldAcceptItemDrop` | `((target: ItemDropTarget, types: DragTypes) => boolean) | undefined` | — | A function returning whether a given target in the droppable collection is a valid "on" drop target for the current drag types. |

### DragPreview

`DragPreview(props: DragPreviewProps): JSX.Element`

The default drag preview rendered by the collection element during drag and drop. Pass this to
your drag hooks' \`renderDragPreview\` to match the default visual. To customize the drag preview's
contents, use Text with the "label" and "description" slots and/or a Icon for the leading icon.

| Name | Type | Default | Description |
|------|------|---------|-------------|
| `children` | `ReactNode` | — | The contents of the drag preview. Supports the `label`, `description`, and `icon` slots. If no children are provided, defaults to the first drag item's plain text content. |
| `items` | `DragItem[]` | — | The currently dragged items, sourced from renderDragPreview. |
| `overflowMode` | `"truncate" | "wrap" | undefined` | — | The overflow mode to be applied on the drag preview. |
