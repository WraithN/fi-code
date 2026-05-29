import type { Components } from 'react-markdown';

/**
 * react-markdown 自定义组件映射
 * 为每个 Markdown 元素应用与项目主题一致的 Tailwind 样式
 */

export const markdownComponents: Components = {
  h1: ({ children }) => (
    <h1 className="text-xl font-bold text-gray-100 mt-4 mb-2 border-b border-tauri-border pb-1">
      {children}
    </h1>
  ),
  h2: ({ children }) => (
    <h2 className="text-lg font-bold text-gray-100 mt-4 mb-2 border-b border-tauri-border pb-1">
      {children}
    </h2>
  ),
  h3: ({ children }) => (
    <h3 className="text-base font-semibold text-gray-100 mt-3 mb-1.5">
      {children}
    </h3>
  ),
  h4: ({ children }) => (
    <h4 className="text-sm font-semibold text-gray-100 mt-3 mb-1.5">
      {children}
    </h4>
  ),
  p: ({ children }) => (
    <p className="my-2">{children}</p>
  ),
  ul: ({ children }) => (
    <ul className="list-disc list-inside my-2 space-y-1">{children}</ul>
  ),
  ol: ({ children }) => (
    <ol className="list-decimal list-inside my-2 space-y-1">{children}</ol>
  ),
  li: ({ children }) => (
    <li className="text-gray-200">{children}</li>
  ),
  a: ({ href, children }) => (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="text-tauri-primary hover:text-tauri-secondary underline transition-colors"
    >
      {children}
    </a>
  ),
  strong: ({ children }) => (
    <strong className="font-bold text-gray-100">{children}</strong>
  ),
  em: ({ children }) => (
    <em className="italic text-gray-300">{children}</em>
  ),
  del: ({ children }) => (
    <del className="line-through text-gray-500">{children}</del>
  ),
  code: ({ children }) => (
    <code className="bg-tauri-dark/80 text-tauri-primary px-1.5 py-0.5 rounded text-xs font-mono">
      {children}
    </code>
  ),
  blockquote: ({ children }) => (
    <blockquote className="border-l-4 border-tauri-primary/50 pl-4 py-1 my-2 bg-tauri-dark/30 rounded-r italic text-gray-300">
      {children}
    </blockquote>
  ),
  table: ({ children }) => (
    <table className="w-full border-collapse my-3 text-xs">{children}</table>
  ),
  thead: ({ children }) => (
    <thead className="bg-tauri-dark/60">{children}</thead>
  ),
  th: ({ children }) => (
    <th className="border border-tauri-border px-3 py-2 text-left font-semibold text-gray-100">
      {children}
    </th>
  ),
  td: ({ children }) => (
    <td className="border border-tauri-border px-3 py-2 text-left">{children}</td>
  ),
  tr: ({ children }) => (
    <tr className="even:bg-tauri-dark/20">{children}</tr>
  ),
  hr: () => (
    <hr className="border-tauri-border my-4" />
  ),
  pre: ({ children }) => (
    <pre className="bg-tauri-dark/80 p-3 rounded-lg overflow-x-auto my-2 text-xs font-mono">
      {children}
    </pre>
  ),
};
