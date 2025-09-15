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

export class TransactionsPage implements RedomComponent {
    transactions: TransactionsResponse | null;
    table: Table;
    truncated: HTMLElement;
    el: HTMLElement;

    constructor() {
        this.transactions = null;

        this.table = new Table();
        this.truncated = el(
            "article.message.is-warning",
            el("div.message-body", `Truncated to ${DISPLAY_LIMIT} rows`),
            { hidden: true }
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

        this.transactions = await loadTransactions();
        this.updateTable();
    }

    updateTable(): void {
        if (this.transactions === null) {
            return;
        }

        let rows = this.transactions;

        // TODO: Filter

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
