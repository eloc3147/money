import { el, List, list, RedomComponentClass, RedomElement, setChildren, RedomQueryArgument, RedomComponent } from "redom";

export class Th implements RedomComponent {
    el: HTMLTableCellElement;

    constructor() {
        this.el = el("th");
    }
    update(data: string) {
        this.el.textContent = data;
    }
}

export class Td implements RedomComponent {
    el: HTMLTableCellElement;

    constructor() {
        this.el = el("td");
    }
    update(data: string) {
        this.el.textContent = data;
    }
}

export class Tr implements RedomComponent {
    el: HTMLTableRowElement;
    list: List;

    constructor(type: RedomComponentClass) {
        this.el = el("tr");
        this.list = list(this.el, type);
    }
    update(data: any[], contents?: any) {
        this.list.update(data, contents);
    }
}

interface OptionConfig {
    cell_value: string;
    selected: boolean;
}

export class Option implements RedomComponent {
    el: HTMLOptionElement;

    constructor() {
        this.el = el("option");
    }
    update(item: OptionConfig, index: number, _data: any, _context?: any) {
        this.el.value = index.toString();
        this.el.textContent = item.cell_value;
        this.el.selected = item.selected;
    }
}

export class TdDropdown implements RedomComponent {
    el: HTMLTableCellElement;
    column_index: number;
    select: List;
    callback: (column_index: number, input_text: string) => void;

    constructor() {
        this.column_index = null;
        this.callback = null;

        this.select = list("select", Option);
        this.select.el.onchange = (_evt) => {
            this.push_selection();
        };

        this.el = el("td", el("div.select", this.select));
    }

    push_selection() {
        var index = (this.select.el as HTMLSelectElement).selectedIndex;
        var input_text = this.select.el.children[index].innerHTML.trim();

        if (this.callback != null) {
            this.callback(this.column_index, input_text);
        }
    }

    update(item: any, index: number, _data: any, context?: any) {
        this.column_index = index;
        this.callback = context.callback;
        this.select.update(item);
    }
}

export class Table implements RedomComponent {
    el: HTMLTableElement;
    header_row: Tr;
    dropdown_element: Tr;
    rows: RedomElement[];

    constructor() {
        this.header_row = new Tr(Th);
        this.dropdown_element = new Tr(TdDropdown);

        this.rows = [
            el("thead", this.header_row),
            this.dropdown_element,
        ];

        this.el = el("table", this.rows, { class: "table" });
    }

    set_headers(headers: string[]) {
        this.header_row.update(headers);
    }

    set_suggestions(suggestions: string[], column_callback: (column_index: number, input_text: string) => void) {
        this.dropdown_element.update(suggestions, { callback: column_callback });
    }

    add_rows(rows: any[][]) {
        for (let row in rows) {
            let el = new Tr(Td);
            el.update(rows[row]);
            this.rows.push(el);
        }
        setChildren(this.el, this.rows);
    }
}

export class ColumnView implements RedomComponent {
    el: HTMLDivElement;
    column: HTMLDivElement;

    constructor(column_class: string, ...contents: RedomQueryArgument[]) {
        this.el = el("div", { class: "columns is-mobile is-centered" },
            this.column = el("div", { class: "column " + column_class }, contents)
        );
    }

    set_column_args(args: string) {
        this.column.className = "column " + args;
    }

    set_contents(contents: HTMLElement[] | RedomElement[]) {
        setChildren(this.column, contents);
    }
}
