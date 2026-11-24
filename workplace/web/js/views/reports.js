// Reports: on-screen summary plus print/PDF/CSV downloads. Summary data comes
// from the server so totals are authoritative.

import { h, emptyState, errorState, spinner, toast } from '../dom.js';
import {
  currentMonth,
  daysInRange,
  displayDate,
  formatPaise,
  monthBounds,
} from '../format.js';

const LAYOUTS = [
  ['sunday', 'Sunday ledger'],
  ['employee', 'By employee'],
  ['daily', 'By day'],
  ['entries', 'Entry detail'],
];

function emptyRow(columns, message) {
  return h('tr', {}, h('td', { colspan: String(columns), class: 'muted' }, message));
}

export function renderReports(root, ctx) {
  const container = h('section', { class: 'view' });
  root.replaceChildren(container);
  let month = currentMonth();
  const initialRange = monthBounds(month);
  let from = initialRange.from;
  let to = initialRange.to;
  let layout = 'sunday';
  let requestController = null;
  let requestToken = 0;

  const monthInput = h('input', {
    type: 'month',
    value: month,
    'aria-label': 'Quick-pick month',
  });
  const fromInput = h('input', {
    type: 'date',
    value: from,
    'aria-label': 'Report start date',
  });
  const toInput = h('input', {
    type: 'date',
    value: to,
    'aria-label': 'Report end date',
  });
  const layoutSelect = h(
    'select',
    { 'aria-label': 'Download layout' },
    LAYOUTS.map(([value, label]) => h('option', { value }, label)),
  );
  layoutSelect.value = layout;
  const rangeError = h('p', { class: 'field-error', role: 'alert' });
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
        h('p', { class: 'subtitle' }, 'Daily, date-range and per-employee earnings summaries'),
      ),
    ),
    h(
      'div',
      { class: 'filters' },
      h(
        'div',
        { class: 'field' },
        h('label', {}, h('span', { class: 'field-label' }, 'Month (quick pick)'), monthInput),
      ),
      h(
        'div',
        { class: 'field' },
        h('label', {}, h('span', { class: 'field-label' }, 'From'), fromInput),
      ),
      h(
        'div',
        { class: 'field' },
        h('label', {}, h('span', { class: 'field-label' }, 'To'), toInput),
      ),
      h(
        'div',
        { class: 'field' },
        h('label', {}, h('span', { class: 'field-label' }, 'Layout'), layoutSelect),
      ),
    ),
    rangeError,
    actions,
    h(
      'p',
      { class: 'field-hint' },
      'Any range up to 366 days. Printable HTML opens in a new tab; PDF, CSV and totals are generated on the server.',
    ),
    region,
  );

  function currentRange() {
    const days = daysInRange(from, to);
    if (days === null || days > 366) return null;
    return { from, to };
  }

  function validatedRange() {
    const range = currentRange();
    if (!range) {
      rangeError.textContent =
        from > to
          ? '“From” must be on or before “To”.'
          : 'Choose a valid date range of at most 366 days.';
      return null;
    }
    rangeError.textContent = '';
    return range;
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
    const range = validatedRange();
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
    const range = validatedRange();
    if (!range) return;
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
    const ledger = data.ledger ?? [];
    if (!data.totals.entries && !ledger.length) {
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
      stat('Business generated', formatPaise(totals.business_minor), `${totals.entries} day sheets`),
      stat('Bonus', formatPaise(totals.bonus_minor), 'added'),
      stat('Advances', formatPaise(totals.advance_minor), 'deducted'),
      stat('Deductions', formatPaise(totals.deduction_minor), 'deducted'),
      stat('Final money', formatPaise(totals.net_minor), 'to present'),
    );

    const previousTotal = Number.isSafeInteger(data.previous_total_minor)
      ? data.previous_total_minor
      : ledger.reduce((sum, row) => sum + row.previous_minor, 0);
    const sundayTotal = Number.isSafeInteger(data.sunday_total_minor)
      ? data.sunday_total_minor
      : ledger.reduce((sum, row) => sum + row.sunday_minor, 0);
    const currentTotal = Number.isSafeInteger(data.current_total_minor)
      ? data.current_total_minor
      : ledger.reduce((sum, row) => sum + row.current_minor, 0);

    const ledgerTable = h(
      'div',
      { class: 'table-wrap' },
      h(
        'table',
        { class: 'data-table ledger-table' },
        h(
          'thead',
          {},
          h(
            'tr',
            {},
            h('th', { scope: 'col', class: 'num' }, 'Sl no'),
            h('th', { scope: 'col' }, 'Name'),
            h('th', { scope: 'col', class: 'num' }, 'Previous total'),
            h('th', { scope: 'col', class: 'num' }, 'Sunday Earning'),
            h('th', { scope: 'col', class: 'num' }, 'Current total'),
          ),
        ),
        h(
          'tbody',
          {},
          ledger.map((row, index) =>
            h(
              'tr',
              {},
              h('td', { class: 'num' }, String(index + 1)),
              h('th', { scope: 'row' }, row.name),
              h('td', { class: 'num' }, formatPaise(row.previous_minor)),
              h('td', { class: 'num' }, formatPaise(row.sunday_minor)),
              h('td', { class: 'num strong' }, formatPaise(row.current_minor)),
            ),
          ),
          h(
            'tr',
            { class: 'total-row' },
            h('td'),
            h('th', { scope: 'row' }, 'TOTAL'),
            h('td', { class: 'num' }, formatPaise(previousTotal)),
            h('td', { class: 'num' }, formatPaise(sundayTotal)),
            h('td', { class: 'num strong' }, formatPaise(currentTotal)),
          ),
        ),
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
            h('th', { scope: 'col', class: 'num' }, 'Sheets'),
            h('th', { scope: 'col', class: 'num' }, 'Business'),
            h('th', { scope: 'col', class: 'num' }, 'Final'),
          ),
        ),
        h(
          'tbody',
          {},
          data.employees.length
            ? data.employees.map((employee) =>
                h(
                  'tr',
                  {},
                  h('th', { scope: 'row' }, employee.name),
                  h('td', { class: 'num' }, String(employee.totals.entries)),
                  h('td', { class: 'num' }, formatPaise(employee.totals.business_minor)),
                  h('td', { class: 'num strong' }, formatPaise(employee.totals.net_minor)),
                ),
              )
            : emptyRow(4, 'No day sheets in this period.'),
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
            h('th', { scope: 'col', class: 'num' }, 'Sheets'),
            h('th', { scope: 'col', class: 'num' }, 'Business'),
            h('th', { scope: 'col', class: 'num' }, 'Final'),
          ),
        ),
        h(
          'tbody',
          {},
          data.days.length
            ? data.days.map((day) =>
                h(
                  'tr',
                  {},
                  h('th', { scope: 'row' }, displayDate(day.work_date)),
                  h('td', { class: 'num' }, String(day.totals.entries)),
                  h('td', { class: 'num' }, formatPaise(day.totals.business_minor)),
                  h('td', { class: 'num strong' }, formatPaise(day.totals.net_minor)),
                ),
              )
            : emptyRow(4, 'No day sheets in this period.'),
        ),
      ),
    );

    const typeTable = h(
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
            h('th', { scope: 'col' }, 'Earning type'),
            h('th', { scope: 'col', class: 'num' }, 'Business'),
          ),
        ),
        h(
          'tbody',
          {},
          (data.types ?? []).length
            ? (data.types ?? []).map((entryType) =>
                h(
                  'tr',
                  {},
                  h('th', { scope: 'row' }, entryType.type_name),
                  h('td', { class: 'num strong' }, formatPaise(entryType.amount_minor)),
                ),
              )
            : emptyRow(2, 'No business recorded in this period.'),
        ),
      ),
    );

    region.replaceChildren(
      stats,
      h('h2', { class: 'section-title' }, 'Sunday ledger'),
      ledgerTable,
      h('h2', { class: 'section-title' }, 'By employee'),
      employeeTable,
      h('h2', { class: 'section-title' }, 'By day'),
      dailyTable,
      h('h2', { class: 'section-title' }, 'Business by earning type'),
      typeTable,
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
    const bounds = monthBounds(month);
    if (!bounds) return;
    from = bounds.from;
    to = bounds.to;
    fromInput.value = from;
    toInput.value = to;
    load();
  });
  fromInput.addEventListener('change', () => {
    from = fromInput.value;
    monthInput.value = '';
    load();
  });
  toInput.addEventListener('change', () => {
    to = toInput.value;
    monthInput.value = '';
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
      const range = validatedRange();
      if (range) ctx.api.openPrintReport({ from: range.from, to: range.to, layout });
    } else if (control.dataset.action === 'pdf') {
      void download('pdf');
    } else if (control.dataset.action === 'csv') {
      void download('csv');
    }
  });

  let wasOffline = !ctx.store.get().online;
  const unsubscribe = ctx.store.subscribe((state) => {
    if (!state.online) {
      // Keep the last loaded report visible but warn that it may be stale.
      if (!container.querySelector('.banner-offline')) {
        actions.before(
          h(
            'div',
            { class: 'banner banner-warn banner-offline' },
            'You are offline. Showing the last loaded report; exports and refreshes need a connection.',
          ),
        );
      }
      wasOffline = true;
    } else {
      container.querySelector('.banner-offline')?.remove();
      if (wasOffline) {
        wasOffline = false;
        load();
      }
    }
  });

  load();

  return () => {
    requestController?.abort();
    unsubscribe();
  };
}
