// HTTP client for the Daybook API. All errors are normalised to ApiError so
// views can show field-level messages and the app can detect expired sessions.

const CLIENT_ID = 'daybook-web/1';
let unauthorizedHandler = null;

export class ApiError extends Error {
  constructor(status, payload = null, message = null) {
    super(
      message ??
        payload?.error?.message ??
        (status === 0 ? 'Cannot reach the server' : `Request failed (${status})`),
    );
    this.name = 'ApiError';
    this.status = status;
    this.code = payload?.error?.code ?? null;
    this.fields = payload?.error?.fields ?? [];
    this.existing = payload?.error?.existing ?? null;
  }
}

export function setUnauthorizedHandler(handler) {
  unauthorizedHandler = handler;
}

function query(params) {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params ?? {})) {
    if (value === null || value === undefined || value === '') continue;
    search.set(key, String(value));
  }
  return search.toString();
}

async function toApiError(response) {
  let payload = null;
  try {
    const text = await response.text();
    payload = text ? JSON.parse(text) : null;
  } catch {
    // Non-JSON error body (proxy, crash): keep the status and a generic message.
  }
  return new ApiError(response.status, payload);
}

async function request(path, { method = 'GET', body, signal, raw = false } = {}) {
  const headers = { 'X-Daybook-Client': CLIENT_ID };
  if (body !== undefined) headers['Content-Type'] = 'application/json';

  let response;
  try {
    response = await fetch(path, {
      method,
      headers,
      credentials: 'same-origin',
      cache: 'no-store',
      body: body === undefined ? undefined : JSON.stringify(body),
      signal,
    });
  } catch (error) {
    if (error?.name === 'AbortError') throw error;
    throw new ApiError(0, null, 'Cannot reach the server. Check Wi-Fi and try again.');
  }

  if (!response.ok) {
    const apiError = await toApiError(response);
    if (
      response.status === 401 &&
      !path.startsWith('/api/auth/login') &&
      typeof unauthorizedHandler === 'function'
    ) {
      unauthorizedHandler();
    }
    throw apiError;
  }
  if (raw) return response;

  const text = await response.text();
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    throw new ApiError(response.status, null, 'Unexpected response from the server');
  }
}

export function login(password, signal) {
  return request('/api/auth/login', { method: 'POST', body: { password }, signal });
}

export function loginEmployee(code, pin, signal) {
  return request('/api/auth/login/employee', { method: 'POST', body: { code, pin }, signal });
}

export function logout() {
  return request('/api/auth/logout', { method: 'POST' });
}

export function changePassword(body) {
  return request('/api/auth/password', { method: 'POST', body });
}

export function me(signal) {
  return request('/api/auth/me', { signal });
}

export function listEmployees(signal) {
  return request('/api/employees', { signal });
}

export function listEarningTypes(signal) {
  return request('/api/earning-types', { signal });
}

export function createEarningType(body) {
  return request('/api/earning-types', { method: 'POST', body });
}

export function updateEarningType(id, body) {
  return request(`/api/earning-types/${encodeURIComponent(id)}`, {
    method: 'PATCH',
    body,
  });
}

export function deleteEarningType(id) {
  return request(`/api/earning-types/${encodeURIComponent(id)}`, { method: 'DELETE' });
}

export function listEntries({ from, to, employeeId } = {}, signal) {
  return request(`/api/entries?${query({ from, to, employee_id: employeeId })}`, { signal });
}

export function createEntry(body) {
  return request('/api/entries', { method: 'POST', body });
}

export function updateEntry(id, body) {
  return request(`/api/entries/${encodeURIComponent(id)}`, { method: 'PATCH', body });
}

export function deleteEntry(id) {
  return request(`/api/entries/${encodeURIComponent(id)}`, { method: 'DELETE' });
}

export function employeePinStatus(id) {
  return request(`/api/employees/${encodeURIComponent(id)}/pin`);
}

export function setEmployeePin(id, pin) {
  return request(`/api/employees/${encodeURIComponent(id)}/pin`, { method: 'PUT', body: { pin } });
}

export function clearEmployeePin(id) {
  return request(`/api/employees/${encodeURIComponent(id)}/pin`, { method: 'DELETE' });
}

export function syncState(signal) {
  return request('/api/sync/state', { signal });
}

export function pull(since, limit = 200, signal) {
  return request(`/api/sync/pull?${query({ since, limit })}`, { signal });
}

export function push(changes, deviceId, signal) {
  return request('/api/sync/push', {
    method: 'POST',
    body: { device_id: deviceId ?? null, changes },
    signal,
  });
}

export function reportSummary({ from, to, employeeId } = {}, signal) {
  return request(`/api/reports/summary?${query({ from, to, employee_id: employeeId })}`, {
    signal,
  });
}

export function openPrintReport({ from, to, employeeId, layout }) {
  const url = `/api/reports/print.html?${query({
    from,
    to,
    employee_id: employeeId,
    layout,
  })}`;
  window.open(url, '_blank', 'noopener');
}

/** Download a PDF/CSV via fetch so errors can be shown as toasts. */
export async function downloadReport(kind, { from, to, employeeId, layout }) {
  const path =
    kind === 'pdf' ? '/api/reports/report.pdf' : '/api/reports/export.csv';
  const extension = kind === 'pdf' ? 'pdf' : 'csv';
  const response = await request(
    `${path}?${query({ from, to, employee_id: employeeId, layout })}`,
    { raw: true },
  );
  const blob = await response.blob();
  const disposition = response.headers.get('content-disposition') ?? '';
  const match = /filename="([^"]+)"/.exec(disposition);
  const filename = match?.[1] ?? `daybook-${extension}`;

  const url = URL.createObjectURL(blob);
  try {
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = filename;
    anchor.rel = 'noopener';
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
  } finally {
    // Bounded cleanup: the browser needs the URL alive for the click to start.
    setTimeout(() => URL.revokeObjectURL(url), 2_000);
  }
}
