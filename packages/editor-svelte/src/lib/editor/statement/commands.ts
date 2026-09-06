import { createCommand, type LexicalCommand } from "lexical";

// Dispatched from a click on statement chrome; handled by the statement-seek
// extension (step 5).
export const SEEK_VIDEO_COMMAND: LexicalCommand<string> = createCommand("SEEK_VIDEO_COMMAND");
