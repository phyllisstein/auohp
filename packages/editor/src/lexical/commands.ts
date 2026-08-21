import { createCommand, type LexicalCommand } from "lexical";


// A typed command is the discoverable, first-class way to expose an editor
// action --- `createCommand<Payload>()` gives us a token any component can
// `dispatchCommand(TOKEN, payload)` against, decoupling the toolbar button from
// the node-mutation logic in the plugin.
export const INSERT_TAG_CHIP_COMMAND: LexicalCommand<string> = createCommand("INSERT_TAG_CHIP_COMMAND");

export const INSERT_SEARCH_RESULT_COMMAND: LexicalCommand<string> = createCommand("INSERT_SEARCH_RESULT_COMMAND");

// `void`, not query variables. The handler derives its query from the current
// selection, so a payload carrying `{ uid, query }` was a lie the type system
// was happy to tell --- dispatchers were passing placeholder objects that the
// handler then ignored. Typing the payload as `void` makes the command's real
// contract ("search for whatever is selected") checkable at the call site.
export const PERFORM_SEARCH_COMMAND: LexicalCommand<void> = createCommand("PERFORM_SEARCH_COMMAND");
