/*jshint esversion: 6 */

import { el, mount, unmount } from "https://redom.js.org/redom.es.min.js";
import init, { Money } from "/money_web.js";
import { UploadPage } from "/upload_page.js";
import { HomePage } from "/home_page.js";


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
            this.content = el("div.container")
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