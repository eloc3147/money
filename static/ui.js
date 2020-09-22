/*jshint esversion: 6 */

import {
    el,
    list,
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
    constructor() {
        this.el = el("div.content");
    }
}

class UploadPage {
    constructor(client) {
        this.client = client;
        this.session = null;

        this.el = el("div.content", [
            (this.header = el("h1", "Upload")),
            el("form.pure-form", [
                this.file_field = el("input", {
                    type: "file"
                }),
                this.submit_button = el("button.pure-button pure-button-primary", "Upload", {
                    type: "submit"
                })
            ])
        ]);

        this.submit_button.onclick = evt => {
            evt.preventDefault();

            var reader = new FileReader();
            reader.onloadend = e => {
                this.session = this.client.load_file(reader);
                this.draw_preview();
            };

            reader.readAsText(this.file_field.files[0]);
        };
    }

    draw_preview() {
        this.preview = new Table();
        let row_count = Math.min(10, this.session.get_row_count());
        this.preview.set_contents(
            this.session.get_headers().map(h => {
                return '"' + h + '"';
            }),
            this.session.get_row_slice(0, row_count)
        );

        setChildren(this.el, [this.header, this.preview]);
    }
}


class MoneyApp {
    constructor(client) {
        this.current = null;
        this.client = client;

        this.el = el("div#layout", [
            el("a.menu-link#menu_link", el("span")),
            el("div#menu", el("div.pure-menu", [
                el("a.pure-menu-heading", "Money View"),
                el("ul.pure-menu-list", [
                    el("li.pure-menu-item", (this.home_button = el("a.pure-menu-link", "Money"))),
                    el("li.pure-menu-item", (this.upload_button = el("a.pure-menu-link", "Upload")))
                ])
            ])),
            (this.main = el("div#main"))
        ]);

        this.home_button.onclick = evt => {
            evt.preventDefault();
            this.go_to("home");
        };

        this.upload_button.onclick = evt => {
            evt.preventDefault();
            this.go_to("upload");
        };

        this.go_to("home");
    }

    go_to(event) {
        if (this.current != null) {
            unmount(this.main, this.current);
        }

        let view;
        switch (event) {
            case "home":
                view = new HomePage();
                this.home_button.className = "pure-menu-link pure-menu-selected";
                this.upload_button.className = "pure-menu-link";
                break;
            case "upload":
                view = new UploadPage(this.client);
                this.home_button.className = "pure-menu-link";
                this.upload_button.className = "pure-menu-link pure-menu-selected";
                break;
        }

        mount(this.main, view);
        this.current = view;
    }
}


async function main() {
    await init();
    let client = new Money();
    mount(document.body, new MoneyApp(client));
}


main();