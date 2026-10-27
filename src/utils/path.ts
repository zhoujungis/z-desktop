/** 计算路径的父目录，兼容 Windows 盘符、UNC 与 POSIX 路径。 */
export function parentPath(input: string): string {
  const trimmed = input.trim().replace(/[\\/]+$/, "");
  if (!trimmed) return "/";

  // UNC：\\server\share\...
  if (trimmed.startsWith("\\\\")) {
    const parts = trimmed.slice(2).split(/[\\/]+/).filter(Boolean);
    if (parts.length <= 2) return `\\\\${parts.join("\\")}`;
    return `\\\\${parts.slice(0, -1).join("\\")}`;
  }

  // Windows 盘符：C:\ / C:\Users
  const drive = trimmed.match(/^([A-Za-z]):(.*)$/);
  if (drive) {
    const rest = drive[2];
    if (!rest) return `${drive[1]}:\\`;
    const idx = Math.max(rest.lastIndexOf("\\"), rest.lastIndexOf("/"));
    const parent = idx <= 0 ? "\\" : rest.slice(0, idx);
    return `${drive[1]}:${parent}`;
  }

  const idx = Math.max(trimmed.lastIndexOf("\\"), trimmed.lastIndexOf("/"));
  if (idx < 0) return ".";
  if (idx === 0) return trimmed[0] === "/" ? "/" : trimmed;
  return trimmed.slice(0, idx);
}
