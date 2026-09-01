// Agent Hub session monitor — observe-only Oh My Pi extension.
//
// Feeds Agent Hub's hook-event inbox (~/.agent-hub/session-monitor/inbox/)
// from omp lifecycle events. The inbox consumes serialized HookEvent files
// (camelCase, see session_monitor/types.rs), so we build the same envelope
// the command hooks produce — including `agent: "omp"` for routing.
// Strictly observe-only: every handler is wrapped in try/catch, writes are
// atomic (tmp + rename), and no failure ever surfaces into the omp session.
import { mkdirSync, writeFileSync, renameSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { randomUUID } from "node:crypto";

const INBOX_DIR = path.join(homedir(), ".agent-hub", "session-monitor", "inbox");

// One user-message lifecycle: prompt / assistant reply / stop share a turnId
// (the service groups on it). Turns that start without a captured prompt
// (resumed sessions, headless runs) fall back to per-event ids.
let currentTurnId = null;

function emit(hookEventName, fields) {
  try {
    const eventId = randomUUID();
    const occurredAt = Date.now();
    if (hookEventName === "UserPromptSubmit") {
      currentTurnId = eventId;
    }
    const payload = {
      eventId,
      agent: "omp",
      hookEventName,
      sessionId: fields.sessionId,
      turnId: hookEventName === "UserPromptSubmit" ? eventId : currentTurnId || eventId,
      source: "terminal",
      cwd: fields.cwd,
      userPrompt: fields.userPrompt,
      assistantReply: fields.assistantReply,
      occurredAt,
    };
    if (!payload.sessionId) return;
    mkdirSync(INBOX_DIR, { recursive: true });
    const tmp = path.join(INBOX_DIR, `.${eventId}.tmp`);
    const dest = path.join(INBOX_DIR, `${occurredAt}-${eventId}.json`);
    writeFileSync(tmp, `${JSON.stringify(payload)}\n`, { flag: "wx" });
    renameSync(tmp, dest);
  } catch {
    // Never surface monitor I/O into the omp session.
  }
}

// Message content is either a plain string or a block array; only text
// blocks are visible prose — thinking/toolCall blocks stay out.
function textOf(content) {
  try {
    if (typeof content === "string") return content;
    if (Array.isArray(content)) {
      return content
        .filter((block) => block && block.type === "text" && typeof block.text === "string")
        .map((block) => block.text)
        .join("\n")
        .trim();
    }
  } catch {
    // ignore
  }
  return "";
}

function sessionIdOf(ctx) {
  try {
    const id = ctx?.sessionManager?.getSessionId?.();
    if (typeof id === "string" && id.trim()) return id.trim();
  } catch {
    // ignore
  }
  return undefined;
}

function baseFields(ctx) {
  return {
    sessionId: sessionIdOf(ctx),
    cwd: typeof ctx?.cwd === "string" ? ctx.cwd : undefined,
  };
}

// The `input` payload's prompt field is not formally documented; probe the
// documented-adjacent shapes (string content, block array, .text) and give
// up silently rather than planting an empty row.
function promptOf(event) {
  const candidates = [event?.content, event?.message?.content, event?.text, event?.prompt];
  for (const candidate of candidates) {
    const text = textOf(candidate);
    if (text) return text;
  }
  return undefined;
}

export default function agentHubOmpMonitor(pi) {
  // Never register more than once per session (defensive; omp reloads
  // extensions with an mtime cache-buster during dev).
  pi.on("input", async (event, ctx) => {
    try {
      const prompt = promptOf(event);
      if (!prompt) return;
      const base = baseFields(ctx);
      emit("UserPromptSubmit", { ...base, userPrompt: prompt });
    } catch {
      // ignore
    }
  });

  // message_end fires per assistant message inside one generation; it only
  // fills the reply text — the turn state is owned by turn_end.
  pi.on("message_end", async (event, ctx) => {
    try {
      const message = event?.message ?? event;
      if (!message || message.role !== "assistant") return;
      const reply = textOf(message.content);
      if (!reply) return;
      emit("AssistantResponse", { ...baseFields(ctx), assistantReply: reply });
    } catch {
      // ignore
    }
  });

  // The only turn-boundary signal: gray/ended + final reply.
  pi.on("turn_end", async (event, ctx) => {
    try {
      const message = event?.message ?? event;
      emit("Stop", {
        ...baseFields(ctx),
        assistantReply: textOf(message?.content) || undefined,
      });
    } catch {
      // ignore
    }
  });

  // Yellow light in / out. Observability events fired by the approval
  // wrapper; never block — returning undefined keeps omp's own flow.
  pi.on("tool_approval_requested", async (event, ctx) => {
    try {
      emit("PermissionRequest", baseFields(ctx));
    } catch {
      // ignore
    }
  });

  pi.on("tool_approval_resolved", async (event, ctx) => {
    try {
      emit("PermissionResult", baseFields(ctx));
    } catch {
      // ignore
    }
  });
}
