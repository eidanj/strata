interface Props {
  target: string;
  hint?: string;
}

export function DropVeil({ target, hint }: Props) {
  return (
    <div className="veil" aria-hidden="true">
      <div>
        <p className="veil__title">Drop to add to {target}</p>
        {hint && <p className="veil__hint">{hint}</p>}
      </div>
    </div>
  );
}
