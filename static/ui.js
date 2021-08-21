/*jshint esversion: 6 */

import {
    el,
    mount,
    unmount,
    setChildren
} from "https://redom.js.org/redom.es.min.js";

import init, {
    Money
} from "/money_web.js";

import {
    Table
} from "/components.js";

class HomePage {
    constructor(args) {
        this.el = el("div.container", el("h1.title", "Home Page (TODO)"));
    }
}

class UploadPage {
    constructor(args) {
        this.client = args.client;
        this.session = null;
        this.preview = null;
        this.submit_button = null;
        this.error_label = null;
        this.submit_wrapper = null;

        this.el = el("div.container",
            this.contents = el("div", { class: "columns is-mobile is-centered" },
                el("div", { class: "column is-half" }, [
                    this.title = el("p", { class: "title is-1" }, "Add Transactions"),
                    this.subtitle = el("p", { class: "subtitle is-3" }, "Select a file"),
                    el("div.field", [
                        el("label.label", "File upload"),
                        el("div.control", this.file_field = el("input.input", { type: "file" }))
                    ]),
                    el("div.field",
                        el("div.control", this.load_button = el("button", { class: "button is-link" }, "Load file"))
                    )
                ])
            )
        );

        this.load_button.onclick = evt => {
            evt.preventDefault();

            var reader = new FileReader();
            reader.onloadend = e => {
                if (reader == null) {
                    console.log("Error: reader is null.");
                    return;
                }
                this.session = this.client.load_file(reader);
                this.draw_preview();

            };

            reader.readAsText(this.file_field.files[0]);
        };
    }

    draw_preview() {
        let row_count = Math.min(10, this.session.get_row_count());

        this.preview = new Table();
        this.preview.set_contents(
            this.session.get_headers().map(h => {
                return '"' + h + '"';
            }),
            this.session.get_header_suggestions(),
            this.session.get_row_slice(0, row_count),
            (column_index, selection) => {
                this.process_update(column_index, selection);
            }
        );

        this.subtitle.innerText = "Select the types of each column";

        setChildren(this.contents, el("div", { class: "column is-full" }, [
            this.title,
            this.subtitle,
            this.preview,
            el("div", { className: "field is-grouped" }, [
                this.submit_wrapper = el("fieldset",
                    el("div.control",
                        this.submit_button = el("button", { class: "button is-link" }, "Load file")
                    )
                ),
                this.error_label = el("div", { className: "notification is-danger is-hidden" }, "")
            ]),

        ]));

        this.submit_button.onclick = evt => {
            evt.preventDefault();

        };

        this.file_field = null;
        this.load_button = null;
    }

    process_update(column_index, selection) {
        this.session.update_header_selection(column_index, selection);

        let selection_error = this.session.get_selection_error();
        if (selection_error !== undefined) {
            this.error_label.textContent = selection_error;
            this.error_label.className = "notification is-danger";
            this.submit_wrapper.setAttribute("disabled", true);
        } else {
            this.error_label.textContent = "";
            this.error_label.className = "notification is-danger is-hidden";
            this.submit_wrapper.removeAttribute("disabled");
        }
    }
}


class NavbarItem {
    constructor(app, name) {
        this.app = app;
        this.name = name;
        this.el = el("a.navbar-item", this.name);

        this.el.onclick = evt => {
            evt.preventDefault();
            this.app.select(this.name);
        };
    }
}


class MoneyApp {
    constructor(client) {
        this.client = client;

        // Nav
        this.navbar_items = [];
        this.page_map = {
            "Home": [HomePage, {}],
            "Upload": [UploadPage, { client: this.client }]
        };
        this.pages = [];
        this.current = null;

        for (const page_name in this.page_map) {
            this.pages.push(new NavbarItem(this, page_name));
        }

        this.el = el("div", [
            el("nav.navbar", { role: "navigation", "aria-label": "main navigation", "is-primary": "" }, [
                el("div.navbar-brand", el("span.navbar-item", "Money")),
                el("div.navbar-menu", el("div.navbar-start", this.pages))
            ]),
            this.content = el("section.section")
        ]);

        this.select(this.pages[0].name);
    }

    select(page) {
        if (this.current != null) {
            unmount(this.content, this.current);
        }

        let page_args = this.page_map[page];
        let page_class = page_args[0];
        let view = new page_class(page_args[1]);

        mount(this.content, view);
        this.current = view;
    }
}


async function main() {
    await init();
    let client = new Money();
    mount(document.body, new MoneyApp(client));
}


main();