/**
 * Agent Hub session monitor for DeepSeek Harness.
 *
 * Observe-only: every waterfall calls next() and returns its decision
 * unchanged. Failures here must never break the agent turn.
 */
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import crypto from 'node:crypto'

export const name = 'agent-hub-dsh-monitor'

const INBOX = path.join(os.homedir(), '.agent-hub', 'session-monitor', 'inbox')

function textFromContent(content) {
  if (!content) return ''
  if (typeof content === 'string') return content.trim()
  if (!Array.isArray(content)) return ''
  return content
    .map((block) => {
      if (!block) return ''
      if (typeof block === 'string') return block
      if (block.type === 'text' && typeof block.text === 'string') return block.text
      return ''
    })
    .filter(Boolean)
    .join('\n')
    .trim()
}

function userPromptFromMessages(messages) {
  if (!Array.isArray(messages)) return ''
  const parts = []
  for (const message of messages) {
    const kind = message?.source?.kind
    if (kind && kind !== 'user') continue
    const text = textFromContent(message?.content)
    if (text) parts.push(text)
  }
  return parts.join('\n').trim()
}

function assistantText(event) {
  const message = event?.data?.message ?? event?.message
  return textFromContent(message?.content)
}

function isSubagent(agent, session) {
  const header = session?.header ?? agent?.session?.header
  return header?.origin === 'subagent'
}

function emit(partial) {
  try {
    if (!partial.sessionId) return
    fs.mkdirSync(INBOX, { recursive: true })
    const eventId = crypto.randomUUID()
    const occurredAt = Date.now()
    const payload = {
      eventId,
      agent: 'dsh',
      hookEventName: partial.hookEventName,
      sessionId: String(partial.sessionId),
      turnId: String(partial.turnId ?? eventId),
      source: 'terminal',
      cwd: partial.cwd ?? null,
      userPrompt: partial.userPrompt ?? null,
      assistantReply: partial.assistantReply ?? null,
      occurredAt,
    }
    const tmp = path.join(INBOX, `.${eventId}.tmp`)
    const dest = path.join(INBOX, `${occurredAt}-${eventId}.json`)
    fs.writeFileSync(tmp, `${JSON.stringify(payload)}\n`)
    fs.renameSync(tmp, dest)
  } catch {
    // Never surface monitor I/O into the harness.
  }
}

function sessionMeta(agent) {
  const header = agent?.session?.header ?? {}
  return {
    sessionId: agent?.id ?? header.id,
    cwd: header.cwd ?? null,
  }
}

export function apply(ctx) {
  try {
    register(ctx)
  } catch {
    // A throw here would fail profile boot. Monitor must never do that.
  }
}

function register(ctx) {
  ctx.on('agent/pre-step', async (payload, next) => {
    const decision = await next()
    try {
      if (decision?.kind && decision.kind !== 'enter') return decision
      const agent = payload?.agent
      if (agent && !isSubagent(agent)) {
        const prompt = userPromptFromMessages(payload.messages)
        if (prompt) {
          const meta = sessionMeta(agent)
          emit({
            hookEventName: 'UserPromptSubmit',
            sessionId: meta.sessionId,
            turnId: payload.turn,
            cwd: meta.cwd,
            userPrompt: prompt,
          })
        }
      }
    } catch { /* ignore */ }
    return decision
  })

  ctx.on('agent/turn-stopping', (payload) => {
    try {
      const agent = payload?.agent
      if (agent && !isSubagent(agent)) {
        const meta = sessionMeta(agent)
        emit({
          hookEventName: 'Stop',
          sessionId: meta.sessionId,
          turnId: payload.turn,
          cwd: meta.cwd,
        })
      }
    } catch { /* ignore */ }
  })

  ctx.on('agent/request-error', async (payload, next) => {
    const action = await next()
    try {
      if (action?.kind === 'retry') return action
      const agent = payload?.agent
      if (agent && !isSubagent(agent)) {
        const meta = sessionMeta(agent)
        emit({
          hookEventName: 'StopFailure',
          sessionId: meta.sessionId,
          turnId: payload.turn,
          cwd: meta.cwd,
        })
      }
    } catch { /* ignore */ }
    return action
  })

  ctx.on('agent/error', (payload) => {
    try {
      const agent = payload?.agent
      if (agent && !isSubagent(agent)) {
        const meta = sessionMeta(agent)
        emit({
          hookEventName: 'StopFailure',
          sessionId: meta.sessionId,
          turnId: payload.turn,
          cwd: meta.cwd,
        })
      }
    } catch { /* ignore */ }
  })

  ctx.on('tools/pre-execute', async (exec, next) => {
    const decision = await next()
    try {
      if (decision?.kind !== 'ask') return decision
      const agent = exec?.agent
      if (agent && !isSubagent(agent)) {
        const meta = sessionMeta(agent)
        emit({
          hookEventName: 'PermissionRequest',
          sessionId: meta.sessionId,
          cwd: meta.cwd,
        })
      }
    } catch { /* ignore */ }
    return decision
  })

  ctx.on('tools/post-execute', async (exec, _result, next) => {
    try {
      const agent = exec?.agent
      if (agent && !isSubagent(agent)) {
        const meta = sessionMeta(agent)
        emit({
          hookEventName: 'PostToolUse',
          sessionId: meta.sessionId,
          cwd: meta.cwd,
        })
      }
    } catch { /* ignore */ }
    return next()
  })

  ctx.on('session/event', (session, event) => {
    try {
      if (isSubagent(null, session)) return
      if (event?.type !== 'assistant/message') return
      const text = assistantText(event)
      if (!text) return
      emit({
        hookEventName: 'AssistantResponse',
        sessionId: session?.id ?? session?.header?.id,
        turnId: event.data?.turn ?? event.turn,
        cwd: session?.header?.cwd ?? null,
        assistantReply: text,
      })
    } catch { /* ignore */ }
  })
}
