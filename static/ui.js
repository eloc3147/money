import {
    el,
    mount,
    unmount
} from "https://redom.js.org/redom.es.min.js";


class MoneyApp {
    constructor() {
        this.home = el("div", "Home");
        this.upload = el("div", el("h1", "Upload"));
        this.current = null;

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
            this.go_to("home");
        };

        this.upload_button.onclick = evt => {
            this.go_to("upload");
        };

        mount(this.main, this.home);
        this.current = this.home;
    }

    go_to(event) {
        if (this.current != null) {
            unmount(this.main, this.current);
        }

        let view;
        switch (event) {
            case "home":
                view = this.home;
                break;
            case "upload":
                view = this.upload;
                break;
        }

        mount(this.main, view);
        this.current = view;
    }
}

mount(document.body, new MoneyApp());