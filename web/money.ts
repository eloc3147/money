import { el, mount, unmount, RedomElement } from "redom";

import init, { Money } from "../money-web/pkg/money_web";
import { UploadPage } from "./upload_page";
import { HomePage } from "./home_page";
import { Page } from "./page";


class NavbarItem {
    app: MoneyApp;
    name: string;
    el: HTMLAnchorElement;

    constructor(app: MoneyApp, name: string) {
        this.app = app;
        this.name = name;
        this.el = el("a", this.name, { class: "navbar-item" });

        this.el.onclick = evt => {
            evt.preventDefault();
            this.app.select(this.name);
        };
    }
}


class MoneyApp {
    client: Money;
    page_map: { [title: string]: Page };
    navbar_items: NavbarItem[];
    current: Page;
    el: HTMLDivElement;
    content: HTMLDivElement;

    constructor(client: Money) {
        this.client = client;

        // Nav
        this.page_map = {
            "Home": new HomePage(),
            "Upload": new UploadPage(this.client)
        };
        this.navbar_items = [];
        this.current = null;

        for (const page_name in this.page_map) {
            this.navbar_items.push(new NavbarItem(this, page_name));
        }

        this.el = el("div", [
            el("nav.navbar", { role: "navigation", "aria-label": "main navigation", "is-primary": "" }, [
                el("div.navbar-brand", el("span.navbar-item", "Money")),
                el("div.navbar-menu", el("div.navbar-start", this.navbar_items))
            ]),
            this.content = el("div", { class: "container" })
        ]);

        this.select(this.navbar_items[0].name);
    }

    select(page_name: string) {
        if (this.current != null) {
            unmount(this.content, this.current);
        }

        let view = this.page_map[page_name];

        mount(this.content, view);
        this.current = view;
    }
}


async function main() {
    await init(new URL("/money_web_bg.wasm", window.location.origin));
    let client = new Money();
    mount(document.body, new MoneyApp(client));
}


main();
