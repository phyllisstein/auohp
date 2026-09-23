import { createCommand, type LexicalCommand } from "lexical";


// Dispatched from a click on statement chrome; handled by StatementSeekExtension.
// A command token lives with the extension that registers its handler, not in a
// shared `commands.ts`.
export const SEEK_VIDEO_COMMAND: LexicalCommand<string> = createCommand("SEEK_VIDEO_COMMAND");
