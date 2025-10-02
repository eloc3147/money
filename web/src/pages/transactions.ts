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

class Table implements RedomComponent {
    body: List;
    el: HTMLElement;

    constructor() {
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
            this.body
        ]);
    }

    setTransactions(transactions: string[][]): void {
        this.body.update(transactions);
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
        this.select_el = el("select", { multiple: true });
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
        this.el = el("div.select.is-multiple", this.select);
    }

    update(item: Set<string>): void {
        this.select.update(Array.from(item).toSorted());
    }
}

type FilterCallback = (accounts: Set<string>) => void;

class TransactionFilters implements RedomComponent {
    account_select: Select;

    selected_accounts: Set<string>;
    select_callback: FilterCallback;

    el: HTMLElement;

    constructor(select_callback: FilterCallback) {
        this.account_select = new Select(this.onAccountsUpdate.bind(this));

        this.selected_accounts = new Set();
        this.select_callback = select_callback;

        this.el = el("div.field", [
            el("label.label", "Account"),
            this.account_select
        ]);
    }

    set_accounts(accounts: Set<string>): void {
        this.account_select.update(accounts);
        this.selected_accounts = accounts;
    }

    onAccountsUpdate(selected: Set<string>): void {
        this.selected_accounts = selected;
        this.pushSelections();
    }

    pushSelections(): void {
        this.select_callback(this.selected_accounts);
    }
}

export class TransactionsPage implements RedomComponent {
    transactions: TransactionsResponse | null;
    filters: TransactionFilters;
    table: Table;
    truncated: HTMLElement;
    el: HTMLElement;

    constructor() {
        this.transactions = null;

        this.filters = new TransactionFilters(this.updateTable.bind(this));
        this.table = new Table();
        this.truncated = el(
            "article.message.is-warning",
            el("div.message-body", `Truncated to ${DISPLAY_LIMIT} rows`),
            { hidden: true }
        );
        this.el = el("div.container.is-fluid", [
            this.filters,
            this.table,
            this.truncated
        ]);
    }

    async onmount(): Promise<void> {
        if (this.transactions != null) {
            return;
        }

        await this.loadTransactions();
        this.filters.pushSelections();
    }

    async loadTransactions(): Promise<void> {
        this.transactions = await loadTransactions();

        let accounts: Set<string> = new Set();
        for (const transaction of this.transactions) {
            accounts.add(transaction[0]);
        }

        this.filters.set_accounts(accounts);
    }

    updateTable(accounts: Set<string>): void {
        if (this.transactions === null) {
            return;
        }

        let rows = this.transactions
            .values()
            .filter((row, _idx) => accounts.has(row[0]))
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
            this.truncated.removeAttribute("hidden");
        } else {
            this.truncated.setAttribute("hidden", "true");
        }
    }
}
