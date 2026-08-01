/**
 * Минимальные типы Chrome API, используемые расширением.
 * Намеренно НЕТ chrome.storage: контекст никогда не сохраняется.
 */

interface ChromeNativePort {
  readonly name: string;
  postMessage(message: unknown): void;
  onMessage: {
    addListener(listener: (message: unknown) => void): void;
  };
  onDisconnect: {
    addListener(listener: () => void): void;
  };
}

interface ChromeSender {
  readonly id?: string;
  readonly url?: string;
  readonly origin?: string;
}

declare const chrome: {
  readonly runtime: {
    readonly id: string;
    sendMessage(message: unknown): Promise<unknown>;
    onMessage: {
      addListener(
        listener: (
          message: unknown,
          sender: ChromeSender,
          sendResponse: (response: unknown) => unknown,
        ) => unknown,
      ): void;
    };
    connectNative(host: string): ChromeNativePort;
    readonly lastError?: { readonly message?: string };
  };
};
