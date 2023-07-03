export function CaptionLine ({ attributes, children, element }) {
  return (
    <div { ...attributes } className='spectrum-Textfield spectrum-Textfield--sizeL' contentEditable={ false }>
      <button className='spectrum-Button spectrum-Button--fill spectrum-Button--accent spectrum-Button--sizeM'>
        <span className='spectrum-Button-label'>Button</span>
      </button>
    </div>
  )
}
