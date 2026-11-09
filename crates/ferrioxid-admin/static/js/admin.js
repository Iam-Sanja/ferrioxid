const invalid = document.querySelector("[aria-invalid='true']");
if (invalid instanceof HTMLElement) {
    invalid.focus();
}

function fingerprint(form) {
    const skip = new Set(["csrf_token", "save", "confirm"]);
    const parts = [];
    const data = new FormData(form);
    for (const [key, value] of data.entries()) {
        if (skip.has(key)) {
            continue;
        }
        parts.push(`${key}=${String(value)}`);
    }
    parts.sort();
    return parts.join("&");
}

function track(form) {
    const mode = form.getAttribute("data-track-dirty");
    if (!mode) {
        return;
    }
    const buttons = [...form.querySelectorAll(".js-save")];
    if (buttons.length === 0) {
        return;
    }
    const initial = fingerprint(form);
    const forced = mode === "record" && form.querySelector(".field-error, [role='alert']") !== null;
    const apply = () => {
        delete form.dataset.submitting;
        const dirty = forced || fingerprint(form) !== initial;
        for (const button of buttons) {
            button.disabled = !dirty;
        }
        form.dataset.dirty = dirty ? "1" : "0";
    };
    apply();
    form.addEventListener("input", apply);
    form.addEventListener("change", apply);
    // A successful native submit may navigate away. Do not warn that the form
    // has unsaved changes when the user explicitly chose to save them.
    form.addEventListener("submit", (event) => {
        if (!event.defaultPrevented && form.checkValidity()) {
            form.dataset.submitting = "1";
        }
    });
    form.addEventListener("invalid", () => {
        delete form.dataset.submitting;
    }, true);
}

for (const form of document.querySelectorAll("form[data-track-dirty]")) {
    track(form);
}

window.addEventListener("beforeunload", (event) => {
    if (document.querySelector("form[data-dirty='1']:not([data-submitting='1'])") === null) {
        return;
    }
    event.preventDefault();
    event.returnValue = "";
});

// Table row navigation is progressive enhancement. The first cell remains
// a normal keyboard-accessible link. Native controls must keep their behavior.
document.addEventListener("click", (event) => {
    if (!(event.target instanceof Element)) {
        return;
    }
    if (event.target.closest("a, button, input, select, textarea, label, summary, details, [role='button'], [contenteditable='true']")) {
        return;
    }
    const row = event.target.closest("tr[data-record-href]");
    if (row instanceof HTMLTableRowElement && row.dataset.recordHref) {
        window.location.assign(row.dataset.recordHref);
    }
});
