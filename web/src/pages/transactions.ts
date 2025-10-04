import { el, list, List, RedomComponent } from "redom";
import { loadTransactions, TransactionsResponse } from "../api";

const DISPLAY_LIMIT = 250;
const AMOUNT_COL = 7;

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

    constructor(onChange: (selected: Set<string>) => void) {
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
    }

    update(item: Set<string>): void {
        this.select.update(Array.from(item).toSorted());
    }
}

type FilterCallback = (
    accounts: Set<string>,
    base_categories: Set<string>,
    categories: Set<string>,
    source_categories: Set<string>
) => void;

class Table implements RedomComponent {
    account_select: Select;
    base_category_select: Select;
    category_select: Select;
    source_category_select: Select;

    selected_accounts: Set<string>;
    selected_base_categories: Set<string>;
    selected_categories: Set<string>;
    selected_source_categories: Set<string>;
    select_callback: FilterCallback;

    body: List;
    el: HTMLElement;

    constructor(select_callback: FilterCallback) {
        this.account_select = new Select(this.onAccountsUpdate.bind(this));
        this.base_category_select = new Select(this.onBaseCategoryUpdate.bind(this));
        this.category_select = new Select(this.onCategoryUpdate.bind(this));
        this.source_category_select = new Select(this.onSourceCategoryUpdate.bind(this));

        this.selected_accounts = new Set();
        this.selected_base_categories = new Set();
        this.selected_categories = new Set();
        this.selected_source_categories = new Set();
        this.select_callback = select_callback;

        this.body = list("tbody", DataRow);
        this.el = el("table.table.is-bordered.is-striped.is-hoverable.is-fullwidth.sticky-table", [
            el("thead.stick-thead", el("tr", [
                el("th", "Account"),
                el("th", el("abbr", "B. Category", { title: "Base Category" })),
                el("th", "Category"),
                el("th", el("abbr", "S. Category", { title: "Source Category" })),
                el("th", "Income"),
                el("th", "Type"),
                el("th", "Date"),
                el("th", "Amount"),
                el("th", "ID"),
                el("th", "Name"),
                el("th", "Memo"),
            ])),
            el("thead", el("tr", [
                el("th", el("div.field", el("div.control", this.account_select))),
                el("th", el("div.field", el("div.control", this.base_category_select))),
                el("th", el("div.field", el("div.control", this.category_select))),
                el("th", el("div.field", el("div.control", this.source_category_select))),
                el("th", ""),
                el("th", ""),
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

    onAccountsUpdate(selected: Set<string>): void {
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

    pushSelections(): void {
        this.select_callback(
            this.selected_accounts,
            this.selected_base_categories,
            this.selected_categories,
            this.selected_source_categories
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
        for (const transaction of this.transactions) {
            accounts.add(transaction[0]);
            base_categories.add(transaction[1]);
            categories.add(transaction[2]);
            source_categories.add(transaction[3] || " - ");
        }

        this.table.setAccounts(accounts);
        this.table.setBaseCategories(base_categories);
        this.table.setCategories(categories);
        this.table.setSourceCategories(source_categories);
    }

    updateTable(
        accounts: Set<string>,
        base_categories: Set<string>,
        categories: Set<string>,
        source_categories: Set<string>
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
                    && source_categories.has(row[3] || " - ");
            })
            .toArray();

        // TODO: Sort

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
