"""Browser regressions against HTML captured from the actual Rust HTTP integration test.

Generate a fresh fixture directory with FEXU_BROWSER_FIXTURES set while running
canonical_editors_preserve_permissions_readonly_fields_roles_and_navigation.
Requires the development-only playwright package and Chromium (or --browser-executable).
"""
import argparse
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--browser-executable")
    args = parser.parse_args()
    from playwright.sync_api import sync_playwright, expect

    root = Path(__file__).resolve().parents[1]
    routes = json.loads((args.fixtures / "routes.json").read_text())
    pages = {
        routes["model"]: "list.html", routes["record"]: "record.html",
        "/readonly": "readonly.html", "/admin/directory/users": "users.html",
        routes["user"]: "user.html", "/admin/directory/roles": "roles.html",
        routes["role"]: "role.html",
    }
    assets = root / "crates/fexu-admin/static"

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            path = urlsplit(self.path).path
            if path in pages:
                body = (args.fixtures / pages[path]).read_bytes()
                kind = "text/html"
            elif path in ("/admin/assets/admin.js", "/admin/assets/admin.css"):
                body = (assets / ("js/admin.js" if path.endswith(".js") else "css/admin.css")).read_bytes()
                kind = "application/javascript" if path.endswith(".js") else "text/css"
            else:
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", kind + "; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_args):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    origin = f"http://127.0.0.1:{server.server_port}"
    args.artifacts.mkdir(parents=True, exist_ok=True)
    try:
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch(executable_path=args.browser_executable)
            context = browser.new_context(viewport={"width": 1440, "height": 1000})
            page = context.new_page()
            errors = []
            page.on("pageerror", lambda error: errors.append(str(error)))
            page.goto(origin + routes["model"])
            row = page.locator(f'tr[data-record-href="{routes["record"]}"]')
            expect(row).to_have_attribute("data-record-href", routes["record"])
            assert row.get_attribute("role") is None
            checkbox = row.locator("input[name=ids]")
            checkbox.check()
            checkbox.focus()
            page.keyboard.press("Space")
            expect(checkbox).not_to_be_checked()
            assert page.url == origin + routes["model"]
            row.locator("summary").click()
            expect(row.locator("details")).to_have_attribute("open", "")
            assert page.url == origin + routes["model"]
            row.locator("summary").click()
            cell = row.locator("td.is-lead")
            # Cancelled clicks, modifier clicks and selected text must not navigate.
            cell.evaluate("el => el.addEventListener('click', event => event.preventDefault(), {capture:true, once:true})")
            cell.click(position={"x": 3, "y": 3})
            assert page.url == origin + routes["model"]
            cell.click(position={"x": 3, "y": 3}, modifiers=["Control"])
            assert page.url == origin + routes["model"]
            cell.evaluate("el => { const range = document.createRange(); range.selectNodeContents(el); getSelection().removeAllRanges(); getSelection().addRange(range); }")
            cell.dispatch_event("click", {"button": 0})
            assert page.url == origin + routes["model"]
            page.evaluate("getSelection().removeAllRanges()")
            # A native anchor supports new tabs and keyboard navigation.
            with context.expect_page() as popup:
                row.locator("td.is-lead a").click(modifiers=["Control"])
            popup.value.wait_for_url(origin + routes["record"])
            popup.value.close()
            row.locator("td.is-lead a").focus()
            page.keyboard.press("Enter")
            page.wait_for_url(origin + routes["record"])
            expect(page.locator('input[name="name"]')).to_be_visible()
            assert page.locator('input[name="locked"]').count() == 0
            page.go_back()
            page.go_forward()
            page.wait_for_url(origin + routes["record"])
            # BFCache/pageshow must clear the submission latch and retain dirty warnings.
            field = page.locator('input[name="name"]')
            original = field.input_value()
            field.fill("Unsaved browser draft")
            form = page.locator('form[data-track-dirty="record"]')
            expect(form).to_have_attribute("data-dirty", "1")
            form.dispatch_event("submit")
            expect(form).to_have_attribute("data-submitting", "1")
            page.evaluate("window.dispatchEvent(new PageTransitionEvent('pageshow', {persisted:true}))")
            assert form.get_attribute("data-submitting") is None
            expect(form).to_have_attribute("data-dirty", "1")
            field.fill(original)
            # Row-background clicks also open the same canonical URL.
            page.goto(origin + routes["model"])
            page.locator(f'tr[data-record-href="{routes["record"]}"] td.is-lead').click(position={"x": 3, "y": 3})
            page.wait_for_url(origin + routes["record"])

            for listing, destination in [
                ("/admin/directory/users", routes["user"]),
                ("/admin/directory/roles", routes["role"]),
            ]:
                page.goto(origin + listing)
                page.locator(f'tr[data-record-href="{destination}"] td.is-lead a').focus()
                page.keyboard.press("Enter")
                page.wait_for_url(origin + destination)
            page.goto(origin + "/readonly")
            assert page.locator('input[name="name"]').count() == 0
            assert page.locator('a[href$="/delete"]').count() == 0

            for width, theme in [(1440, "light"), (390, "dark")]:
                page.set_viewport_size({"width": width, "height": 1000})
                for name, path in [("record", routes["record"]), ("user", routes["user"]), ("role", routes["role"])]:
                    page.goto(origin + path)
                    page.locator("html").evaluate("(el, theme) => el.dataset.theme = theme", theme)
                    page.screenshot(path=str(args.artifacts / f"{name}-{width}-{theme}.png"), full_page=True)
                    overflow = page.evaluate("Array.from(document.querySelectorAll('body *')).filter(el => el.getBoundingClientRect().right > document.documentElement.clientWidth + 1).map(el => ({tag:el.tagName,cls:el.className,width:Math.round(el.getBoundingClientRect().width)})).slice(0,12)")
                    assert page.evaluate("(width) => document.documentElement.scrollWidth <= width + 1", width), (name, width, overflow)

            no_js = browser.new_context(java_script_enabled=False)
            no_js_page = no_js.new_page()
            no_js_page.goto(origin + routes["model"])
            no_js_page.locator(f'tr[data-record-href="{routes["record"]}"] td.is-lead a').click()
            no_js_page.wait_for_url(origin + routes["record"])
            expect(no_js_page.locator('input[name="name"]')).to_be_visible()
            no_js.close()
            assert not errors, errors
            browser.close()
        print("Passed: row clicks, native links/new tabs, keyboard, controls, permissions DOM, history and dirty-state regressions.")
        print("Rendered desktop/light and mobile/dark record, user and role pages.")
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


if __name__ == "__main__":
    main()
