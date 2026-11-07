import { useEffect, useRef } from "react";
import { useI18n } from "../i18n";

type Props = {
  title: string;
  body: string;
  confirmLabel: string;
  onCancel: () => void;
  onConfirm: () => void;
};

export function ConfirmDialog({ title, body, confirmLabel, onCancel, onConfirm }: Props) {
  const { t } = useI18n();
  const cancelRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    cancelRef.current?.focus();
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onCancel();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);
  return (
    <div className="dialog-backdrop" role="presentation">
      <div role="dialog" aria-modal="true" aria-labelledby="confirm-title" className="dialog">
        <h3 id="confirm-title">{title}</h3>
        <p>{body}</p>
        <div className="mt-4 flex gap-3">
          <button ref={cancelRef} type="button" className="btn btn-secondary" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn btn-danger" onClick={onConfirm}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
