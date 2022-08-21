import {
    el,
    List,
    list,
    RedomComponentClass,
    RedomElement,
    setChildren,
    RedomQueryArgument,
    RedomComponent,
    mount
} from "redom";


export class Page implements RedomComponent {
    el: HTMLElement | SVGElement | RedomComponent;
}


export class Th implements RedomComponent {
    el: HTMLTableCellElement;

    constructor() {
        this.el = el("th");
    }

    update(data: string): void {
        this.el.textContent = data;
    }
}


export class Td implements RedomComponent {
    el: HTMLTableCellElement;

    constructor() {
        this.el = el("td");
    }

    update(data: string): void {
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

    update(data: any[], contents?: any): void {
        this.list.update(data, contents);
    }
}


export interface OptionConfig {
    value: string,
    selected: boolean
}


export class Option implements RedomComponent {
    el: HTMLOptionElement;

    constructor() {
        this.el = el("option");
    }

    update(config: OptionConfig, index: number, _data: any, _context?: any): void {
        this.el.value = index.toString();
        this.el.textContent = config.value;
        this.el.selected = config.selected;
    }
}


export class TdDropdown implements RedomComponent {
    el: HTMLTableCellElement;
    column_index: number | null;
    select: List;
    callback: ((column_index: number, input_text: string) => void) | null;

    constructor() {
        this.column_index = null;
        this.callback = null;

        this.select = list("select", Option);
        this.select.el.onchange = (_evt) => {
            this.push_selection();
        };

        this.el = el("td", el("div.select", this.select));
    }

    push_selection(): void {
        let index = (this.select.el as HTMLSelectElement).selectedIndex;
        let input_text = this.select.el.children[index].innerHTML.trim();

        if (this.callback != null && this.column_index != null) {
            this.callback(this.column_index, input_text);
        }
    }

    update(item: OptionConfig[], index: number, _data: any, context?: any): void {
        this.column_index = index;
        this.callback = context.callback;
        this.select.update(item);
    }
}


export class Table implements RedomComponent {
    el: HTMLTableElement;
    header_row: Tr | null;

    constructor(headers: string[] | null) {
        let rows: HTMLTableSectionElement[] = [];

        if (headers != null) {
            this.header_row = new Tr(Th);
            this.header_row.update(headers);
            rows.push(el("thead", this.header_row));
        } else {
            this.header_row = null;
        }

        this.el = el("table", rows, { class: "table" });
    }

    clear_rows(): void {
        if (this.header_row != null) {
            setChildren(this.el, [this.header_row]);
        } else {
            setChildren(this.el, []);
        }
    }

    add_row(row: Tr): void {
        mount(this.el, row);
    }

    add_rows(rows: Tr[]): void {
        for (let row in rows) {
            this.add_row(rows[row]);
        }
    }

    add_plain_rows(rows: string[][] | number[][]): void {
        for (let row in rows) {
            let row_el = new Tr(Td);
            row_el.update(rows[row]);
            this.add_row(row_el);
        }
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

    set_column_args(args: string): void {
        this.column.className = "column " + args;
    }

    set_contents(contents: HTMLElement[] | RedomElement[]): void {
        setChildren(this.column, contents);
    }
}
