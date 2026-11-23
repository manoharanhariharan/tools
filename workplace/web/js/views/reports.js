// Reports: on-screen summary plus print/PDF/CSV downloads. Summary data comes
// from the server so totals are authoritative.

import { h, emptyState, errorState, spinner, toast } from '../dom.js';
import {
  currentMonth,
  displayDate,
  formatHours,
  formatPaise,
  monthBounds,
} from '../format.js';
import { basisLabel } from '../pay.js';

const LAYOUTS = [
  ['employee', 'By employee'],
  ['daily', 'By day'],
  ['entries', 'Entry detail'],
];

export function renderReports(root, ctx) {
  const container = h('section', { class: 'view' });
  root.replaceChildren(container);
  let month = currentMonth();
  let layout = 'employee';
  let requestController = null;
  let requestToken = 0;

  const monthInput = h('input', {
    type: 'month',
    value: month,
    'aria-label': 'Report month',
  });
  const layoutSelect = h(
    'select',
    { 'aria-label': 'Download layout' },
    LAYOUTS.map(([value, label]) => h('option', { value }, label)),
  );
  const monthError = h('p', { class: 'field-error', role: 'alert' });
  const region = h('div', { class: 'report-region', 'aria-live': 'polite' });

  const actions = h(
    'div',
    { class: 'button-row' },
    h('button', { class: 'btn btn-primary', type: 'button', 'data-action': 'print' }, 'View & print'),
    h('button', { class: 'btn', type: 'button', 'data-action': 'pdf' }, 'Download PDF'),
    h('button', { class: 'btn', type: 'button', 'data-action': 'csv' }, 'Download CSV'),
  );

  container.append(
    h(
      'div',
      { class: 'view-head' },
      h(
        'div',
        {},
        h('h1', {}, 'Reports'),
        h('p', { class: 'subtitle' }, 'Daily, monthly and per-employee earnings summaries'),
      ),
    ),
    h(
      'div',
      { class: 'filters' },
      h('div', { class: 'field' }, h('label', {}, h('span', { class: 'field-label' }, 'Month'), monthInput), monthError),
      h('div', { class: 'field' }, h('label', {}, h('span', { class: 'field-label' }, 'Layout'), layoutSelect)),
    ),
    actions,
    h('p', { class: 'field-hint' }, 'Printable HTML opens in a new tab. The server generates PDF, CSV and totals.'),
    region,
  );

  function currentRange() {
    return monthBounds(month);
  }

  function ensureOnline() {
    if (!ctx.store.get().online) {
      toast('Reports are generated on the server. Reconnect to continue.', 'error');
      return false;
    }
    return true;
  }

  async function download(kind) {
    if (!ensureOnline()) return;
    const range = currentRange();
    if (!range) return;
    try {
      await ctx.api.downloadReport(kind, {
        from: range.from,
        to: range.to,
        layout,
      });
      toast(kind === 'pdf' ? 'PDF downloaded' : 'CSV downloaded', 'success');
    } catch (error) {
      toast(error?.message ?? 'Download failed', 'error');
    }
  }

  async function load() {
    const range = currentRange();
    if (!range) {
      monthError.textContent = 'Choose a valid month';
      return;
    }
    monthError.textContent = '';
    requestToken += 1;
    const token = requestToken;
    requestController?.abort();
    requestController = new AbortController();

    if (!ctx.store.get().online) {
      region.replaceChildren(
        errorState(
          'Reports need a connection to the server. Your entries are safe on this device and will sync when you are back online.',
          () => load(),
        ),
      );
      return;
    }

    region.replaceChildren(spinner('Loading report…'));
    try {
      const data = await ctx.api.reportSummary(
        { from: range.from, to: range.to },
        requestController.signal,
      );
      if (token !== requestToken) return;
      renderReport(data, range);
    } catch (error) {
      if (error?.name === 'AbortError' || token !== requestToken) return;
      region.replaceChildren(errorState(error?.message ?? 'Could not load the report', () => load()));
    }
  }

  function renderReport(data, range) {
    if (!data.totals.entries) {
      region.replaceChildren(
        emptyState(
          'No entries in this period',
          `${displayDate(range.from)} to ${displayDate(range.to)} has no recorded earnings.`,
        ),
      );
      return;
    }

    const totals = data.totals;
    const stats = h(
      'section',
      { class: 'stat-grid', 'aria-label': 'Report totals' },
      stat('Gross earnings', formatPaise(totals.gross_minor), `${totals.entries} entries`),
      stat('Advances', formatPaise(totals.advance_minor), 'deducted'),
      stat('Deductions', formatPaise(totals.deduction_minor), 'deducted'),
      stat('Net payout', formatPaise(totals.net_minor), 'to pay'),
      stat(
        'Hours',
        `${formatHours(totals.regular_minutes)} / ${formatHours(totals.overtime_minutes)}`,
        'regular / overtime',
      ),
    );

    const employeeTable = h(
      'div',
      { class: 'table-wrap' },
      h(
        'table',
        { class: 'data-table' },
        h(
          'thead',
          {},
          h(
            'tr',
            {},
            h('th', { scope: 'col' }, 'Employee'),
            h('th', { scope: 'col' }, 'Basis'),
            h('th', { scope: 'col', class: 'num' }, 'Entries'),
            h('th', { scope: 'col', class: 'num' }, 'Net'),
          ),
        ),
        h(
          'tbody',
          {},
          data.employees.map((employee) =>
            h(
              'tr',
              {},
              h('th', { scope: 'row' }, employee.name),
              h('td', {}, basisLabel(employee.pay_basis)),
              h('td', { class: 'num' }, String(employee.totals.entries)),
              h('td', { class: 'num strong' }, formatPaise(employee.totals.net_minor)),
            ),
          ),
        ),
      ),
    );

    const dailyTable = h(
      'div',
      { class: 'table-wrap' },
      h(
        'table',
        { class: 'data-table' },
        h(
          'thead',
          {},
          h(
            'tr',
            {},
            h('th', { scope: 'col' }, 'Date'),
            h('th', { scope: 'col', class: 'num' }, 'Entries'),
            h('th', { scope: 'col', class: 'num' }, 'Gross'),
            h('th', { scope: 'col', class: 'num' }, 'Net'),
          ),
        ),
        h(
          'tbody',
          {},
          data.days.map((day) =>
            h(
              'tr',
              {},
              h('th', { scope: 'row' }, displayDate(day.work_date)),
              h('td', { class: 'num' }, String(day.totals.entries)),
              h('td', { class: 'num' }, formatPaise(day.totals.gross_minor)),
              h('td', { class: 'num strong' }, formatPaise(day.totals.net_minor)),
            ),
          ),
        ),
      ),
    );

    region.replaceChildren(
      stats,
      h('h2', { class: 'section-title' }, 'By employee'),
      employeeTable,
      h('h2', { class: 'section-title' }, 'Daily totals'),
      dailyTable,
    );
  }

  function stat(label, value, hint) {
    return h(
      'div',
      { class: 'stat' },
      h('span', { class: 'stat-label' }, label),
      h('strong', { class: 'stat-value' }, value),
      h('span', { class: 'stat-hint' }, hint),
    );
  }

  monthInput.addEventListener('change', () => {
    month = monthInput.value;
    load();
  });
  layoutSelect.addEventListener('change', () => {
    layout = layoutSelect.value;
  });
  container.addEventListener('click', (event) => {
    const control = event.target.closest('[data-action]');
    if (!control) return;
    if (control.dataset.action === 'print') {
      if (!ensureOnline()) return;
      const range = currentRange();
      if (range) ctx.api.openPrintReport({ from: range.from, to: range.to, layout });
    } else if (control.dataset.action === 'pdf') {
      void download('pdf');
    } else if (control.dataset.action === 'csv') {
      void download('csv');
    }
  });

  const unsubscribe = ctx.store.subscribe((state) => {
    if (!state.online && !region.querySelector('.report-loading')) {
      // If we went offline after loading, keep the last report visible but warn.
      if (!container.querySelector('.banner-offline')) {
        actions.before(
          h(
            'div',
            { class: 'banner banner-warn banner-offline' },
            'You are offline. Showing the last loaded report; exports and refreshes need a connection.',
          ),
        );
      }
    } else if (state.online) {
      container.querySelector('.banner-offline')?.remove();
    }
  });

  load();

  return () => {
    requestController?.abort();
    unsubscribe();
  };
}
