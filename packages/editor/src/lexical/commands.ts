import { createCommand, type LexicalCommand } from "lexical";


// A typed command is the discoverable, first-class way to expose an editor
// action --- `createCommand<Payload>()` gives us a token any component can
// `dispatchCommand(TOKEN, payload)` against, decoupling the toolbar button from
// the node-mutation logic in the plugin.
export const INSERT_TAG_CHIP_COMMAND: LexicalCommand<string> = createCommand("INSERT_TAG_CHIP_COMMAND");

export const INSERT_SEARCH_RESULT_COMMAND: LexicalCommand<string> = createCommand("INSERT_SEARCH_RESULT_COMMAND");

export const SEEK_VIDEO_COMMAND: LexicalCommand<string> = createCommand("SEEK_VIDEO_COMMAND");
