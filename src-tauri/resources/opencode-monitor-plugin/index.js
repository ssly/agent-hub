// Agent Hub session monitor — observe-only OpenCode extension.
//
// Feeds Agent Hub's hook-event inbox (~/.agent-hub/session-monitor/inbox/)
// from OpenCode lifecycle events. The inbox consumes serialized HookEvent files
// (camelCase, see session_monitor/types.rs), so we build the same envelope
// the command hooks produce — including `agent: "opencode"` for routing.
// Strictly observe-only: every handler is wrapped in try/catch, writes are
// atomic (tmp + rename), and no failure ever surfaces into OpenCode.
import { mkdirSync, writeFileSync, renameSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { randomUUID } from "node:crypto";

const INBOX_DIR = path.join(homedir(), ".agent-hub", "session-monitor", "inbox");

const sessionTurns = new Map();
const sessionPrompts = new Map();
const sessionReplies = new Map();

function emit(hookEventName, fields) {
  try {
    const sessionId = fields.sessionId;
    if (!sessionId) return;

    const eventId = randomUUID();
    const occurredAt = Date.now();

    if (hookEventName === "UserPromptSubmit") {
      sessionTurns.set(sessionId, eventId);
      if (fields.userPrompt) {
        sessionPrompts.set(sessionId, fields.userPrompt);
      }
      sessionReplies.delete(sessionId);
    }

    const turnId = sessionTurns.get(sessionId) || eventId;
    const userPrompt = fields.userPrompt || sessionPrompts.get(sessionId);
    const assistantReply = fields.assistantReply || sessionReplies.get(sessionId);

    const payload = {
      eventId,
      agent: "opencode",
      hookEventName,
      sessionId,
      turnId,
      source: "terminal",
      cwd: fields.cwd,
      userPrompt: userPrompt || undefined,
      assistantReply: assistantReply || undefined,
      occurredAt,
    };

    mkdirSync(INBOX_DIR, { recursive: true });
    const tmp = path.join(INBOX_DIR, `.${eventId}.tmp`);
    const dest = path.join(INBOX_DIR, `${occurredAt}-${eventId}.json`);
    writeFileSync(tmp, `${JSON.stringify(payload)}\n`, { flag: "wx" });
    renameSync(tmp, dest);
  } catch {
    // Never surface monitor I/O into the OpenCode session.
  }
}

function extractText(item) {
  if (!item) return "";
  if (typeof item === "string") return item.trim();
  if (typeof item.text === "string" && item.text.trim()) return item.text.trim();
  if (typeof item.prompt === "string" && item.prompt.trim()) return item.prompt.trim();
  if (item.payload) {
    const res = extractText(item.payload);
    if (res) return res;
  }
  if (item.data) {
    const res = extractText(item.data);
    if (res) return res;
  }
  if (item.info) {
    const res = extractText(item.info);
    if (res) return res;
  }
  if (Array.isArray(item.parts)) {
    const res = item.parts.map(extractText).filter(Boolean).join("\n").trim();
    if (res) return res;
  }
  if (Array.isArray(item.content)) {
    const res = item.content.map(extractText).filter(Boolean).join("\n").trim();
    if (res) return res;
  }
  return "";
}

function handleEvent(ev, defaultCwd) {
  try {
    if (!ev || typeof ev !== "object") return;
    const type = ev.type;
    const data = ev.data || {};
    const sessionId = data.sessionID || data.sessionId || ev.sessionID || ev.sessionId;
    const cwd = ev.location?.directory || defaultCwd;

    if (!sessionId && !data.sessionID) return;
    const sid = sessionId || data.sessionID;

    if (type === "session.inbox.enqueued") {
      const item = data.item;
      if (item) {
        const text = extractText(item);
        if (text) {
          sessionPrompts.set(sid, text);
          emit("UserPromptSubmit", { sessionId: sid, cwd, userPrompt: text });
        }
      }
    } else if (type === "session.execution.started") {
      emit("UserPromptSubmit", { sessionId: sid, cwd, userPrompt: sessionPrompts.get(sid) });
    } else if (type === "message.updated") {
      const info = data.info;
      if (info?.role === "user") {
        const text = extractText(info);
        if (text) {
          sessionPrompts.set(sid, text);
          emit("UserPromptSubmit", { sessionId: sid, cwd, userPrompt: text });
        }
      } else if (info?.role === "assistant") {
        const text = extractText(info);
        if (text) {
          sessionReplies.set(sid, text);
        }
      }
    } else if (type === "message.part.updated") {
      const part = data.part;
      if (part && (part.type === "text" || part.text)) {
        const text = extractText(part);
        if (text) {
          if (part.role === "user") {
            sessionPrompts.set(sid, text);
            emit("UserPromptSubmit", { sessionId: sid, cwd, userPrompt: text });
          } else {
            sessionReplies.set(sid, text);
          }
        }
      }
    } else if (type === "session.text.ended") {
      if (data.text) {
        sessionReplies.set(sid, data.text);
      }
    } else if (type === "permission.asked") {
      emit("PermissionRequest", { sessionId: sid, cwd });
    } else if (type === "permission.replied") {
      emit("PermissionResult", { sessionId: sid, cwd });
    } else if (type === "session.execution.succeeded") {
      emit("Stop", { sessionId: sid, cwd, assistantReply: sessionReplies.get(sid) });
    } else if (type === "session.execution.failed" || type === "session.execution.interrupted" || type === "session.error") {
      emit("StopFailure", { sessionId: sid, cwd });
    } else if (type === "session.idle") {
      emit("Stop", { sessionId: sid, cwd, assistantReply: sessionReplies.get(sid) });
    }
  } catch {
    // Ignore all handler errors
  }
}

export default {
  id: "agent-hub-opencode-monitor",
  setup(context) {
    const abort = new AbortController();
    const defaultCwd = context.location?.directory;

    (async () => {
      try {
        const stream = context.event.subscribe({ signal: abort.signal });
        for await (const ev of stream) {
          handleEvent(ev, defaultCwd);
        }
      } catch {
        // Stream ended or aborted
      }
    })();

    return () => {
      abort.abort();
    };
  },
};
