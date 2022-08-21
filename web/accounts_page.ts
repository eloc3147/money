import { el } from "redom";
import { ColumnView, Page, Table, Tr } from "./components";
import { get_accounts } from "./api";


export class AccountsPage implements Page {
    el: ColumnView;
    title: HTMLParagraphElement;
    table: Table | null;

    constructor() {
        this.title = el("p", { class: "title is-1" }, "Accounts");
        this.table = null;
        this.el = new ColumnView("is-half", [this.title]);
    }

    onmount() {
        this.update_list();
    }

    onremount() {
        this.update_list();
    }

    async update_list() {
        let accounts = await get_accounts();

        this.table = new Table(["Name"]);
        this.table.add_plain_rows(accounts.accounts.map((a) => [a]))
        this.el.set_contents([this.title, this.table]);
    }
}
