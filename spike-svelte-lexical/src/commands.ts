// Verbatim port. `createCommand` is core-lexical; there is no framework in it.
import { createCommand, type LexicalCommand } from "lexical";

export const INSERT_TAG_CHIP_COMMAND: LexicalCommand<string> = createCommand("INSERT_TAG_CHIP_COMMAND");
export const SEEK_VIDEO_COMMAND: LexicalCommand<string> = createCommand("SEEK_VIDEO_COMMAND");
