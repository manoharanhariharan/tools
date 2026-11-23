// Sync status, pending queue and conflict resolution.

import { h, toast } from '../dom.js';
import { displayDate, formatPaise, formatTimestamp } from '../format.js';
import { basisLabel } from '../pay.js';
import {
  discardLocalVersion,
  keepMyVersion,
  useServerVersion,
} from '../mutations.js';
import { openEmployeeDialog } from './employee-dialog.js';
import { openEntryDialog } from './entry-dialog.js';

const EXPLANATIONS = {
  stale: 'This record was changed on another device after your copy was edited.',
  already_exists: 'The server already has a different record with this id.',
  missing_on_server: 'The server never received this record.',
  duplicate_code: 'Another active employee already uses this employee code.',
  duplicate_entry_date: 'Another entry already exists for that employee and day.',
  validation_failed: 'The server rejected some fields in this change.',
};

export function renderSync(root, ctx) {
  const container = h('section', { class: 'view' });
  root.replaceChildren(container);
  const abort = new AbortController();
  const unsubscribe = ctx.store.subscribe(() => paint());

  container.addEventListener(
    'click',
    async (event) => {
      const control = event.target.closest('[data-action]');
      if (!control) return;
      const action = control.dataset.action;
      const conflict = ctx
        .store
        .get()
        .conflicts.find((candidate) => candidate.key === control.dataset.key);

      try {
        if (action === 'sync-now') await ctx.sync.run({ manual: true });
        else if (action === 'logout') await ctx.logout();
        else if (action === 'keep-mine' && conflict) {
          await keepMyVersion(conflict);
          toast('Your version will be pushed again', 'success');
          ctx.refresh();
          void ctx.sync.run();
        } else if (action === 'use-server' && conflict) {
          await useServerVersion(conflict);
          toast('Server version kept', 'success');
          ctx.refresh();
        } else if (action === 'discard' && conflict) {
          await discardLocalVersion(conflict);
          toast('Local change discarded', 'success');
          ctx.refresh();
        } else if (action === 'edit-conflict' && conflict) {
          openConflictEditor(ctx, conflict);
        }
      } catch (error) {
        console.error(error);
        toast(error?.message ?? 'Action failed', 'error');
      }
    },
    { signal: abort.signal },
  );

  function describe(conflict) {
    if (conflict.entity === 'employee') {
      const row = ctx.findEmployee(conflict.id) ?? conflict.local ?? conflict.server;
      return row?.name ?? 'Employee';
    }
    const row =
      ctx.store.get().entries.find((entry) => entry.id === conflict.id) ??
      conflict.local ??
      conflict.server;
    const employee = ctx.findEmployee(row?.employee_id);
    return `${employee?.name ?? 'Employee'} · ${displayDate(row?.work_date ?? '')}`;
  }

  function comparison(conflict) {
    if (conflict.entity === 'entry') {
      return h(
        'dl',
        { class: 'compare' },
        h('dt', {}, 'Your version'),
        h('dd', {}, conflict.local ? formatPaise(conflict.local.net_minor ?? 0) : 'removed'),
        h('dt', {}, 'Server version'),
        h(
          'dd',
          {},
          conflict.server
            ? conflict.server.deleted_at
              ? 'deleted'
              : formatPaise(conflict.server.net_minor ?? 0)
            : 'not present',
        ),
      );
    }
    return h(
      'dl',
      { class: 'compare' },
      h('dt', {}, 'Your version'),
      h(
        'dd',
        {},
        conflict.local
          ? `${conflict.local.name} · ${formatPaise(conflict.local.rate_minor ?? 0)}`
          : 'removed',
      ),
      h('dt', {}, 'Server version'),
      h(
        'dd',
        {},
        conflict.server
          ? `${conflict.server.name} · ${formatPaise(conflict.server.rate_minor ?? 0)}`
          : 'not present',
      ),
    );
  }

  function conflictCard(conflict) {
    const permanent =
      conflict.code === 'duplicate_code' || conflict.code === 'duplicate_entry_date';
    const isInvalid = conflict.type === 'invalid';
    const fields = conflict.error?.fields ?? [];
    const actions = h('div', { class: 'button-row' });

    if (!permanent && !isInvalid && conflict.server) {
      actions.append(
        h(
          'button',
          { class: 'btn btn-primary', type: 'button', 'data-action': 'keep-mine', 'data-key': conflict.key },
          'Keep my change',
        ),
      );
    }
    if (conflict.server) {
      actions.append(
        h(
          'button',
          { class: 'btn', type: 'button', 'data-action': 'use-server', 'data-key': conflict.key },
          'Use server version',
        ),
      );
    } else {
      actions.append(
        h(
          'button',
          { class: 'btn btn-danger', type: 'button', 'data-action': 'discard', 'data-key': conflict.key },
          'Discard my change',
        ),
      );
    }
    if (conflict.entity === 'entry' || conflict.entity === 'employee') {
      actions.append(
        h(
          'button',
          { class: 'btn btn-ghost', type: 'button', 'data-action': 'edit-conflict', 'data-key': conflict.key },
          conflict.entity === 'entry' ? 'Edit entry' : 'Edit employee',
        ),
      );
    }

    return h(
      'article',
      { class: 'conflict-card' },
      h(
        'header',
        { class: 'conflict-head' },
        h('div', {}, h('h3', {}, describe(conflict)), h('p', { class: 'muted' }, `${conflict.entity} · ${conflict.code ?? conflict.type}`)),
        h('span', { class: 'badge badge-warn' }, isInvalid ? 'Rejected' : 'Conflict'),
      ),
      h('p', {}, EXPLANATIONS[conflict.code] ?? EXPLANATIONS[conflict.type] ?? 'This change needs review.'),
      comparison(conflict),
      fields.length
        ? h(
            'ul',
            { class: 'error-list' },
            fields.map((field) => h('li', {}, `${field.field}: ${field.message}`)),
          )
        : null,
      actions,
    );
  }

  function paint() {
    const state = ctx.store.get();
    const pending = state.outbox.length;
    const conflicts = state.conflicts;

    const statusGrid = h(
      'section',
      { class: 'stat-grid' },
      h(
        'div',
        { class: 'stat' },
        h('span', { class: 'stat-label' }, 'Connection'),
        h('strong', { class: 'stat-value stat-small' }, state.online ? 'Online' : 'Offline'),
        h('span', { class: 'stat-hint' }, state.online ? 'local network' : 'saved on this device'),
      ),
      h(
        'div',
        { class: 'stat' },
        h('span', { class: 'stat-label' }, 'Last sync'),
        h(
          'strong',
          { class: 'stat-value stat-small' },
          state.lastSyncAt ? formatTimestamp(state.lastSyncAt) : 'Not yet',
        ),
        h('span', { class: 'stat-hint' }, state.syncing ? 'syncing now…' : 'automatic every minute'),
      ),
      h(
        'div',
        { class: 'stat' },
        h('span', { class: 'stat-label' }, 'Waiting to push'),
        h('strong', { class: 'stat-value stat-small' }, String(pending)),
        h('span', { class: 'stat-hint' }, 'local changes'),
      ),
      h(
        'div',
        { class: 'stat' },
        h('span', { class: 'stat-label' }, 'Conflicts'),
        h('strong', { class: 'stat-value stat-small' }, String(conflicts.length)),
        h('span', { class: 'stat-hint' }, conflicts.length ? 'needs review' : 'all clear'),
      ),
    );

    const actions = h(
      'div',
      { class: 'button-row' },
      h(
        'button',
        {
          class: 'btn btn-primary',
          type: 'button',
          'data-action': 'sync-now',
          disabled: state.syncing || !state.online,
        },
        state.syncing ? 'Syncing…' : 'Sync now',
      ),
      h('button', { class: 'btn btn-ghost', type: 'button', 'data-action': 'logout' }, 'Log out'),
    );

    const body = conflicts.length
      ? h(
          'div',
          { class: 'conflict-list' },
          h('h2', { class: 'section-title' }, `Needs review (${conflicts.length})`),
          conflicts.map(conflictCard),
        )
      : h(
          'div',
          { class: 'state state-empty' },
          h('h2', {}, 'Everything is in sync'),
          h('p', {}, 'No conflicts or rejected changes. Devices reconcile automatically when they reconnect.'),
        );

    container.replaceChildren(
      h(
        'div',
        { class: 'view-head' },
        h(
          'div',
          {},
          h('h1', {}, 'Sync'),
          h('p', { class: 'subtitle' }, 'Offline changes, sync state and conflict resolution'),
        ),
      ),
      statusGrid,
      state.syncError ? h('div', { class: 'banner banner-error' }, state.syncError) : null,
      actions,
      body,
      h(
        'details',
        { class: 'info-block' },
        h('summary', {}, 'How sync behaves'),
        h(
          'ul',
          {},
          h('li', {}, 'Edits are saved on this device first, then pushed when a connection is available.'),
          h('li', {}, 'Every record carries a server sequence number. Pulls never overwrite edits that are still waiting to push.'),
          h('li', {}, 'If two devices edit the same record, the change is held here so you can choose a version instead of losing data.'),
          h('li', {}, 'Deletes are soft: historical pay stays visible in reports while the row disappears from daily entry screens.'),
          h('li', {}, 'The server recalculates all amounts from pay rules; the preview shown offline is an estimate.'),
        ),
      ),
    );
  }

  function openConflictEditor(currentCtx, conflict) {
    if (conflict.entity === 'entry') {
      const row =
        currentCtx.store.get().entries.find((entry) => entry.id === conflict.id) ??
        conflict.local;
      const employee = currentCtx.findEmployee(row?.employee_id);
      if (!row || !employee) {
        toast('The employee for this entry is unavailable; discard the change instead.', 'error');
        return;
      }
      openEntryDialog(currentCtx, { employee, date: row.work_date, existing: row });
      return;
    }
    const employee = currentCtx.findEmployee(conflict.id) ?? conflict.local;
    if (!employee) {
      toast('This employee no longer exists; discard the change instead.', 'error');
      return;
    }
    openEmployeeDialog(currentCtx, { existing: employee });
  }

  paint();
  return () => {
    abort.abort();
    unsubscribe();
  };
}
