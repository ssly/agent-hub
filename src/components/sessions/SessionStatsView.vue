<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { RefreshCw } from 'lucide-vue-next'
import AppLoading from '@/components/ui/AppLoading.vue'
import AgentIcon from '@/components/agents/AgentIcon.vue'
import { useSessionsStore, type SessionStatsRange } from '@/stores/sessions'
import { formatInt, formatSessionTime } from '@/lib/utils'

// The Sessions "All" entry: nothing but tallies. Everything is computed in the
// backend (one pass per session, memoized there) so the ranges below only ever
// re-read what changed since the previous window.
const { t, locale } = useI18n()
const store = useSessionsStore()

const RANGES: SessionStatsRange[] = ['1d', '7d', '31d']

const report = computed(() => store.stats)
const totals = computed(() => report.value?.totals ?? { total: 0, user: 0, assistant: 0 })
const agents = computed(() => report.value?.agents ?? [])

const failedSessions = computed(() =>
  agents.value.reduce((sum, agent) => sum + (agent.failed_sessions || 0), 0),
)

/** Bars are scaled against the busiest Agent, so the chart stays readable
 *  whether the window holds a hundred messages or a hundred thousand. */
const busiest = computed(() =>
  agents.value.reduce((max, agent) => Math.max(max, agent.messages.total), 0),
)

/** Donut sweep for the AI share of all messages (the rest is mine). */
const aiAngle = computed(() =>
  totals.value.total > 0 ? `${(totals.value.assistant / totals.value.total) * 360}deg` : '0deg',
)

function percent(value: number, base: number): number {
  return base > 0 ? Math.round((value / base) * 100) : 0
}

/** Track width of one Agent's bar: proportional to its message count, with a
 *  floor so Agents that do have messages never render as an empty row. */
function barWidth(agent: { messages: { total: number } }): string {
  if (busiest.value <= 0 || agent.messages.total <= 0) return '0%'
  return `${Math.max(2, (agent.messages.total / busiest.value) * 100)}%`
}

function splitWidth(value: number, total: number): string {
  return total > 0 ? `${(value / total) * 100}%` : '0%'
}

function agentLastActive(agent: { last_active_at: number }): string {
  return agent.last_active_at > 0 ? formatSessionTime(agent.last_active_at, locale.value) : ''
}

onMounted(() => {
  // The store keeps the last report across tab switches; only the first visit
  // (or an explicit refresh) needs to read the transcripts again.
  if (!store.stats) store.loadStats()
})
</script>

<template>
  <div class="stats-page">
    <div class="ah-filter-bar ah-filter-bar--list">
      <div class="ah-filter-bar__inner stats-bar">
        <div class="stats-bar__title">
          <span class="stats-bar__heading">{{ t('session.stats_title') }}</span>
          <span class="stats-bar__hint">{{ t('session.stats_hint') }}</span>
        </div>
        <div class="stats-bar__actions">
          <div class="stats-ranges" role="group" :aria-label="t('session.stats_title')">
            <button
              v-for="range in RANGES"
              :key="range"
              type="button"
              class="stats-range"
              :class="{ 'is-active': store.statsRange === range }"
              :aria-pressed="store.statsRange === range"
              @click="store.setStatsRange(range)"
            >
              {{ t(`session.stats_range_${range}`) }}
            </button>
          </div>
          <button
            v-tooltip="t('ui.refresh')"
            type="button"
            class="btn btn-secondary btn-sm btn-icon"
            :disabled="store.statsLoading"
            :aria-label="t('ui.refresh')"
            @click="store.loadStats()"
          >
            <RefreshCw :size="14" :class="{ 'is-spinning': store.statsLoading }" />
          </button>
        </div>
      </div>
    </div>

    <div class="stats-page__body">
      <div class="ah-view-content stats-page__content">
        <AppLoading v-if="store.statsLoading && !report" class="py-16">
          {{ t('session.stats_loading') }}
        </AppLoading>

        <div v-else-if="store.statsError" class="stats-notice" style="color: var(--danger)">
          {{ t('session.stats_error', { error: store.statsError }) }}
        </div>

        <div v-else-if="!report || report.session_count === 0" class="stats-notice">
          {{ t('session.stats_empty') }}
        </div>

        <template v-else>
          <!-- Summary: one donut for the AI/me split, four raw numbers. -->
          <section class="stats-summary" :class="{ 'is-stale': store.statsLoading }">
            <div class="stats-donut" :style="{ '--stats-ai-angle': aiAngle }">
              <div class="stats-donut__hole">
                <strong>{{ formatInt(totals.total) }}</strong>
                <span>{{ t('session.stats_messages') }}</span>
              </div>
            </div>

            <div class="stats-legend">
              <span class="stats-legend__row">
                <i class="stats-legend__dot stats-legend__dot--ai" />
                {{ t('session.stats_ai') }}
                <b>{{ percent(totals.assistant, totals.total) }}%</b>
              </span>
              <span class="stats-legend__row">
                <i class="stats-legend__dot stats-legend__dot--me" />
                {{ t('session.stats_me') }}
                <b>{{ percent(totals.user, totals.total) }}%</b>
              </span>
            </div>

            <div class="stats-cards">
              <div class="stats-card">
                <span class="stats-card__value">{{ formatInt(report.session_count) }}</span>
                <span class="stats-card__label">{{ t('session.stats_sessions') }}</span>
              </div>
              <div class="stats-card">
                <span class="stats-card__value">{{ formatInt(totals.total) }}</span>
                <span class="stats-card__label">{{ t('session.stats_messages') }}</span>
              </div>
              <div class="stats-card">
                <span class="stats-card__value" style="color: var(--success)">
                  {{ formatInt(totals.assistant) }}
                </span>
                <span class="stats-card__label">{{ t('session.stats_ai') }}</span>
              </div>
              <div class="stats-card">
                <span class="stats-card__value" style="color: var(--accent)">
                  {{ formatInt(totals.user) }}
                </span>
                <span class="stats-card__label">{{ t('session.stats_me') }}</span>
              </div>
            </div>
          </section>

          <!-- Per-Agent breakdown. The bar is the share of the busiest Agent,
               split into my prompts and AI replies — no chart library, just
               two flex segments. -->
          <section class="stats-agents">
            <h3 class="stats-agents__heading">{{ t('session.stats_by_agent') }}</h3>

            <div
              v-for="agent in agents"
              :key="agent.platform_id"
              class="stats-agent"
              :class="{ 'is-stale': store.statsLoading }"
            >
              <div class="stats-agent__head">
                <AgentIcon :agent-id="agent.platform_id" :size="14" class="stats-agent__icon" />
                <span
                  v-tooltip:right.clamp="agentLastActive(agent)"
                  class="stats-agent__name"
                >
                  {{ agent.display_name }}
                </span>
                <div class="stats-agent__meta">
                  <span>
                    <b>{{ formatInt(agent.session_count) }}</b>
                    {{ t('session.stats_sessions') }}
                  </span>
                  <span>
                    <b>{{ formatInt(agent.messages.total) }}</b>
                    {{ t('session.stats_messages') }}
                  </span>
                  <span class="stats-agent__meta-ai">
                    AI <b>{{ formatInt(agent.messages.assistant) }}</b>
                  </span>
                  <span class="stats-agent__meta-me">
                    {{ t('session.stats_me') }} <b>{{ formatInt(agent.messages.user) }}</b>
                  </span>
                </div>
              </div>

              <div class="stats-agent__track">
                <div class="stats-agent__bar" :style="{ width: barWidth(agent) }">
                  <i
                    class="stats-agent__seg stats-agent__seg--me"
                    :style="{ width: splitWidth(agent.messages.user, agent.messages.total) }"
                  />
                  <i
                    class="stats-agent__seg stats-agent__seg--ai"
                    :style="{ width: splitWidth(agent.messages.assistant, agent.messages.total) }"
                  />
                </div>
              </div>
            </div>
          </section>

          <div class="stats-footnotes">
            <span v-if="report.inactive_agents > 0">
              {{ t('session.stats_inactive_agents', { n: report.inactive_agents }) }}
            </span>
            <span v-if="failedSessions > 0" style="color: var(--warning)">
              {{ t('session.stats_failed_sessions', { n: failedSessions }) }}
            </span>
            <span>{{ t('session.stats_generated_at', { time: formatSessionTime(report.generated_at, locale) }) }}</span>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.stats-page {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 0;
  --stats-list-pad: 24px;
}
/* The filter bar tracks the list column in the sessions browser; the statistics
   page owns its own gutter (there is no list to align with). */
.ah-filter-bar--list {
  padding-left: var(--stats-list-pad);
  padding-right: var(--stats-list-pad);
}
.ah-filter-bar__inner {
  max-width: none;
  margin: 0;
}
.stats-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}
.stats-bar__title {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}
.stats-bar__heading {
  font-size: 13px;
  font-weight: 600;
  color: var(--ink);
  white-space: nowrap;
}
.stats-bar__hint {
  font-size: 11px;
  color: var(--ink-4);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.stats-bar__actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: none;
}
/* Range picker: one pill, three segments — the active one carries the accent. */
.stats-ranges {
  display: inline-flex;
  padding: 2px;
  gap: 2px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--sunken);
}
.stats-range {
  padding: 3px 10px;
  border: none;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--ink-3);
  font-size: 11.5px;
  line-height: 1.6;
  white-space: nowrap;
  cursor: pointer;
  transition: background var(--dur-fast) var(--ease-soft), color var(--dur-fast) var(--ease-soft);
}
.stats-range:hover { color: var(--ink); }
.stats-range.is-active {
  background: var(--surface);
  color: var(--accent);
  font-weight: 600;
  box-shadow: var(--shadow-mist);
}
.is-spinning { animation: stats-spin .8s linear infinite; }
@keyframes stats-spin { to { transform: rotate(360deg); } }

.stats-page__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 16px var(--stats-list-pad) 28px;
}
.stats-page__content {
  max-width: 64rem;
}
.stats-notice {
  padding: 32px 0;
  text-align: center;
  font-size: 12px;
  color: var(--ink-3);
}
/* Re-counting after a range switch keeps the previous numbers on screen and
   only dims them, so the page never blinks empty. */
.is-stale { opacity: .55; transition: opacity var(--dur-base) var(--ease-soft); }

.stats-summary {
  display: flex;
  align-items: center;
  gap: 20px;
  flex-wrap: wrap;
  padding: 16px 18px;
  border: 1px solid var(--hairline);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-mist);
}
.stats-donut {
  flex: none;
  width: 116px;
  height: 116px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  background: conic-gradient(
    var(--success) 0deg var(--stats-ai-angle),
    var(--accent) var(--stats-ai-angle) 360deg
  );
}
.stats-donut__hole {
  width: 84px;
  height: 84px;
  border-radius: 50%;
  background: var(--surface);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1px;
}
.stats-donut__hole strong {
  font-size: 17px;
  font-weight: 600;
  color: var(--ink);
  font-variant-numeric: tabular-nums;
  line-height: 1.1;
}
.stats-donut__hole span {
  font-size: 10px;
  color: var(--ink-4);
}
.stats-legend {
  display: flex;
  flex-direction: column;
  gap: 7px;
  font-size: 12px;
  color: var(--ink-2);
}
.stats-legend__row {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
}
.stats-legend__row b {
  color: var(--ink);
  font-variant-numeric: tabular-nums;
}
.stats-legend__dot {
  width: 8px;
  height: 8px;
  border-radius: 2px;
  flex: none;
}
.stats-legend__dot--ai { background: var(--success); }
.stats-legend__dot--me { background: var(--accent); }
.stats-cards {
  margin-left: auto;
  display: grid;
  grid-template-columns: repeat(2, minmax(96px, auto));
  gap: 12px 26px;
}
.stats-card {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.stats-card__value {
  font-size: 19px;
  font-weight: 600;
  line-height: 1.15;
  color: var(--ink);
  font-variant-numeric: tabular-nums;
}
.stats-card__label {
  font-size: 11px;
  color: var(--ink-3);
}

.stats-agents {
  margin-top: 20px;
}
.stats-agents__heading {
  margin: 0 0 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--ink-3);
}
.stats-agent {
  padding: 10px 12px 11px;
  margin-bottom: 6px;
  border: 1px solid var(--hairline);
  border-radius: var(--radius-sm);
  background: var(--surface);
  transition: border-color var(--dur-fast) var(--ease-soft);
}
.stats-agent:hover { border-color: var(--border); }
.stats-agent__head {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}
.stats-agent__icon {
  flex: none;
  color: var(--ink-3);
}
.stats-agent__name {
  font-size: 13px;
  font-weight: 500;
  color: var(--ink);
  white-space: nowrap;
}
.stats-agent__meta {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 14px;
  font-size: 11.5px;
  color: var(--ink-3);
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}
.stats-agent__meta b { color: var(--ink-2); font-weight: 600; }
.stats-agent__meta-ai b { color: var(--success); }
.stats-agent__meta-me b { color: var(--accent); }
.stats-agent__track {
  margin-top: 8px;
  height: 8px;
  border-radius: var(--radius-pill);
  background: var(--sunken);
  overflow: hidden;
}
.stats-agent__bar {
  height: 100%;
  display: flex;
  border-radius: var(--radius-pill);
  overflow: hidden;
  transition: width var(--dur-slow) var(--ease-out);
}
.stats-agent__seg { height: 100%; }
.stats-agent__seg--me { background: var(--accent); }
.stats-agent__seg--ai { background: var(--success); }

.stats-footnotes {
  margin-top: 14px;
  display: flex;
  flex-wrap: wrap;
  gap: 4px 14px;
  font-size: 11px;
  color: var(--ink-4);
}
</style>
