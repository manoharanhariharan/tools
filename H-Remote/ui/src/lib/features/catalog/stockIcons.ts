export interface StockIcon {
  id: string;
  name: string;
  svg: string;
}

const SVG_OPEN = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">';
const SVG_CLOSE = "</svg>";

function icon(body: string): string {
  return `${SVG_OPEN}${body}${SVG_CLOSE}`;
}

/**
 * Built-in connection icons. They are rendered to a 64×64 PNG in the editor so
 * the persisted `iconBase64` field keeps its existing PNG/JPEG contract.
 */
export const STOCK_ICONS: StockIcon[] = [
  {
    id: "windows",
    name: "Windows",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#0B67B2"/>' +
        '<rect x="13" y="13" width="17" height="17" rx="1.5" fill="#fff"/>' +
        '<rect x="34" y="13" width="17" height="17" rx="1.5" fill="#fff"/>' +
        '<rect x="13" y="34" width="17" height="17" rx="1.5" fill="#fff"/>' +
        '<rect x="34" y="34" width="17" height="17" rx="1.5" fill="#fff"/>',
    ),
  },
  {
    id: "linux",
    name: "Linux",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#F4C542"/>' +
        '<ellipse cx="32" cy="35" rx="15" ry="19" fill="#171B21"/>' +
        '<ellipse cx="32" cy="40" rx="10" ry="12.5" fill="#F7FAFD"/>' +
        '<circle cx="27" cy="28" r="2.6" fill="#fff"/><circle cx="37" cy="28" r="2.6" fill="#fff"/>' +
        '<circle cx="27.5" cy="28.3" r="1.2" fill="#111"/><circle cx="36.5" cy="28.3" r="1.2" fill="#111"/>' +
        '<path d="M27.5 33.5h9L32 39z" fill="#F08A24"/>' +
        '<ellipse cx="25" cy="53" rx="5.5" ry="2.4" fill="#F08A24"/>' +
        '<ellipse cx="39" cy="53" rx="5.5" ry="2.4" fill="#F08A24"/>',
    ),
  },
  {
    id: "apple",
    name: "macOS",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#ECEFF3"/>' +
        '<path d="M32 23c-3.4-5-10-4.2-13 0.3-4.3 5.6-3.2 16.4 2.5 22.6 3.1 3.3 6.4 3.3 8.5 1.1 1.2-1.2 2.8-1.2 4 0 2.1 2.2 5.4 2.2 8.5-1.1 5.7-6.2 6.8-17 2.5-22.6-3-4.5-9.6-5.3-13-0.3z" fill="#3A3F46"/>' +
        '<path d="M33.5 20.5c0.8-4.5 4-7.2 8.6-7.3-0.9 4.5-4.1 7.2-8.6 7.3z" fill="#3A3F46"/>',
    ),
  },
  {
    id: "server",
    name: "Server",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#2D4B63"/>' +
        '<rect x="13" y="12" width="38" height="12" rx="3" fill="#D8E6F2"/>' +
        '<rect x="13" y="26" width="38" height="12" rx="3" fill="#B7CDDF"/>' +
        '<rect x="13" y="40" width="38" height="12" rx="3" fill="#D8E6F2"/>' +
        '<circle cx="19" cy="18" r="2" fill="#3EA66B"/><circle cx="19" cy="32" r="2" fill="#3EA66B"/><circle cx="19" cy="46" r="2" fill="#3EA66B"/>' +
        '<rect x="25" y="17" width="20" height="2" rx="1" fill="#7890A3"/><rect x="25" y="31" width="20" height="2" rx="1" fill="#7890A3"/><rect x="25" y="45" width="20" height="2" rx="1" fill="#7890A3"/>',
    ),
  },
  {
    id: "database",
    name: "Database",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#3E6C8A"/>' +
        '<ellipse cx="32" cy="18" rx="17" ry="6" fill="#DCEAF5"/>' +
        '<path d="M15 18v27c0 3.3 7.6 6 17 6s17-2.7 17-6V18" fill="#B8D2E5"/>' +
        '<path d="M15 30c0 3.3 7.6 6 17 6s17-2.7 17-6" fill="none" stroke="#7FA8C4" stroke-width="2.5"/>' +
        '<path d="M15 41c0 3.3 7.6 6 17 6s17-2.7 17-6" fill="none" stroke="#7FA8C4" stroke-width="2.5"/>',
    ),
  },
  {
    id: "cloud",
    name: "Cloud",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#2C6E8F"/>' +
        '<path d="M20 44h25a9 9 0 0 0 1-17.9A13 13 0 0 0 21 29a8 8 0 0 0-1 15z" fill="#EAF4FB"/>' +
        '<path d="M23 37h19" stroke="#7EAAC9" stroke-width="2.4" stroke-linecap="round"/>',
    ),
  },
  {
    id: "globe",
    name: "Web",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#2E7D6B"/>' +
        '<circle cx="32" cy="32" r="20" fill="#E4F4EE"/>' +
        '<ellipse cx="32" cy="32" rx="9" ry="20" fill="none" stroke="#4B9B87" stroke-width="2.4"/>' +
        '<path d="M12 32h40M15 21h34M15 43h34" stroke="#4B9B87" stroke-width="2.4" fill="none"/>',
    ),
  },
  {
    id: "terminal",
    name: "Terminal",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#161C22"/>' +
        '<rect x="8" y="13" width="48" height="38" rx="5" fill="#0C1218" stroke="#3E5C72" stroke-width="2"/>' +
        '<path d="M17 25l8 7-8 7" fill="none" stroke="#5CE39B" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>' +
        '<path d="M30 40h15" stroke="#5CE39B" stroke-width="3" stroke-linecap="round"/>',
    ),
  },
  {
    id: "container",
    name: "Container",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#236A8C"/>' +
        '<path d="M16 32l16-9 16 9-16 9z" fill="#D9EEF9"/>' +
        '<path d="M16 32v10l16 9V41z" fill="#9CC9E1"/>' +
        '<path d="M48 32v10l-16 9V41z" fill="#B9DCEF"/>' +
        '<path d="M32 13l8 4.5-8 4.5-8-4.5z" fill="#EAF6FC"/>',
    ),
  },
  {
    id: "shield",
    name: "Firewall",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#6A3E8C"/>' +
        '<path d="M32 9l18 7v15c0 11-7.5 19-18 24-10.5-5-18-13-18-24V16z" fill="#EFE4FA"/>' +
        '<path d="M23 28l6 6 13-13" fill="none" stroke="#6A3E8C" stroke-width="3.4" stroke-linecap="round" stroke-linejoin="round"/>',
    ),
  },
  {
    id: "vm",
    name: "Virtual machine",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#3B5470"/>' +
        '<rect x="10" y="18" width="34" height="24" rx="3" fill="#BBD2E6"/>' +
        '<rect x="20" y="24" width="34" height="24" rx="3" fill="#E7F1FA" stroke="#3B5470" stroke-width="2"/>' +
        '<rect x="26" y="31" width="22" height="10" rx="1.5" fill="#7FA6C5"/>',
    ),
  },
  {
    id: "desktop",
    name: "Desktop",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#455A64"/>' +
        '<rect x="9" y="13" width="46" height="30" rx="4" fill="#E8F1F6"/>' +
        '<rect x="14" y="18" width="36" height="20" rx="2" fill="#82AEC8"/>' +
        '<path d="M26 43v6h12v-6M20 51h24" fill="none" stroke="#E8F1F6" stroke-width="3" stroke-linecap="round"/>',
    ),
  },
];

export function stockIconDataUrl(icon: StockIcon): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(icon.svg)}`;
}

/** Rasterises a stock SVG into the PNG base64 contract used by servers. */
export async function stockIconPngBase64(icon: StockIcon): Promise<string> {
  const image = new Image();
  image.decoding = "async";
  image.src = stockIconDataUrl(icon);
  await image.decode();
  const canvas = document.createElement("canvas");
  canvas.width = 64;
  canvas.height = 64;
  const context = canvas.getContext("2d");
  if (!context) {
    throw new Error("This system cannot render stock icons.");
  }
  context.clearRect(0, 0, 64, 64);
  context.drawImage(image, 0, 0, 64, 64);
  return canvas.toDataURL("image/png").split(",")[1] ?? "";
}
