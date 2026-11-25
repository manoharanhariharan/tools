// Employee "My day": the signed-in employee's own day sheets, editable online.
// Employees do not use the offline sync engine; every read/write goes to the
// server and is scoped to their own employee id on the server side.

import { confirmDialog, h, openDialog, spinner, toast } from '../dom.js';
import { addDays, displayDate, formatDecimal, formatPaise, isoToday, parsePaise } from '../format.js';
import { computeTotals } from '../totals.js';

const MAX_AMOUNT_MINOR = 100_000_000_000;
const MAX_DESCRIPTION_LEN = 300;

function readAmount(input) {
  const raw = input.value.trim();
  if (!raw) return { value: 0 };
  const paise = parsePaise(raw);
  if (paise === null) return { error: 'Enter an amount like 1250.50' };
  if (paise > MAX_AMOUNT_MINOR) return { error: 'Amount is too large' };
  return { value: paise };
}

function field(labelText, control, hintText = null) {
  const error = h('p', { class: 'field-error', role: 'alert' });
  const wrapper = h(
    'div',
    { class: 'field' },
    h('label', {}, h('span', { class: 'field-label' }, labelText), control),
    hintText ? h('p', { class: 'field-hint' }, hintText) : null,
    error,
  );
  return { wrapper, control, error };
}

export function renderEmployeeDay(root, ctx) {
  const container = h('section', { class: 'view' });
  root.replaceChildren(container);
  const abort = new AbortController();
  let date = isoToday();
  let types = [];
  let entry = null;
  let requestToken = 0;
  const region = h('div', { class: 'report-region', 'aria-live': 'polite' });

  const dateInput = h('input', { type: 'date', value: date, 'aria-label': 'Entry date' });
  const head = h(
    'div',
    { class: 'view-head' },
    h(
      'div',
      {},
      h('h1', {}, 'My day'),
      h(
        'p',
        { class: 'subtitle' },
        `${ctx.state.employeeName || 'Employee'}${ctx.state.employeeCode ? ` · Code ${ctx.state.employeeCode}` : ''}`,
      ),
    ),
    h(
      'div',
      { class: 'date-controls' },
      h('button', { class: 'icon-btn', type: 'button', 'data-action': 'prev-day', 'aria-label': 'Previous day' }, '‹'),
      dateInput,
      h('button', { class: 'icon-btn', type: 'button', 'data-action': 'next-day', 'aria-label': 'Next day' }, '›'),
      h('button', { class: 'btn btn-ghost', type: 'button', 'data-action': 'today' }, 'Today'),
    ),
  );

  container.append(head, region);

  function setDate(next) {
    date = next;
    dateInput.value = next;
    void load();
  }

  function entryPayload(lines, adjustments, note) {
    return {
      employee_id: ctx.state.employeeId,
      work_date: date,
      lines: lines.map((line) => ({
        type_id: line.type_id,
        description: line.description,
        amount_minor: line.amount_minor,
      })),
      bonus_minor: adjustments.bonus_minor,
      advance_minor: adjustments.advance_minor,
      deduction_minor: adjustments.deduction_minor,
      note,
    };
  }

  function openEntryForm(existing) {
    const existingByType = new Map((existing?.lines ?? []).map((line) => [line.type_id, line]));

    const rows = [];
    const seen = new Set();
    for (const type of types) {
      const line = existingByType.get(type.id);
      rows.push({
        type_id: type.id,
        type_name: type.name,
        archived: false,
        description: line?.description ?? '',
        amount: line?.amount_minor ?? 0,
      });
      seen.add(type.id);
    }
    for (const line of existing?.lines ?? []) {
      if (seen.has(line.type_id)) continue;
      rows.push({
        type_id: line.type_id,
        type_name: line.type_name,
        archived: true,
        description: line.description,
        amount: line.amount_minor,
      });
      seen.add(line.type_id);
    }

    const rowControls = rows.map((row) => {
      const description = h('input', {
        type: 'text',
        autocomplete: 'off',
        maxlength: String(MAX_DESCRIPTION_LEN),
        value: row.description,
        placeholder: 'What was done? (optional)',
      });
      const amount = h('input', {
        type: 'text',
        inputmode: 'decimal',
        autocomplete: 'off',
        value: row.amount ? formatDecimal(row.amount) : '',
        placeholder: '0.00',
      });
      const amountError = h('p', { class: 'field-error', role: 'alert' });
      const element = h(
        'div',
        { class: 'earning-row' },
        h(
          'div',
          { class: 'earning-row-head' },
          h('strong', {}, row.type_name),
          row.archived ? h('span', { class: 'badge badge-neutral' }, 'Archived type') : null,
        ),
        h(
          'div',
          { class: 'earning-row-fields' },
          h('label', { class: 'field' }, h('span', { class: 'field-label' }, 'Description'), description),
          h('label', { class: 'field field-amount' }, h('span', { class: 'field-label' }, 'Amount (₹)'), amount),
        ),
        amountError,
      );
      return { ...row, description, amount, amountError, element };
    });

    const bonus = h('input', { type: 'text', inputmode: 'decimal', autocomplete: 'off', value: existing ? formatDecimal(existing.bonus_minor) : '0' });
    const advance = h('input', { type: 'text', inputmode: 'decimal', autocomplete: 'off', value: existing ? formatDecimal(existing.advance_minor) : '0' });
    const deduction = h('input', { type: 'text', inputmode: 'decimal', autocomplete: 'off', value: existing ? formatDecimal(existing.deduction_minor) : '0' });
    const note = h('textarea', { rows: '2', maxlength: '2000', placeholder: 'Optional note for the day' });
    note.value = existing?.note ?? '';

    const bonusField = field('Bonus (₹)', bonus);
    const advanceField = field('Advance paid (₹)', advance, 'Deducted from the final money.');
    const deductionField = field('Deduction (₹)', deduction);
    const noteField = field('Note', note);
    const linesError = h('p', { class: 'field-error', role: 'alert' });
    const preview = h('div', { class: 'preview', 'aria-live': 'polite' });

    const typesSection = rows.length
      ? h('div', { class: 'earning-rows' }, rowControls.map((row) => row.element))
      : h(
          'div',
          { class: 'state state-empty' },
          h('h2', {}, 'No earning types yet'),
          h('p', {}, 'Your admin needs to add earning types before you can record earnings.'),
        );

    const content = h(
      'div',
      { class: 'entry-form' },
      h('p', { class: 'dialog-subtitle' }, `${displayDate(date)} · your business for the day`),
      typesSection,
      linesError,
      h('div', { class: 'form-grid' }, bonusField.wrapper, advanceField.wrapper, deductionField.wrapper),
      noteField.wrapper,
      h('div', { class: 'preview-wrap' }, h('h3', {}, 'Day total'), preview),
      h('p', { class: 'field-hint' }, 'The server recalculates these amounts; the preview is an estimate.'),
    );

    function collect() {
      const errors = {};
      const lines = [];
      let anyAmount = false;
      rowControls.forEach((row, index) => {
        const amount = readAmount(row.amount);
        if (amount.error) errors[`amount-${index}`] = amount.error;
        else if (amount.value > 0) {
          anyAmount = true;
          lines.push({ type_id: row.type_id, description: row.description.value.trim(), amount_minor: amount.value });
        }
        if (row.description.value.trim().length > MAX_DESCRIPTION_LEN) {
          errors[`description-${index}`] = 'Description is too long';
        }
      });
      if (!anyAmount) errors.lines = 'Enter at least one earning amount.';

      const adjustments = {};
      for (const [key, input] of [['bonus_minor', bonus], ['advance_minor', advance], ['deduction_minor', deduction]]) {
        const amount = readAmount(input);
        if (amount.error) errors[key] = amount.error;
        else adjustments[key] = amount.value;
      }
      return { lines, adjustments, errors };
    }

    function showErrors(errors) {
      linesError.textContent = errors.lines ?? '';
      rowControls.forEach((row, index) => {
        row.amountError.textContent = errors[`amount-${index}`] ?? errors[`description-${index}`] ?? '';
        row.amount.setAttribute('aria-invalid', errors[`amount-${index}`] ? 'true' : 'false');
        row.description.setAttribute('aria-invalid', errors[`description-${index}`] ? 'true' : 'false');
      });
      bonusField.error.textContent = errors.bonus_minor ?? '';
      advanceField.error.textContent = errors.advance_minor ?? '';
      deductionField.error.textContent = errors.deduction_minor ?? '';
    }

    function updatePreview() {
      const { lines, adjustments, errors } = collect();
      if (errors.lines || Object.keys(errors).length) {
        preview.replaceChildren(h('p', { class: 'muted' }, 'Fix the highlighted fields to see the estimate.'));
        return;
      }
      const totals = computeTotals(lines, adjustments);
      preview.replaceChildren(
        h(
          'dl',
          { class: 'preview-grid' },
          h('dt', { class: 'strong' }, 'Business'),
          h('dd', { class: 'strong' }, formatPaise(totals.business_minor)),
          h('dt', {}, 'Bonus'),
          h('dd', {}, formatPaise(totals.bonus_minor)),
          h('dt', { class: 'strong' }, 'Gross'),
          h('dd', { class: 'strong' }, formatPaise(totals.gross_minor)),
          h('dt', {}, 'Advance'),
          h('dd', {}, `− ${formatPaise(totals.advance_minor)}`),
          h('dt', {}, 'Deduction'),
          h('dd', {}, `− ${formatPaise(totals.deduction_minor)}`),
          h('dt', { class: 'strong' }, 'Final money'),
          h('dd', { class: 'strong net' }, formatPaise(totals.net_minor)),
        ),
      );
    }

    content.addEventListener('input', updatePreview);
    content.addEventListener('change', updatePreview);
    updatePreview();

    const actions = [];
    if (existing) {
      actions.push({
        label: 'Delete',
        kind: 'danger',
        onClick: async () => {
          const confirmed = await confirmDialog({
            title: 'Delete this day sheet?',
            message: `Remove your earnings for ${displayDate(date)}?`,
            confirmLabel: 'Delete day',
            danger: true,
          });
          if (!confirmed) return false;
          try {
            await ctx.api.deleteEntry(existing.id);
            toast('Day deleted', 'success');
            await load();
          } catch (error) {
            toast(error?.message ?? 'Could not delete the day', 'error');
            return false;
          }
        },
      });
    }
    actions.push({
      label: existing ? 'Save changes' : 'Save day',
      kind: 'primary',
      onClick: async () => {
        const { lines, adjustments, errors } = collect();
        showErrors(errors);
        if (Object.keys(errors).length) {
          toast('Please fix the highlighted fields', 'error');
          return false;
        }
        try {
          const payload = entryPayload(lines, adjustments, note.value.trim());
          if (existing) await ctx.api.updateEntry(existing.id, payload);
          else await ctx.api.createEntry(payload);
          toast(existing ? 'Day updated' : 'Day saved', 'success');
          await load();
        } catch (error) {
          if (error?.code === 'duplicate_entry_date') {
            toast('A day sheet already exists for this date', 'error');
          } else {
            toast(error?.message ?? 'Could not save the day', 'error');
          }
          return false;
        }
      },
    });

    openDialog({
      title: existing ? 'Edit my day earnings' : 'Add my day earnings',
      content,
      actions,
    });
  }

  async function load() {
    requestToken += 1;
    const token = requestToken;
    region.replaceChildren(spinner('Loading…'));
    try {
      const [typesResult, entries] = await Promise.all([
        ctx.api.listEarningTypes(),
        ctx.api.listEntries({ from: date, to: date, employeeId: ctx.state.employeeId }, abort.signal),
      ]);
      if (token !== requestToken) return;
      types = (typesResult ?? []).filter((type) => type.active !== false);
      entry = (entries ?? []).find((candidate) => candidate.work_date === date && !candidate.deleted_at) ?? null;
      paint();
    } catch (error) {
      if (error?.name === 'AbortError') return;
      region.replaceChildren(
        h(
          'div',
          { class: 'state state-error' },
          h('h2', {}, 'Something went wrong'),
          h('p', {}, error?.message ?? 'Could not load your day'),
          h('button', { class: 'btn', type: 'button', on: { click: () => load() } }, 'Try again'),
        ),
      );
    }
  }

  function paint() {
    if (!entry) {
      region.replaceChildren(
        h(
          'div',
          { class: 'state' },
          h('h2', {}, `No day sheet for ${displayDate(date)}`),
          h('p', {}, 'Record the business you generated today.'),
          h(
            'button',
            { class: 'btn btn-primary', type: 'button', on: { click: () => openEntryForm(null) } },
            'Add my earnings',
          ),
        ),
      );
      return;
    }

    const lineCount = entry.lines?.length ?? 0;
    region.replaceChildren(
      h(
        'div',
        { class: 'stat-grid', 'aria-label': 'My day summary' },
        h(
          'div',
          { class: 'stat' },
          h('span', { class: 'stat-label' }, 'Business'),
          h('strong', { class: 'stat-value' }, formatPaise(entry.business_minor)),
          h('span', { class: 'stat-hint' }, `${lineCount} earning type${lineCount === 1 ? '' : 's'}`),
        ),
        h(
          'div',
          { class: 'stat' },
          h('span', { class: 'stat-label' }, 'Final money'),
          h('strong', { class: 'stat-value' }, formatPaise(entry.net_minor)),
          h('span', { class: 'stat-hint' }, 'after bonus, advance and deduction'),
        ),
      ),
      h(
        'div',
        { class: 'table-wrap' },
        h(
          'table',
          { class: 'data-table' },
          h('thead', {}, h('tr', {}, h('th', { scope: 'col' }, 'Earning type'), h('th', { scope: 'col', class: 'num' }, 'Amount'))),
          h(
            'tbody',
            {},
            (entry.lines ?? []).map((line) =>
              h(
                'tr',
                {},
                h('th', { scope: 'row' }, line.type_name),
                h('td', { class: 'num' }, formatPaise(line.amount_minor)),
              ),
            ),
          ),
        ),
      ),
      entry.note
        ? h('div', { class: 'info-block' }, h('p', { class: 'muted' }, `Note: ${entry.note}`))
        : null,
      h(
        'div',
        { class: 'button-row' },
        h('button', { class: 'btn btn-primary', type: 'button', on: { click: () => openEntryForm(entry) } }, 'Edit my earnings'),
      ),
    );
  }

  container.addEventListener('click', (event) => {
    const control = event.target.closest('[data-action]');
    if (!control) return;
    if (control.dataset.action === 'prev-day') setDate(addDays(date, -1));
    else if (control.dataset.action === 'next-day') setDate(addDays(date, 1));
    else if (control.dataset.action === 'today') setDate(isoToday());
  });

  dateInput.addEventListener('change', () => {
    if (/^\d{4}-\d{2}-\d{2}$/.test(dateInput.value)) setDate(dateInput.value);
  });

  void load();
  return () => abort.abort();
}
