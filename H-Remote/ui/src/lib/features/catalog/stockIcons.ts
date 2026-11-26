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
  {
    id: "network",
    name: "Network",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#2F5D7C"/>' +
        '<path d="M32 17v10M16 47l12-12M48 47L36 35" stroke="#DCEBF6" stroke-width="3" stroke-linecap="round"/>' +
        '<circle cx="32" cy="15" r="5" fill="#DCEBF6"/><circle cx="15" cy="49" r="5" fill="#DCEBF6"/><circle cx="49" cy="49" r="5" fill="#DCEBF6"/>' +
        '<circle cx="32" cy="33" r="6" fill="#8FD0FF"/>',
    ),
  },
  {
    id: "switch",
    name: "Switch",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#31536B"/>' +
        '<rect x="9" y="24" width="46" height="18" rx="4" fill="#E2EFF8"/>' +
        '<circle cx="18" cy="33" r="2.4" fill="#3EA66B"/><circle cx="26" cy="33" r="2.4" fill="#3EA66B"/>' +
        '<path d="M34 33h14M41 28l5 5-5 5" fill="none" stroke="#5F8AA8" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/>',
    ),
  },
  {
    id: "wifi",
    name: "Wi-Fi",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#2B6C75"/>' +
        '<path d="M12 27a28 28 0 0 1 40 0M19 35a18 18 0 0 1 26 0M26 43a8 8 0 0 1 12 0" fill="none" stroke="#E3F7F4" stroke-width="4" stroke-linecap="round"/>' +
        '<circle cx="32" cy="50" r="3.5" fill="#8FE3D6"/>',
    ),
  },
  {
    id: "printer",
    name: "Printer",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#4B5D6A"/>' +
        '<rect x="17" y="12" width="30" height="16" rx="2" fill="#EAF2F7"/>' +
        '<rect x="10" y="27" width="44" height="20" rx="4" fill="#B9CFDD"/>' +
        '<rect x="19" y="39" width="26" height="14" rx="2" fill="#EAF2F7"/>' +
        '<circle cx="47" cy="33" r="2" fill="#49B675"/>',
    ),
  },
  {
    id: "nas",
    name: "NAS / Storage",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#3E5268"/>' +
        '<rect x="14" y="11" width="36" height="13" rx="3" fill="#DCE8F2"/>' +
        '<rect x="14" y="26" width="36" height="13" rx="3" fill="#C5D9E8"/>' +
        '<rect x="14" y="41" width="36" height="13" rx="3" fill="#DCE8F2"/>' +
        '<circle cx="21" cy="17.5" r="2" fill="#3EA66B"/><circle cx="21" cy="32.5" r="2" fill="#3EA66B"/><circle cx="21" cy="47.5" r="2" fill="#3EA66B"/>',
    ),
  },
  {
    id: "backup",
    name: "Backup",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#5A4A7A"/>' +
        '<path d="M49 32a17 17 0 1 1-5-12" fill="none" stroke="#F0E9FA" stroke-width="3.4" stroke-linecap="round"/>' +
        '<path d="M45 12v9h-9" fill="none" stroke="#F0E9FA" stroke-width="3.4" stroke-linecap="round" stroke-linejoin="round"/>' +
        '<ellipse cx="32" cy="35" rx="10" ry="4" fill="#C9B6E8"/>' +
        '<path d="M22 35v9c0 2.2 4.5 4 10 4s10-1.8 10-4v-9" fill="#C9B6E8"/>',
    ),
  },
  {
    id: "docker",
    name: "Docker",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#1C6E9E"/>' +
        '<rect x="13" y="31" width="7" height="7" fill="#E6F5FC"/><rect x="22" y="31" width="7" height="7" fill="#E6F5FC"/>' +
        '<rect x="31" y="31" width="7" height="7" fill="#E6F5FC"/><rect x="22" y="22" width="7" height="7" fill="#E6F5FC"/>' +
        '<rect x="31" y="22" width="7" height="7" fill="#E6F5FC"/><rect x="40" y="31" width="7" height="7" fill="#E6F5FC"/>' +
        '<path d="M10 42h39c3 0 5-2 5-5-3 0-5 1-6 2 0-5-4-8-9-8" fill="none" stroke="#E6F5FC" stroke-width="3" stroke-linecap="round"/>',
    ),
  },
  {
    id: "kubernetes",
    name: "Kubernetes",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#2C4C8C"/>' +
        '<path d="M32 9l18 10v20L32 55 14 39V19z" fill="#DCE7FA"/>' +
        '<circle cx="32" cy="32" r="10" fill="none" stroke="#2C4C8C" stroke-width="2.6"/>' +
        '<path d="M32 22v20M22 32h20M25 25l14 14M39 25 25 39" stroke="#2C4C8C" stroke-width="2.2"/>',
    ),
  },
  {
    id: "git",
    name: "Git",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#5B3B2E"/>' +
        '<path d="M20 46V24a8 8 0 0 1 8-8h8" fill="none" stroke="#F6E4D8" stroke-width="3" stroke-linecap="round"/>' +
        '<path d="M36 32v14" fill="none" stroke="#F6E4D8" stroke-width="3" stroke-linecap="round"/>' +
        '<circle cx="20" cy="50" r="5" fill="#F6E4D8"/><circle cx="36" cy="50" r="5" fill="#F6E4D8"/><circle cx="36" cy="18" r="5" fill="#F6E4D8"/>',
    ),
  },
  {
    id: "proxy",
    name: "Proxy",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#3D5A6C"/>' +
        '<rect x="10" y="20" width="16" height="12" rx="2" fill="#DCEAF5"/>' +
        '<rect x="38" y="32" width="16" height="12" rx="2" fill="#DCEAF5"/>' +
        '<path d="M26 26h8a6 6 0 0 1 6 6v0M38 38h-8a6 6 0 0 1-6-6v0" fill="none" stroke="#8FD0FF" stroke-width="3" stroke-linecap="round"/>' +
        '<path d="M30 22l4 4-4 4M34 34l-4 4 4 4" fill="none" stroke="#8FD0FF" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>',
    ),
  },
  {
    id: "camera",
    name: "Camera",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#43505A"/>' +
        '<rect x="11" y="22" width="42" height="26" rx="5" fill="#E7EEF4"/>' +
        '<path d="M24 22l3-5h10l3 5" fill="#C2D1DD"/>' +
        '<circle cx="32" cy="35" r="8" fill="#6E8EA8"/><circle cx="32" cy="35" r="4" fill="#E7EEF4"/>',
    ),
  },
  {
    id: "phone",
    name: "Phone",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#3F4D63"/>' +
        '<rect x="21" y="9" width="22" height="46" rx="5" fill="#E8EEF6"/>' +
        '<rect x="25" y="16" width="14" height="27" rx="2" fill="#8FB0CE"/>' +
        '<circle cx="32" cy="49" r="2.4" fill="#3F4D63"/>',
    ),
  },
  {
    id: "laptop",
    name: "Laptop",
    svg: icon(
      '<rect width="64" height="64" rx="13" fill="#4A5560"/>' +
        '<rect x="14" y="16" width="36" height="24" rx="3" fill="#EAF0F5"/>' +
        '<rect x="19" y="21" width="26" height="14" rx="1.5" fill="#7FA6C5"/>' +
        '<path d="M10 44h44l-3 6H13z" fill="#C5D3DE"/>',
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
