// Daily earnings screen: one row per employee, showing the business they
// generated and the final money for the selected date.

import { h, emptyState } from '../dom.js';
import { addDays, displayDate, formatPaise, isoToday } from '../format.js';
import { openEmployeeDialog } from './employee-dialog.js';
import { openEntryDialog } from './entry-dialog.js';

function initials(name) {
  return String(name)
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? '')
    .join('');
}

function weekday(iso) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!match) return '';
  const date = new Date(Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3])));
  return new Intl.DateTimeFormat('en-IN', { weekday: 'long', timeZone: 'UTC' }).format(date);
}

function statusTags(ctx, entry) {
  if (!entry) return [];
  const tags = [];
  if (ctx.isPending('entry', entry.id)) tags.push(h('span', { class: 'badge badge-pending' }, 'Pending'));
  if (ctx.findConflict('entry', entry.id)) tags.push(h('span', { class: 'badge badge-warn' }, 'Conflict'));
  return tags;
}

export function renderToday(root, ctx) {
  const container = h('section', { class: 'view' });
  root.replaceChildren(container);
  let date = ctx.store.get().selectedDate ?? isoToday();
  const abort = new AbortController();
  const unsubscribe = ctx.store.subscribe(() => paint());

  container.addEventListener(
    'click',
    (event) => {
      const control = event.target.closest('[data-action]');
      if (!control) return;
      const action = control.dataset.action;
      if (action === 'prev-day') setDate(addDays(date, -1));
      else if (action === 'next-day') setDate(addDays(date, 1));
      else if (action === 'today') setDate(isoToday());
      else if (action === 'add-employee') openEmployeeDialog(ctx, {});
      else if (action === 'manage-employees') ctx.navigate('/employees');
      else if (action === 'open-entry') {
        const employee = ctx.findEmployee(control.dataset.employeeId);
        if (!employee) return;
        const existing = ctx.store.get().entries.find(
          (entry) => entry.employee_id === employee.id && entry.work_date === date,
        );
        openEntryDialog(ctx, { employee, date, existing: existing ?? null });
      }
    },
    { signal: abort.signal },
  );

  container.addEventListener(
    'change',
    (event) => {
      const input = event.target.closest('input[data-role="date"]');
      if (input && /^\d{4}-\d{2}-\d{2}$/.test(input.value)) setDate(input.value);
    },
    { signal: abort.signal },
  );

  function setDate(next) {
    date = next;
    ctx.store.set({ selectedDate: next });
  }

  function paint() {
    const state = ctx.store.get();
    const today = isoToday();
    const employees = state.employees.filter((employee) => !employee.deleted_at);
    const dayEntries = state.entries.filter(
      (entry) => entry.work_date === date && !entry.deleted_at,
    );
    const entryByEmployee = new Map(dayEntries.map((entry) => [entry.employee_id, entry]));
    const employeeById = new Map(state.allEmployees.map((employee) => [employee.id, employee]));

    const rows = employees
      .filter((employee) => employee.active || entryByEmployee.has(employee.id))
      .sort((a, b) => a.name.localeCompare(b.name, 'en', { sensitivity: 'base' }));

    // Day sheets belonging to archived employees must still show and count.
    const archivedIds = new Set(
      dayEntries
        .map((entry) => entry.employee_id)
        .filter((id) => !employees.some((employee) => employee.id === id)),
    );
    for (const id of archivedIds) {
      const employee = employeeById.get(id);
      rows.push({
        id,
        name: employee?.name ?? 'Former employee',
        code: employee?.code ?? null,
        active: false,
        archived: true,
      });
    }

    const business = dayEntries.reduce((sum, entry) => sum + (entry.business_minor ?? 0), 0);
    const net = dayEntries.reduce((sum, entry) => sum + (entry.net_minor ?? 0), 0);
    const pendingToday = dayEntries.filter((entry) => ctx.isPending('entry', entry.id)).length;

    const head = h(
      'div',
      { class: 'view-head' },
      h(
        'div',
        {},
        h('h1', {}, date === today ? 'Today' : displayDate(date)),
        h('p', { class: 'subtitle' }, `${weekday(date)} · ${displayDate(date)}`),
      ),
      h(
        'div',
        { class: 'date-controls' },
        h(
          'button',
          { class: 'icon-btn', type: 'button', 'data-action': 'prev-day', 'aria-label': 'Previous day' },
          '‹',
        ),
        h('input', {
          type: 'date',
          value: date,
          'data-role': 'date',
          'aria-label': 'Entry date',
        }),
        h(
          'button',
          { class: 'icon-btn', type: 'button', 'data-action': 'next-day', 'aria-label': 'Next day' },
          '›',
        ),
        h('button', { class: 'btn btn-ghost', type: 'button', 'data-action': 'today' }, 'Today'),
      ),
    );

    const stats = h(
      'section',
      { class: 'stat-grid', 'aria-label': 'Day summary' },
      h(
        'div',
        { class: 'stat' },
        h('span', { class: 'stat-label' }, 'Business for the day'),
        h('strong', { class: 'stat-value' }, formatPaise(business)),
        h(
          'span',
          { class: 'stat-hint' },
          `${dayEntries.length} day sheet${dayEntries.length === 1 ? '' : 's'}`,
        ),
      ),
      h(
        'div',
        { class: 'stat' },
        h('span', { class: 'stat-label' }, 'Final money'),
        h('strong', { class: 'stat-value' }, formatPaise(net)),
        h('span', { class: 'stat-hint' }, 'after bonus, advance and deduction'),
      ),
      h(
        'div',
        { class: 'stat' },
        h('span', { class: 'stat-label' }, 'Pending sync'),
        h('strong', { class: 'stat-value stat-small' }, String(pendingToday)),
        h(
          'span',
          { class: 'stat-hint' },
          state.online ? (state.syncing ? 'syncing…' : 'waiting for sync') : 'offline',
        ),
      ),
    );

    const banner = !state.online
      ? h('div', { class: 'banner banner-warn' }, 'You are offline. Day sheets are saved on this device and will sync when connected.')
      : state.syncError
        ? h('div', { class: 'banner banner-error' }, `Sync problem: ${state.syncError}`)
        : null;

    let list;
    if (!state.employees.length) {
      list = emptyState(
        'No employees yet',
        'Add your first employee, then record the business they generate each day.',
        h(
          'button',
          { class: 'btn btn-primary', type: 'button', 'data-action': 'add-employee' },
          'Add employee',
        ),
      );
    } else if (!rows.length) {
      list = emptyState(
        'No active employees',
        'Activate an employee or add a new one to record earnings.',
        h(
          'button',
          { class: 'btn btn-primary', type: 'button', 'data-action': 'manage-employees' },
          'Manage employees',
        ),
      );
    } else {
      list = h(
        'div',
        { class: 'entry-list' },
        rows.map((employee) => {
          const entry = entryByEmployee.get(employee.id);
          const lineCount = entry?.lines?.length ?? 0;
          const side = entry
            ? h(
                'span',
                { class: 'row-side' },
                h('strong', {}, formatPaise(entry.net_minor)),
                h(
                  'span',
                  { class: 'row-sub' },
                  `${formatPaise(entry.business_minor)} business · ${lineCount} type${lineCount === 1 ? '' : 's'}`,
                ),
                h('span', { class: 'badge-row' }, statusTags(ctx, entry)),
              )
            : h('span', { class: 'row-side row-add' }, 'Add business');

          const inner = [
            h('span', { class: 'avatar', 'aria-hidden': 'true' }, initials(employee.name)),
            h(
              'span',
              { class: 'row-main' },
              h('span', { class: 'row-name' }, employee.name),
              h(
                'span',
                { class: 'row-meta' },
                `${employee.code ? `Code ${employee.code}` : 'No code'}${employee.archived ? ' · archived' : ''}`,
              ),
            ),
            side,
          ];

          if (employee.archived || employee.deleted_at) {
            return h('div', { class: 'employee-row employee-row-static' }, inner);
          }
          return h(
            'button',
            {
              class: 'employee-row',
              type: 'button',
              'data-action': 'open-entry',
              'data-employee-id': employee.id,
            },
            inner,
          );
        }),
      );
      if (!dayEntries.length) {
        list.append(
          h('p', { class: 'list-hint' }, 'No day sheets for this date yet. Tap an employee to add the business they generated.'),
        );
      }
    }

    container.replaceChildren(...[head, banner, stats, list].filter(Boolean));
  }

  paint();
  return () => {
    abort.abort();
    unsubscribe();
  };
}
