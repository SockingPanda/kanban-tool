import React from 'react';
const paths = {
  panelCollapse: ['M3 4h18v16H3zM9 4v16M16 9l-3 3 3 3'],
  panelExpand: ['M3 4h18v16H3zM9 4v16M13 9l3 3-3 3'],
  map: ['M3 5l6-2 6 2 6-2v16l-6 2-6-2-6 2V5', 'M9 3v16M15 5v16'],
  layers: ['M12 3L2 8l10 5 10-5-10-5', 'M2 12l10 5 10-5M2 16l10 5 10-5'],
  task: ['M9 6h11M9 12h11M9 18h11', 'M3 6l1 1 2-2M3 12l1 1 2-2M3 18l1 1 2-2'],
  cycle: ['M20 7a9 9 0 1 0 1 9', 'M20 2v5h-5'],
  grid: ['M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z'],
  list: ['M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01'],
  plus: ['M12 5v14M5 12h14'], close: ['M6 6l12 12M18 6L6 18'],
  search: ['M21 21l-5-5', 'M17 10a7 7 0 1 1-14 0 7 7 0 0 1 14 0'],
  chevron: ['M9 5l7 7-7 7'], down: ['M6 9l6 6 6-6'], left: ['M15 5l-7 7 7 7'],
  arrow: ['M5 12h14M13 6l6 6-6 6'], arrowUp: ['M7 17L17 7M7 7h10v10'],
  more: ['M5 12h.01M12 12h.01M19 12h.01'], filter: ['M4 6h16M7 12h10M10 18h4'],
  check: ['M5 12l4 4L19 6'], checkCircle: ['M22 11.1V12a10 10 0 1 1-5.9-9.1', 'M22 4L12 14l-3-3'],
  circle: ['M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0'],
  active: ['M12 3a9 9 0 1 0 9 9', 'M12 7v5l3 2'],
  warning: ['M12 3L2 21h20L12 3', 'M12 9v4M12 17h.01'],
  shield: ['M12 3l8 3v6c0 5-8 9-8 9s-8-4-8-9V6l8-3', 'M8 12l3 3 5-5'],
  file: ['M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z', 'M14 2v6h6M8 13h8M8 17h6'],
  clock: ['M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0', 'M12 7v5l3 2'],
  settings: ['M4 7h16M4 17h16', 'M8 4v6M16 14v6'],
  moon: ['M21 13a9 9 0 1 1-10-10 7 7 0 0 0 10 10'], sun: ['M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5L19 19M5 19l1.5-1.5M17.5 6.5L19 5', 'M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0'],
  box: ['M3 7l9-5 9 5v10l-9 5-9-5V7', 'M3 7l9 5 9-5M12 12v10M7.5 4.5l9 5'],
  folder: ['M3 7V4h6l2 3h10v13H3z'],
  link: ['M10 13a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-2 2', 'M14 11a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l2-2'],
  unlink: ['M3 3l18 18M9 5l2-2a5 5 0 0 1 7 7l-1 1M7 13l-3 3a5 5 0 0 0 7 7l2-2'],
  branch: ['M6 3v12a6 6 0 0 0 12 0V9', 'M9 6a3 3 0 1 1-6 0 3 3 0 0 1 6 0M21 6a3 3 0 1 1-6 0 3 3 0 0 1 6 0'],
  tree: ['M12 3v6M5 14V9h14v5M12 9v5', 'M2 14h6v6H2zM9 14h6v6H9zM16 14h6v6h-6z'],
  database: ['M20 6c0 2-4 3-8 3S4 8 4 6s4-3 8-3 8 1 8 3', 'M4 6v12c0 2 4 3 8 3s8-1 8-3V6M4 12c0 2 4 3 8 3s8-1 8-3'],
  code: ['M8 6l-6 6 6 6M16 6l6 6-6 6M14 3l-4 18'],
  pulse: ['M2 12h5l3-8 4 16 3-8h5'],
  star: ['M12 3l3 6 7 1-5 5 1 7-6-3-6 3 1-7-5-5 7-1z'],
  calendar: ['M4 5h16v16H4zM16 3v4M8 3v4M4 11h16'],
  comment: ['M21 15a3 3 0 0 1-3 3H8l-5 4V6a3 3 0 0 1 3-3h12a3 3 0 0 1 3 3v9', 'M7 8h10M7 12h7'],
  flag: ['M4 22V3M4 3c5-4 10 4 16 0v12c-6 4-11-4-16 0'],
  archive: ['M3 3h18v5H3zM5 8v13h14V8M9 12h6'],
  trash: ['M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7'],
  edit: ['M16 3l5 5L9 20l-6 1 1-6L16 3M14 5l5 5'],
  download: ['M12 3v12M7 10l5 5 5-5M4 16v5h16v-5'], upload: ['M12 15V3M7 8l5-5 5 5M4 16v5h16v-5'],
  reset: ['M3 10a9 9 0 1 1 2 9', 'M3 3v7h7'],
  zoomIn: ['M21 21l-5-5', 'M17 10a7 7 0 1 1-14 0 7 7 0 0 1 14 0M7 10h6M10 7v6'],
  minus: ['M5 12h14'], fit: ['M8 3H3v5M16 3h5v5M21 16v5h-5M3 16v5h5'],
  panel: ['M3 3h18v18H3zM9 3v18'], play: ['M7 3l14 9-14 9V3'],
  pause: ['M7 4v16M17 4v16'], lock: ['M5 10h14v11H5zM8 10V6a4 4 0 0 1 8 0v4'],
  eye: ['M2 12s4-7 10-7 10 7 10 7-4 7-10 7-10-7-10-7', 'M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0'],
  book: ['M4 3h16v18H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2M4 17h16'],
  command: ['M9 7V4a2 2 0 1 0-2 2h10a2 2 0 1 0-2-2v16a2 2 0 1 0 2-2H7a2 2 0 1 0 2 2V7'],
  dot: ['M12 12h.01'], user: ['M16 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0', 'M4 22v-3a8 8 0 0 1 16 0v3'],
  grip: ['M9 5h.01M15 5h.01M9 12h.01M15 12h.01M9 19h.01M15 19h.01'],
  timeline: ['M4 4v16M9 5h11v4H9zM7 13h9v4H7z'], spark: ['M12 3l3 6 6 3-6 3-3 6-3-6-6-3 6-3z']
};
export type IconName = keyof typeof paths;
export function Icon({ name, size = 18, className = '', ...props }: {
  name: IconName;
  size?: number;
  className?: string;
} & React.SVGProps<SVGSVGElement>) {
  return <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" className={className} {...props}>
    {(paths[name] ?? paths.box).map((d, i) => <path d={d} key={i} />)}
  </svg>;
}
