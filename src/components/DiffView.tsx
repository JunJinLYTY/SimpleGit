interface DiffViewProps {
  diff: string;
  loading: boolean;
  error?: string;
}

export function DiffView({ diff, loading, error }: DiffViewProps) {
  if (loading) {
    return <div className="diff-view diff-muted">正在读取差异…</div>;
  }
  if (error) {
    return (
      <div className="diff-view diff-error" role="alert">
        {error}
      </div>
    );
  }
  if (!diff.trim()) {
    return (
      <div className="diff-view diff-muted">
        该文件没有可显示的文本差异，可能是二进制文件或未跟踪文件。
      </div>
    );
  }

  return (
    <pre className="diff-view" aria-label="文件差异">
      {diff.split("\n").map((line, index) => (
        <span
          className={
            line.startsWith("+")
              ? "diff-line diff-add"
              : line.startsWith("-")
                ? "diff-line diff-remove"
                : "diff-line"
          }
          key={`${index}-${line}`}
        >
          {line}
          {"\n"}
        </span>
      ))}
    </pre>
  );
}
