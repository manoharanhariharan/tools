// Render the Sunday ledger to a PNG image and share it (WhatsApp etc.) or
// download it. No external dependencies; drawing is done on an offscreen canvas.

import { formatPaise } from './format.js';

const PAD = 18;
const ROW_H = 34;
const HEADER_H = 40;
const TITLE_H = 64;

function text(ctx, value, x, y, { bold = false, right = false, color = '#17202a', size = 14 } = {}) {
  ctx.fillStyle = color;
  ctx.font = `${bold ? '700' : '400'} ${size}px "Segoe UI", Roboto, Arial, sans-serif`;
  ctx.textBaseline = 'middle';
  ctx.textAlign = right ? 'right' : 'left';
  ctx.fillText(String(value), x, y);
}

/**
 * Draw the Sunday ledger (sl no, name, previous, Sunday, current) and return a
 * canvas. Rows are [{ name, previous_minor, sunday_minor, current_minor }].
 */
export function renderLedgerImage({ title, period, rows, previousTotal, sundayTotal, currentTotal }) {
  const columns = [
    { header: 'Sl no', width: 56, right: true },
    { header: 'Name', width: 150, right: false },
    { header: 'Previous total', width: 120, right: true },
    { header: 'Sunday Earning', width: 120, right: true },
    { header: 'Current total', width: 120, right: true },
  ];
  const totalWidth = columns.reduce((sum, column) => sum + column.width, 0) + PAD * 2;
  const height = TITLE_H + HEADER_H + (rows.length + 1) * ROW_H + PAD;

  const canvas = document.createElement('canvas');
  const scale = 2; // crisp on high-DPI screens
  canvas.width = totalWidth * scale;
  canvas.height = height * scale;
  const ctx = canvas.getContext('2d');
  ctx.scale(scale, scale);
  ctx.fillStyle = '#ffffff';
  ctx.fillRect(0, 0, totalWidth, height);

  text(ctx, title, PAD, 26, { bold: true, size: 17 });
  text(ctx, period, PAD, 46, { color: '#64748b', size: 12 });

  let x = PAD;
  const headerY = TITLE_H;
  ctx.fillStyle = '#f4f6f9';
  ctx.fillRect(PAD, headerY, totalWidth - PAD * 2, HEADER_H);
  for (const column of columns) {
    text(ctx, column.header, column.right ? x + column.width : x, headerY + HEADER_H / 2, {
      bold: true,
      right: column.right,
      color: '#526273',
      size: 11,
    });
    x += column.width;
  }

  let y = headerY + HEADER_H;
  rows.forEach((row, index) => {
    if (index % 2 === 1) {
      ctx.fillStyle = '#fbfcfd';
      ctx.fillRect(PAD, y, totalWidth - PAD * 2, ROW_H);
    }
    let cellX = PAD;
    const values = [
      String(index + 1),
      row.name,
      formatPaise(row.previous_minor),
      formatPaise(row.sunday_minor),
      formatPaise(row.current_minor),
    ];
    columns.forEach((column, colIndex) => {
      const bold = colIndex === 1 || colIndex === 4;
      text(ctx, values[colIndex], column.right ? cellX + column.width : cellX, y + ROW_H / 2, {
        bold,
        right: column.right,
        size: colIndex === 1 ? 13 : 12,
      });
      cellX += column.width;
    });
    y += ROW_H;
  });

  ctx.fillStyle = '#f4f6f9';
  ctx.fillRect(PAD, y, totalWidth - PAD * 2, ROW_H);
  let totalX = PAD;
  const totalValues = ['', 'TOTAL', formatPaise(previousTotal), formatPaise(sundayTotal), formatPaise(currentTotal)];
  columns.forEach((column, colIndex) => {
    text(ctx, totalValues[colIndex], column.right ? totalX + column.width : totalX, y + ROW_H / 2, {
      bold: true,
      right: column.right,
    });
    totalX += column.width;
  });

  return canvas;
}

/** Share the canvas as a PNG file, falling back to a download. */
export async function shareOrDownload(canvas, filename) {
  const blob = await new Promise((resolve) => canvas.toBlob(resolve, 'image/png'));
  if (!blob) throw new Error('Could not render the image');
  const file = new File([blob], filename, { type: 'image/png' });

  if (navigator.share && navigator.canShare && navigator.canShare({ files: [file] })) {
    try {
      await navigator.share({ files: [file], title: 'Daybook report' });
      return { shared: true };
    } catch (error) {
      if (error?.name === 'AbortError') return { shared: false, cancelled: true };
      // Fall through to download if sharing is unavailable or fails.
    }
  }

  const url = URL.createObjectURL(blob);
  try {
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = filename;
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
  } finally {
    setTimeout(() => URL.revokeObjectURL(url), 2_000);
  }
  return { shared: false };
}
