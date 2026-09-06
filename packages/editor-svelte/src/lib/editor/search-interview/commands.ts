import { createCommand, type LexicalCommand } from "lexical";

export const INSERT_SEARCH_RESULT_COMMAND: LexicalCommand<string> = createCommand("INSERT_SEARCH_RESULT_COMMAND");
