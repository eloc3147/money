/*jshint esversion: 6 */

import { el, list, setChildren } from "https://redom.js.org/redom.es.min.js";

export class Th {
    constructor() {
        this.el = el("th");
    }
    update(data) {
        this.el.textContent = data;
    }
}

export class Td {
    constructor() {
        this.el = el("td");
    }
    update(data) {
        this.el.textContent = data;
    }
}

export class Tr {
    constructor(type) {
        this.el = el("tr");
        this.list = list(this.el, type);
    }
    update(data, index, items, context) {
        this.list.update(data, index, items, context);
    }
}

export class Option {
    constructor(type) {
        this.el = el("option");
    }
    update(data, index) {
        this.el.value = index;
        this.el.textContent = data[0];
        if (data[1] === true) {
            this.el.selected = "true";
        }
    }
}

export class TdDropdown {
    constructor() {
        this.column_index = null;
        this.callback = null;

        this.select = list("select", Option);
        this.select.el.onchange = (evt) => {
            this.push_selection();
        };

        this.el = el("td", el("div.select", this.select));
    }

    push_selection() {
        var index = this.select.el.selectedIndex;
        var input_text = this.select.el.children[index].innerHTML.trim();

        if (this.callback != null) {
            this.callback(this.column_index, input_text);
        }
    }

    update(data, index, _, context) {
        this.column_index = index;
        this.callback = context.callback;
        this.select.update(data);
    }
}

export class Table {
    constructor() {
        this.header_row = new Tr(Th);
        this.dropdown_element = new Tr(TdDropdown);

        this.rows = [
            el("thead", this.header_row),
            this.dropdown_element,
        ];

        this.el = el("table.table", this.rows);
    }

    set_headers(headers) {
        this.header_row.update(headers);
    }

    set_suggestions(suggestions, column_callback) {
        this.dropdown_element.update(suggestions, { callback: column_callback });
    }

    add_rows(rows) {
        for (let row in rows) {
            let el = new Tr(Td);
            el.update(rows[row]);
            this.rows.push(el);
        }
        setChildren(this.el, this.rows);
    }
}

export class ColumnView {
    constructor(column_args, contents) {
        this.el = el("div", { class: "columns is-mobile is-centered" },
            this.column = el("div", { class: "column " + column_args }, contents)
        );
    }

    set_column_args(args) {
        this.column.className = "column " + args;
    }

    set_contents(contents) {
        setChildren(this.column, contents);
    }
}
