/*jshint esversion: 6 */

import {
    el,
    list,
    setChildren
} from "https://redom.js.org/redom.es.min.js";

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
    update(data) {
        console.log(data);
        this.list.update(data);
    }
}

export class Option {
    constructor(type) {
        this.el = el("option");
    }
    update(data) {
        this.el.value = data[0];
        this.el.textContent = data[1];
        if (data[2] === true) {
            this.el.selected = "true";
        }
    }
}

export class TdDropdown {
    constructor() {
        this.select = list("select", Option);
        this.el = el("td", this.select);
    }
    update(data) {
        this.select.update(data);
    }
}

export class Table {
    constructor() {
        this.el = el("table.pure-table-bordered");
    }

    set_contents(headers, suggestions, rows) {
        let row_elements = rows.map(r => {
            let el = new Tr(Td);
            el.update(r);
            return el;
        });

        let dropdown_element = new Tr(TdDropdown);
        console.log(suggestions);
        dropdown_element.update(suggestions);
        row_elements.unshift(dropdown_element);

        let header_element = new Tr(Th);
        header_element.update(headers);
        row_elements.unshift(header_element);

        setChildren(this.el, row_elements);
    }

}