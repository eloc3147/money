import {
    el,
    List,
    list,
    RedomElement,
    setChildren,
    RedomQueryArgument,
    RedomComponent,
    mount
} from "redom";


export interface Page extends RedomComponent {
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


interface ReadableList<T> extends List {
    // Undocumented REDOM parameter
    views: T[];
}


export class Tr<T extends RedomComponent> implements RedomComponent {
    el: HTMLTableRowElement;
    list: ReadableList<T>;
    cells: T[];

    constructor(type: { new(): T }) {
        this.el = el("tr");
        this.list = list(this.el, type) as ReadableList<T>;
        this.cells = [];
    }

    update(data: any[], contents?: any): void {
        this.cells = data;
        this.list.update(this.cells, contents);
    }

    get_cell(index: number): T {
        return this.list.views[index];
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
        const index = (this.select.el as HTMLSelectElement).selectedIndex;
        const input_text = this.select.el.children[index].innerHTML.trim();

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

type TableCellOptions = Td | TdDropdown;


export class Table implements RedomComponent {
    el: HTMLTableElement;
    body: HTMLTableSectionElement;
    rows: Tr<Th | TableCellOptions>[];

    constructor(headers: string[] | null) {
        this.rows = [];

        const sections: HTMLTableSectionElement[] = [];

        if (headers != null) {
            const header_row = new Tr(Th);
            header_row.update(headers);

            sections.push(el("thead", header_row));
        }

        this.body = el("tbody");
        sections.push(this.body);

        this.el = el("table", sections, { class: "table" });
    }

    clear_rows(): void {
        this.rows = [];
        setChildren(this.body, this.rows);
    }

    add_row(row: Tr<TableCellOptions>): void {
        this.rows.push(row);
        mount(this.body, row);
    }

    add_rows(rows: Tr<TableCellOptions>[]): void {
        for (const row in rows) {
            this.add_row(rows[row]);
        }
    }

    add_plain_rows(rows: string[][] | number[][]): void {
        for (const row in rows) {
            const row_el = new Tr(Td);
            row_el.update(rows[row]);
            this.add_row(row_el);
        }
    }

    get_row(index: number): Tr<TableCellOptions> {
        return this.rows[index];
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
