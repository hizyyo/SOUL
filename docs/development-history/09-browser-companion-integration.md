# Browser Companion Integration

## Objective

Provide the same purpose-scoped context path to supported Chromium web chats.

## Delivered

- Manifest V3 extension for supported ChatGPT, Gemini, and Claude web origins.
- Native Messaging bridge to the local SOUL runtime.
- Site-specific composer adapters with explicit support boundaries.
- Context preview, approval, insertion, and disclosure receipts.
- Replay nonce protection and strict protocol framing.
- Connection closure on malformed, oversized, or unsupported messages.

## Verification

Tests covered extension identity, manifest permissions, adapter detection, composer lifecycle, submission handling, protocol validation, nonce replay, and token-limit validation.

## Security Boundary

The extension requests only Native Messaging and allowlisted site access. Context is not stored in browser extension storage or written to local receipts.
