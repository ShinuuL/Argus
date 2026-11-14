/**
 * Componentes do chat do Argus, usados pela aba de conversa (`@/views/session`):
 * caixa de composição (menções, comandos de barra, anexos, fila), renderização
 * de mensagens e Markdown, zona de soltar arquivos e a pílula de uso do dia.
 */
import { startQueueDrain } from "./store";
import "./chat.css";

export { Composer } from "./Composer";
export type { ComposerHandle, ComposerProps, OutgoingMessage } from "./Composer";
export { Markdown, MessageList, MessageView } from "./MessageView";
export type { MarkdownProps, MessageListProps, MessageViewProps } from "./MessageView";
export { DropZone } from "./DropZone";
export type { DropZoneProps } from "./DropZone";
export { UsagePill } from "./UsagePill";
export { Lightbox, Thumb } from "./Lightbox";
export { AttachmentChips } from "./AttachmentChips";
export { QueueStrip } from "./QueueStrip";
export { SuggestPopover } from "./SuggestPopover";
export { buildPrompt, splitPrompt } from "./attachments";
export type { AttachmentKind, ChatAttachment, SentAttachment } from "./attachments";
export { LOCAL_COMMANDS } from "./slash";
export type { LocalCommandName } from "./slash";
export { RepeatDetector, repeatMessage, RUN_FOR_REAL_PROMPT, toolCallAsText } from "./toolcall";
export { chatKey, useChat } from "./store";

// A fila de mensagens sai sozinha mesmo sem nenhuma caixa montada.
startQueueDrain();
