// Employee "My reports": the signed-in employee's own Sunday ledger and day
// sheets for any date range, with PNG share/print/PDF/CSV export.

import { h, emptyState, errorState, spinner, toast } from '../dom.js';
import { currentMonth, daysInRange, displayDate, formatPaise, monthBounds } from '../format.js';
import { renderLedgerImage, shareOrDownload } from '../share-image.js';

export function renderEmployeeReports(root, ctx) {
  const container = h('section', { class: 'view' });
  root.replaceChildren(container);
  let month = currentMonth();
  const initialRange = monthBounds(month);
  let from = initialRange.from;
  let to = initialRange.to;
  let requestController = null;
  let requestToken = 0;
  let currentData = null;
  let currentRange = null;

  const monthInput = h('input', { type: 'month', value: month, 'aria-label': 'Quick-pick month' });
  const fromInput = h('input', { type: 'date', value: from, 'aria-label': 'Report start date' });
  const toInput = h('input', { type: 'date', value: to, 'aria-label': 'Report end date' });
  const rangeError = h('p', { class: 'field-error', role: 'alert' });
  const region = h('div', { class: 'report-region', 'aria-live': 'polite' });

  const actions = h(
    'div',
    { class: 'button-row' },
    h('button', { class: 'btn btn-primary', type: 'button', 'data-action': 'share' }, 'Share as image'),
    h('button', { class: 'btn', type: 'button', 'data-action': 'print' }, 'View & print'),
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
        h('h1', {}, 'My reports'),
        h('p', { class: 'subtitle' }, `${ctx.state.employeeName || 'Your'} earnings and balances`),
      ),
    ),
    h(
      'div',
      { class: 'filters' },
      h('div', { class: 'field' }, h('label', {}, h('span', { class: 'field-label' }, 'Month (quick pick)'), monthInput)),
      h('div', { class: 'field' }, h('label', {}, h('span', { class: 'field-label' }, 'From'), fromInput)),
      h('div', { class: 'field' }, h('label', {}, h('span', { class: 'field-label' }, 'To'), toInput)),
    ),
    rangeError,
    actions,
    region,
  );

  function validatedRange() {
    const days = daysInRange(from, to);
    if (days === null || days > 366) {
      rangeError.textContent = from > to ? '“From” must be on or before “To”.' : 'Choose a valid date range of at most 366 days.';
      return null;
    }
    rangeError.textContent = '';
    return { from, to };
  }

  function ensureOnline() {
    if (!navigator.onLine) {
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
      await ctx.api.downloadReport(kind, { from: range.from, to: range.to, employeeId: ctx.state.employeeId, layout: 'sunday' });
      toast(kind === 'pdf' ? 'PDF downloaded' : 'CSV downloaded', 'success');
    } catch (error) {
      toast(error?.message ?? 'Download failed', 'error');
    }
  }

  async function shareImage(data, range) {
    if (!data?.ledger?.length) return;
    const rows = data.ledger.map((row) => ({
      name: row.name,
      previous_minor: row.previous_minor,
      sunday_minor: row.sunday_minor,
      current_minor: row.current_minor,
    }));
    const canvas = renderLedgerImage({
      title: 'Daybook — my Sunday ledger',
      period: `${displayDate(range.from)} to ${displayDate(range.to)}`,
      rows,
      previousTotal: data.previous_total_minor,
      sundayTotal: data.sunday_total_minor,
      currentTotal: data.current_total_minor,
    });
    try {
      const result = await shareOrDownload(canvas, `daybook-${range.from}-${range.to}.png`);
      if (!result.shared && !result.cancelled) toast('Image downloaded', 'success');
    } catch (error) {
      toast(error?.message ?? 'Could not create the image', 'error');
    }
  }

  async function load() {
    const range = validatedRange();
    if (!range) return;
    if (!ensureOnline()) return;
    requestToken += 1;
    const token = requestToken;
    requestController?.abort();
    requestController = new AbortController();

    region.replaceChildren(spinner('Loading report…'));
    try {
      const data = await ctx.api.reportSummary(
        { from: range.from, to: range.to, employeeId: ctx.state.employeeId },
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
    const lines = data.lines ?? [];
    if (!ledger.length && !lines.length) {
      region.replaceChildren(emptyState('No earnings in this period', `${displayDate(range.from)} to ${displayDate(range.to)} has nothing recorded for you.`));
      return;
    }

    const stats = h(
      'section',
      { class: 'stat-grid', 'aria-label': 'My report totals' },
      stat('Previous total', formatPaise(data.previous_total_minor ?? 0), 'before this period'),
      stat('Sunday Earning', formatPaise(data.sunday_total_minor ?? 0), 'Sundays in range'),
      stat('Current total', formatPaise(data.current_total_minor ?? 0), 'your balance'),
    );

    const ledgerTable = h(
      'div',
      { class: 'table-wrap' },
      h(
        'table',
        { class: 'data-table ledger-table' },
        h(
          'thead',
          {},
          h('tr', {}, h('th', { scope: 'col' }, 'Name'), h('th', { scope: 'col', class: 'num' }, 'Previous total'), h('th', { scope: 'col', class: 'num' }, 'Sunday Earning'), h('th', { scope: 'col', class: 'num' }, 'Current total')),
        ),
        h(
          'tbody',
          {},
          ledger.map((row) =>
            h(
              'tr',
              {},
              h('th', { scope: 'row' }, row.name),
              h('td', { class: 'num' }, formatPaise(row.previous_minor)),
              h('td', { class: 'num' }, formatPaise(row.sunday_minor)),
              h('td', { class: 'num strong' }, formatPaise(row.current_minor)),
            ),
          ),
        ),
      ),
    );

    const dayTable = h(
      'div',
      { class: 'table-wrap' },
      h(
        'table',
        { class: 'data-table' },
        h('thead', {}, h('tr', {}, h('th', { scope: 'col' }, 'Date'), h('th', { scope: 'col', class: 'num' }, 'Business'), h('th', { scope: 'col', class: 'num' }, 'Final money'))),
        h(
          'tbody',
          {},
          lines.length
            ? lines.map((line) =>
                h(
                  'tr',
                  {},
                  h('th', { scope: 'row' }, displayDate(line.work_date)),
                  h('td', { class: 'num' }, formatPaise(line.business_minor)),
                  h('td', { class: 'num strong' }, formatPaise(line.net_minor)),
                ),
              )
            : h('tr', {}, h('td', { colspan: '3', class: 'muted' }, 'No day sheets in this period.')),
        ),
      ),
    );

    region.replaceChildren(stats, h('h2', { class: 'section-title' }, 'My Sunday ledger'), ledgerTable, h('h2', { class: 'section-title' }, 'My day sheets'), dayTable);
    currentData = data;
    currentRange = range;
  }

  function stat(label, value, hint) {
    return h('div', { class: 'stat' }, h('span', { class: 'stat-label' }, label), h('strong', { class: 'stat-value' }, value), h('span', { class: 'stat-hint' }, hint));
  }

  monthInput.addEventListener('change', () => {
    month = monthInput.value;
    const bounds = monthBounds(month);
    if (!bounds) return;
    from = bounds.from;
    to = bounds.to;
    fromInput.value = from;
    toInput.value = to;
    void load();
  });
  fromInput.addEventListener('change', () => {
    from = fromInput.value;
    monthInput.value = '';
    void load();
  });
  toInput.addEventListener('change', () => {
    to = toInput.value;
    monthInput.value = '';
    void load();
  });

  container.addEventListener('click', (event) => {
    const control = event.target.closest('[data-action]');
    if (!control) return;
    const range = validatedRange();
    if (control.dataset.action === 'share') {
      if (!range || !currentData) return;
      void shareImage(currentData, currentRange);
    } else if (control.dataset.action === 'print') {
      if (range) ctx.api.openPrintReport({ from: range.from, to: range.to, employeeId: ctx.state.employeeId, layout: 'sunday' });
    } else if (control.dataset.action === 'pdf') {
      void download('pdf');
    } else if (control.dataset.action === 'csv') {
      void download('csv');
    }
  });

  void load();
  return () => {
    requestController?.abort();
  };
}
