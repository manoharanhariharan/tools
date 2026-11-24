// Settings: per-device appearance and the admin password change form.

import { h, spinner, toast } from '../dom.js';
import { THEMES, applyTheme, readStoredTheme, themeLabel } from '../theme.js';

export function renderSettings(root, ctx) {
  const container = h('section', { class: 'view' });
  root.replaceChildren(container);
  const abort = new AbortController();

  container.append(
    h(
      'div',
      { class: 'view-head' },
      h(
        'div',
        {},
        h('h1', {}, 'Settings'),
        h('p', { class: 'subtitle' }, 'Appearance for this device and your admin account'),
      ),
    ),
  );

  /* ---------------------------------------------------------- appearance */

  const themeSelect = h(
    'select',
    { 'aria-label': 'Theme', 'data-theme-select': '' },
    THEMES.map((theme) => h('option', { value: theme.value }, theme.label)),
  );
  themeSelect.value = readStoredTheme();
  themeSelect.addEventListener(
    'change',
    () => {
      applyTheme(themeSelect.value);
      toast(`Theme set to ${themeLabel(themeSelect.value)}`, 'success');
    },
    { signal: abort.signal },
  );

  const appearance = h(
    'section',
    { class: 'settings-card', 'aria-labelledby': 'settings-appearance' },
    h('h2', { id: 'settings-appearance' }, 'Appearance'),
    h('p', { class: 'field-hint' }, 'Saved on this device only, so each phone can look different.'),
    h(
      'div',
      { class: 'field' },
      h('label', {}, h('span', { class: 'field-label' }, 'Theme'), themeSelect),
    ),
  );

  /* ------------------------------------------------------------- account */

  const current = h('input', {
    type: 'password',
    name: 'current-password',
    autocomplete: 'current-password',
    required: true,
    maxlength: '200',
  });
  const next = h('input', {
    type: 'password',
    name: 'new-password',
    autocomplete: 'new-password',
    required: true,
    minlength: '10',
    maxlength: '200',
  });
  const confirm = h('input', {
    type: 'password',
    name: 'confirm-password',
    autocomplete: 'new-password',
    required: true,
    minlength: '10',
    maxlength: '200',
  });

  const currentError = h('p', { class: 'field-error', role: 'alert' });
  const nextError = h('p', { class: 'field-error', role: 'alert' });
  const confirmError = h('p', { class: 'field-error', role: 'alert' });
  const formError = h('p', { class: 'form-error', role: 'alert' });

  function setError(input, target, message) {
    target.textContent = message ?? '';
    if (message) input.setAttribute('aria-invalid', 'true');
    else input.removeAttribute('aria-invalid');
  }

  function clearErrors() {
    setError(current, currentError, '');
    setError(next, nextError, '');
    setError(confirm, confirmError, '');
    formError.textContent = '';
  }

  const submit = h(
    'button',
    { class: 'btn btn-primary', type: 'submit' },
    'Update password',
  );

  const form = h(
    'form',
    { class: 'settings-form', novalidate: true },
    h(
      'div',
      { class: 'field' },
      h(
        'label',
        {},
        h('span', { class: 'field-label' }, 'Current password'),
        current,
      ),
      currentError,
    ),
    h(
      'div',
      { class: 'field' },
      h('label', {}, h('span', { class: 'field-label' }, 'New password'), next),
      h('p', { class: 'field-hint' }, 'At least 10 characters, up to 200.'),
      nextError,
    ),
    h(
      'div',
      { class: 'field' },
      h(
        'label',
        {},
        h('span', { class: 'field-label' }, 'Confirm new password'),
        confirm,
      ),
      confirmError,
    ),
    formError,
    h('div', { class: 'button-row' }, submit),
  );

  form.addEventListener(
    'submit',
    async (event) => {
      event.preventDefault();
      clearErrors();

      const currentValue = current.value;
      const nextValue = next.value;
      if (!currentValue) {
        setError(current, currentError, 'Enter your current password');
        current.focus();
        return;
      }
      if (nextValue.length < 10) {
        setError(next, nextError, 'Use at least 10 characters');
        next.focus();
        return;
      }
      if (nextValue !== confirm.value) {
        setError(confirm, confirmError, 'Passwords do not match');
        confirm.focus();
        return;
      }
      if (nextValue === currentValue) {
        setError(next, nextError, 'Choose a password different from the current one');
        next.focus();
        return;
      }
      if (!ctx.store.get().online) {
        formError.textContent = 'Changing the password needs a connection to the server.';
        return;
      }

      submit.disabled = true;
      submit.replaceChildren(spinner('Updating…'));
      try {
        await ctx.api.changePassword({
          current_password: currentValue,
          new_password: nextValue,
        });
        form.reset();
        toast('Password updated. Other devices were signed out.', 'success');
      } catch (error) {
        if (error?.status === 429) {
          formError.textContent = 'Too many attempts. Wait a few minutes and try again.';
        } else if (error?.fields?.length) {
          for (const field of error.fields) {
            if (field.field === 'current_password') {
              setError(current, currentError, field.message);
            } else if (field.field === 'new_password') {
              setError(next, nextError, field.message);
            } else {
              formError.textContent = field.message;
            }
          }
        } else if (error?.status === 0) {
          formError.textContent = 'Cannot reach the server. Check Wi-Fi and try again.';
        } else {
          formError.textContent = error?.message ?? 'Could not update the password';
        }
      } finally {
        submit.disabled = false;
        submit.replaceChildren('Update password');
      }
    },
    { signal: abort.signal },
  );

  const account = h(
    'section',
    { class: 'settings-card', 'aria-labelledby': 'settings-account' },
    h('h2', { id: 'settings-account' }, 'Admin account'),
    h(
      'p',
      { class: 'field-hint' },
      'Changing the password signs out every other device. This session stays signed in.',
    ),
    form,
  );

  container.append(appearance, account);
  return () => abort.abort();
}
