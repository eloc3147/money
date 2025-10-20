import { el, list, List, RedomComponent } from "redom";
import { loadTransactions, TransactionsResponse } from "../api";

const DISPLAY_LIMIT = 250;
const AMOUNT_COL = 7;

function sortNormal<T>(a: T, b: T): number {
    if (a < b) {
        return -1;
    } else if (a > b) {
        return 1;
    } else {
        return 0;
    }
}

function sortStringOrNull(a: string | null, b: string | null): number {
    const a_str = a || " - ";
    const b_str = b || " - ";
    if (a_str < b_str) {
        return -1;
    } else if (a_str > b_str) {
        return 1;
    } else {
        return 0;
    }
}


class DataCell implements RedomComponent {
    el: HTMLTableCellElement;

    constructor() {
        this.el = el("td");
    }

    update(data: string, index: number): void {
        this.el.textContent = data;
        if (index == AMOUNT_COL) {
            this.el.className = "amount-cell";
        }
    }
}

class DataRow implements RedomComponent {
    el: List;

    constructor() {
        this.el = list("tr", DataCell);
    }

    update(data: string[]): void {
        this.el.update(data);
    }
}

class SortHeader implements RedomComponent {
    ascending: boolean | null;

    ascending_arrow: HTMLElement;
    descending_arrow: HTMLElement;
    el: HTMLElement;

    constructor(onClick: () => void, name: string, long_name: string | null, ascending: boolean | null = null) {
        this.ascending = null;

        this.ascending_arrow = el("div.sort-arrow", "\u{25b2}");
        this.descending_arrow = el("div.sort-arrow", "\u{25bc}");
        this.el = el("div.sort-header", [
            long_name !== null ? el("abbr", name, { title: long_name }) : name,
            el("div.column-sorter", [
                this.ascending_arrow,
                this.descending_arrow,
            ])
        ]);

        this.el.onclick = (_ev: PointerEvent) => onClick();

        if (ascending !== null) {
            this.select(ascending);
        }
    }

    select(ascending: boolean | null): void {
        if (ascending === this.ascending) {
            return;
        }

        if (ascending === true) {
            this.ascending_arrow.classList.add("selected-arrow");
            this.descending_arrow.classList.remove("selected-arrow");
        } else if (ascending === false) {
            this.ascending_arrow.classList.remove("selected-arrow");
            this.descending_arrow.classList.add("selected-arrow");
        } else {
            this.ascending_arrow.classList.remove("selected-arrow");
            this.descending_arrow.classList.remove("selected-arrow");
        }

        this.ascending = ascending;
    }
}

class SelectOption implements RedomComponent {
    el: HTMLElement;

    constructor() {
        this.el = el("option", { selected: true });
    }

    update(item: string): void {
        this.el.textContent = item;
    }
}

class Select implements RedomComponent {
    select_el: HTMLSelectElement;
    select: List;
    el: HTMLElement;

    constructor(onChange: (selected: Set<string>) => void, options: Set<string> | null = null) {
        this.select_el = el("select.filter-select", { multiple: true }) as HTMLSelectElement;
        this.select = list(this.select_el, SelectOption);
        this.select.el.onchange = (_e: Event) => {
            // Get selected option labels
            let selected: Set<string> = new Set();
            const selectedOptions = this.select_el.selectedOptions;
            for (let i = 0; i < selectedOptions.length; i++) {
                selected.add((selectedOptions[i] as HTMLOptionElement).label);
            }

            onChange(selected);
        };
        this.el = el("div.select.is-multiple.is-info.is-small.filter-container", this.select);

        if (options !== null) {
            this.update(options);
        }
    }

    update(item: Set<string>): void {
        this.select.update(Array.from(item).toSorted());
    }
}

type FilterCallback = (
    accounts: Set<string>,
    base_categories: Set<string>,
    categories: Set<string>,
    source_categories: Set<string>,
    incomes: Set<boolean>,
    types: Set<string>,
    sort_column: number,
    sort_ascending: boolean
) => void;

class Table implements RedomComponent {
    account_select: Select;
    base_category_select: Select;
    category_select: Select;
    source_category_select: Select;
    income_select: Select;
    type_select: Select;

    selected_accounts: Set<string>;
    selected_base_categories: Set<string>;
    selected_categories: Set<string>;
    selected_source_categories: Set<string>;
    selected_incomes: Set<boolean>;
    selected_types: Set<string>;
    select_callback: FilterCallback;

    sort_headers: SortHeader[];

    sort_idx: number;
    sort_ascending: boolean;

    body: List;
    el: HTMLElement;

    constructor(select_callback: FilterCallback) {
        this.account_select = new Select(this.onAccountUpdate.bind(this));
        this.base_category_select = new Select(this.onBaseCategoryUpdate.bind(this));
        this.category_select = new Select(this.onCategoryUpdate.bind(this));
        this.source_category_select = new Select(this.onSourceCategoryUpdate.bind(this));
        this.income_select = new Select(this.onIncomeUpdate.bind(this), new Set(["Yes", "No"]));
        this.type_select = new Select(this.onTypeUpdate.bind(this));

        this.selected_accounts = new Set();
        this.selected_base_categories = new Set();
        this.selected_categories = new Set();
        this.selected_source_categories = new Set();
        this.selected_incomes = new Set([true, false]);
        this.selected_types = new Set();
        this.select_callback = select_callback;

        this.sort_headers = [
            new SortHeader((() => this.onColumnSort(0)).bind(this), "Account", null),
            new SortHeader((() => this.onColumnSort(1)).bind(this), "B. Category", "Base Category"),
            new SortHeader((() => this.onColumnSort(2)).bind(this), "Category", null),
            new SortHeader((() => this.onColumnSort(3)).bind(this), "S. Category", "Source Category"),
            new SortHeader((() => this.onColumnSort(4)).bind(this), "Income", null),
            new SortHeader((() => this.onColumnSort(5)).bind(this), "Type", null),
            new SortHeader((() => this.onColumnSort(6)).bind(this), "Date", null, false),
            new SortHeader((() => this.onColumnSort(7)).bind(this), "Amount", null),
            new SortHeader((() => this.onColumnSort(8)).bind(this), "ID", null),
            new SortHeader((() => this.onColumnSort(9)).bind(this), "Name", null),
            new SortHeader((() => this.onColumnSort(10)).bind(this), "Memo", null)
        ];

        this.sort_idx = 6;
        this.sort_ascending = false;

        this.body = list("tbody", DataRow);
        this.el = el("table.table.is-bordered.is-striped.is-hoverable.is-fullwidth.sticky-table", [
            el("thead.stick-thead", el("tr", [
                el("th", this.sort_headers[0] as SortHeader),
                el("th", this.sort_headers[1] as SortHeader),
                el("th", this.sort_headers[2] as SortHeader),
                el("th", this.sort_headers[3] as SortHeader),
                el("th", this.sort_headers[4] as SortHeader),
                el("th", this.sort_headers[5] as SortHeader),
                el("th", this.sort_headers[6] as SortHeader),
                el("th", this.sort_headers[7] as SortHeader),
                el("th", this.sort_headers[8] as SortHeader),
                el("th", this.sort_headers[9] as SortHeader),
                el("th", this.sort_headers[10] as SortHeader),
            ])),
            el("thead", el("tr", [
                el("th", el("div.field", el("div.control", this.account_select))),
                el("th", el("div.field", el("div.control", this.base_category_select))),
                el("th", el("div.field", el("div.control", this.category_select))),
                el("th", el("div.field", el("div.control", this.source_category_select))),
                el("th", el("div.field", el("div.control", this.income_select))),
                el("th", el("div.field", el("div.control", this.type_select))),
                el("th", ""),
                el("th", ""),
                el("th", ""),
                el("th", ""),
                el("th", ""),
            ])),
            this.body
        ]);
    }

    setTransactions(transactions: string[][]): void {
        this.body.update(transactions);
    }


    setAccounts(accounts: Set<string>): void {
        this.account_select.update(accounts);
        this.selected_accounts = accounts;
    }

    setBaseCategories(base_categories: Set<string>): void {
        this.base_category_select.update(base_categories);
        this.selected_base_categories = base_categories;
    }

    setCategories(categories: Set<string>): void {
        this.category_select.update(categories);
        this.selected_categories = categories;
    }

    setSourceCategories(source_categories: Set<string>): void {
        this.source_category_select.update(source_categories);
        this.selected_source_categories = source_categories;
    }

    setTypes(types: Set<string>): void {
        this.type_select.update(types);
        this.selected_types = types;
    }

    onAccountUpdate(selected: Set<string>): void {
        this.selected_accounts = selected;
        this.pushSelections();
    }

    onBaseCategoryUpdate(selected: Set<string>): void {
        this.selected_base_categories = selected;
        this.pushSelections();
    }

    onCategoryUpdate(selected: Set<string>): void {
        this.selected_categories = selected;
        this.pushSelections();
    }

    onSourceCategoryUpdate(selected: Set<string>): void {
        this.selected_source_categories = selected;
        this.pushSelections();
    }

    onIncomeUpdate(selected: Set<string>): void {
        this.selected_incomes = new Set(selected.values().map((v) => v == "Yes").toArray());
        this.pushSelections();
    }

    onTypeUpdate(selected: Set<string>): void {
        this.selected_types = selected;
        this.pushSelections();
    }

    onColumnSort(column: number): void {
        if (column == this.sort_idx) {
            this.sort_ascending = !this.sort_ascending;
        } else {
            this.sort_headers[this.sort_idx]?.select(null);
            this.sort_ascending = false;
        }

        this.sort_idx = column;
        this.sort_headers[column]?.select(this.sort_ascending);
        this.pushSelections();
    }

    pushSelections(): void {
        this.select_callback(
            this.selected_accounts,
            this.selected_base_categories,
            this.selected_categories,
            this.selected_source_categories,
            this.selected_incomes,
            this.selected_types,
            this.sort_idx,
            this.sort_ascending,
        );
    }
}

export class TransactionsPage implements RedomComponent {
    transactions: TransactionsResponse | null;
    table: Table;
    truncated: HTMLElement;
    el: HTMLElement;

    constructor() {
        this.transactions = null;

        this.table = new Table(this.updateTable.bind(this));
        this.truncated = el(
            "article.message.is-warning.is-hidden",
            el("div.message-body", `Truncated to ${DISPLAY_LIMIT} rows`)
        );
        this.el = el("div.container.is-fluid", [
            this.table,
            this.truncated
        ]);
    }

    async onmount(): Promise<void> {
        if (this.transactions != null) {
            return;
        }

        await this.loadTransactions();
        this.table.pushSelections();
    }

    async loadTransactions(): Promise<void> {
        this.transactions = await loadTransactions();

        let accounts: Set<string> = new Set();
        let base_categories: Set<string> = new Set();
        let categories: Set<string> = new Set();
        let source_categories: Set<string> = new Set();
        let types: Set<string> = new Set();
        for (const transaction of this.transactions) {
            accounts.add(transaction[0]);
            base_categories.add(transaction[1]);
            categories.add(transaction[2]);
            source_categories.add(transaction[3] || " - ");
            types.add(transaction[5]);
        }

        this.table.setAccounts(accounts);
        this.table.setBaseCategories(base_categories);
        this.table.setCategories(categories);
        this.table.setSourceCategories(source_categories);
        this.table.setTypes(types);
    }

    updateTable(
        accounts: Set<string>,
        base_categories: Set<string>,
        categories: Set<string>,
        source_categories: Set<string>,
        incomes: Set<boolean>,
        types: Set<string>,
        sort_column: number,
        sort_ascending: boolean
    ): void {
        if (this.transactions === null) {
            return;
        }

        // TODO: When fully implemented, if this takes way longer than 39ms, compare performance against sqlite again
        let rows = this.transactions
            .values()
            .filter((row, _idx) => {
                return accounts.has(row[0])
                    && base_categories.has(row[1])
                    && categories.has(row[2])
                    && source_categories.has(row[3] || " - ")
                    && incomes.has(row[4])
                    && types.has(row[5]);
            })
            .toArray();


        let compareFn: (a: any, b: any) => number;
        switch (sort_column) {
            case 3:
            case 8:
            case 10:
                compareFn = sortStringOrNull;
                break;
            default:
                compareFn = sortNormal;
                break;
        }

        const reverse = sort_ascending ? 1 : -1;
        rows.sort((a, b) => compareFn(a[sort_column], b[sort_column]) * reverse);

        // Truncate
        let truncated = false;
        if (rows.length > DISPLAY_LIMIT) {
            rows = rows.slice(0, DISPLAY_LIMIT);
            truncated = true;
        }

        // Format
        let mapped_rows = rows.map(([
            account,
            base_category,
            category,
            source_category,
            income,
            transaction_type,
            date_str,
            amount,
            transaction_id,
            name,
            memo
        ]) => [
                account,
                base_category,
                category,
                source_category == null ? "" : source_category,
                income ? "Yes" : "No",
                transaction_type,
                date_str,
                amount.toFixed(2),
                transaction_id == null ? "" : transaction_id,
                name,
                memo == null ? "" : memo
            ]);

        // Apply rows
        this.table.setTransactions(mapped_rows);

        if (truncated) {
            this.truncated.classList.remove("is-hidden");
        } else {
            this.truncated.classList.add("is-hidden");
        }
    }
}
