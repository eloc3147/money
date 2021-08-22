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
        this.el = el("table.table");
    }

    set_contents(headers, suggestions, rows, column_callback) {
        let row_elements = rows.map(r => {
            let el = new Tr(Td);
            el.update(r);
            return el;
        });

        let dropdown_element = new Tr(TdDropdown);
        dropdown_element.update(suggestions, {
            callback: column_callback
        });
        row_elements.unshift(dropdown_element);

        let header_row = new Tr(Th);
        header_row.update(headers);

        let header_element = el("thead", header_row);
        row_elements.unshift(header_element);

        setChildren(this.el, row_elements);
    }

}