const invalid = document.querySelector("[aria-invalid='true']");
if (invalid instanceof HTMLElement) {
    invalid.focus();
}
