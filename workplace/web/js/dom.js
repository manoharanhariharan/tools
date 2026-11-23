// Small DOM toolkit. Untrusted strings always become text nodes; the helper
// deliberately has no innerHTML escape hatch.

export function h(tag, props = {}, ...children) {
  const element = document.createElement(tag);
  for (const [key, value] of Object.entries(props ?? {})) {
    if (value === null || value === undefined || value === false) continue;
    if (key === 'class') element.className = value;
    else if (key === 'text') element.textContent = String(value);
    else if (key === 'dataset') Object.assign(element.dataset, value);
    else if (key === 'style' && typeof value === 'object') Object.assign(element.style, value);
    else if (key === 'on') {
      for (const [type, handler] of Object.entries(value)) element.addEventListener(type, handler);
    } else if (key === 'attrs') {
      for (const [name, attrValue] of Object.entries(value)) {
        if (attrValue !== null && attrValue !== undefined && attrValue !== false) {
          element.setAttribute(name, String(attrValue));
        }
      }
    } else if (['checked', 'disabled', 'hidden', 'required', 'selected', 'readonly', 'multiple'].includes(key)) {
      element[key] = Boolean(value);
    } else if (key in element && typeof element[key] === 'boolean') {
      element[key] = Boolean(value);
    } else if (key === 'value') {
      element.value = value;
    } else if (key === 'for') {
      element.htmlFor = value;
    } else if (key === 'html') {
      throw new Error('Raw HTML is not allowed; pass text or nodes instead.');
    } else {
      element.setAttribute(key, String(value));
    }
  }
  append(element, children);
  return element;
}

export function append(parent, children) {
  for (const child of children.flat(4)) {
    if (child === null || child === undefined || child === false) continue;
    parent.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
  return parent;
}

export function clear(element) {
  while (element.firstChild) element.removeChild(element.firstChild);
  return element;
}

export function badge(text, kind = 'neutral') {
  return h('span', { class: `badge badge-${kind}` }, text);
}

export function spinner(label = 'Loading…') {
  return h(
    'div',
    { class: 'loading', role: 'status', 'aria-live': 'polite' },
    h('span', { class: 'spinner', 'aria-hidden': 'true' }),
    label,
  );
}

export function emptyState(title, message, action = null) {
  return h(
    'div',
    { class: 'state state-empty' },
    h('h2', {}, title),
    message ? h('p', {}, message) : null,
    action,
  );
}

export function errorState(message, retry = null) {
  return h(
    'div',
    { class: 'state state-error', role: 'alert' },
    h('h2', {}, 'Something went wrong'),
    h('p', {}, message),
    retry
      ? h('button', { class: 'btn', type: 'button', on: { click: retry } }, 'Try again')
      : null,
  );
}

/* ---------------------------------------------------------------- toasts */

const MAX_TOASTS = 4;
const toastTimers = new Set();

function toastRoot() {
  let root = document.getElementById('toast-root');
  if (!root) {
    root = h('div', { id: 'toast-root', class: 'toast-root', 'aria-live': 'polite' });
    document.body.append(root);
  }
  return root;
}

export function toast(message, kind = 'info', { timeout = 4_500, action = null } = {}) {
  const root = toastRoot();
  while (root.children.length >= MAX_TOASTS) root.firstElementChild?.remove();

  const element = h(
    'div',
    { class: `toast toast-${kind}`, role: kind === 'error' ? 'alert' : 'status' },
    h('span', { class: 'toast-text' }, message),
    action
      ? h(
          'button',
          {
            class: 'toast-action',
            type: 'button',
            on: {
              click: () => {
                action.run();
                dismiss();
              },
            },
          },
          action.label,
        )
      : null,
    h(
      'button',
      {
        class: 'toast-close',
        type: 'button',
        'aria-label': 'Dismiss',
        on: { click: () => dismiss() },
      },
      '×',
    ),
  );

  let timer = null;
  function dismiss() {
    if (timer) {
      clearTimeout(timer);
      toastTimers.delete(timer);
      timer = null;
    }
    element.remove();
  }

  root.append(element);
  timer = setTimeout(dismiss, timeout);
  toastTimers.add(timer);
  return dismiss;
}

/* ---------------------------------------------------------------- dialogs */

/**
 * Open a modal dialog. Returns { close, dialog, body, footer }.
 * The element is removed on close; listeners live on the dialog itself, so
 * there is nothing left behind after removal.
 */
export function openDialog({ title, description = null, content, actions = [], onClose = null }) {
  const body = h('div', { class: 'dialog-body' });
  if (content) append(body, [content]);

  const footer = h('div', { class: 'dialog-actions' });
  for (const action of actions) {
    footer.append(
      h(
        'button',
        {
          type: 'button',
          class: `btn ${action.kind ? `btn-${action.kind}` : ''}`.trim(),
          on: {
            click: async () => {
              if (action.onClick) {
                const result = await action.onClick();
                if (result === false) return;
              }
              if (action.close !== false) close();
            },
          },
        },
        action.label,
      ),
    );
  }

  const dialog = h(
    'dialog',
    { class: 'dialog', 'aria-label': title },
    h(
      'header',
      { class: 'dialog-head' },
      h('h2', {}, title),
      h(
        'button',
        {
          type: 'button',
          class: 'icon-btn',
          'aria-label': 'Close dialog',
          on: { click: () => close() },
        },
        '×',
      ),
    ),
    description ? h('p', { class: 'dialog-description' }, description) : null,
    body,
    footer.children.length ? footer : null,
  );

  let closed = false;
  function close() {
    if (closed) return;
    closed = true;
    dialog.close();
    dialog.remove();
    onClose?.();
  }

  dialog.addEventListener('cancel', (event) => {
    event.preventDefault();
    close();
  });
  document.body.append(dialog);
  dialog.showModal();

  // Focus the first non-close control for keyboard users.
  const focusTarget = dialog.querySelector('input, select, textarea, .btn');
  focusTarget?.focus?.();

  return { dialog, body, footer, close };
}

export function confirmDialog({
  title,
  message,
  confirmLabel = 'Confirm',
  cancelLabel = 'Cancel',
  danger = false,
}) {
  return new Promise((resolve) => {
    let resolved = false;
    const settle = (value) => {
      if (resolved) return;
      resolved = true;
      resolve(value);
    };
    const dialog = openDialog({
      title,
      content: h('p', {}, message),
      actions: [
        {
          label: cancelLabel,
          onClick: () => {
            settle(false);
          },
        },
        {
          label: confirmLabel,
          kind: danger ? 'danger' : 'primary',
          onClick: () => {
            settle(true);
          },
        },
      ],
      onClose: () => settle(false),
    });
    dialog.dialog.addEventListener('close', () => settle(false), { once: true });
  });
}
