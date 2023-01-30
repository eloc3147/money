import { el, mount, RedomComponent, setChildren } from "redom";
import { Table, ColumnView, OptionConfig, Page, Tr, TdDropdown, Td } from "./components";
import { HEADER_OPTIONS, REQUIRED_HEADERS } from "./api/base";
import { add_upload, get_upload_rows, submit_upload } from "./api/upload";


export class UploadPage implements Page {
    title: HTMLParagraphElement;
    subtitle: HTMLParagraphElement;
    error_label: HTMLDivElement;
    error_box: HTMLElement;
    upload_view: HTMLElement;

    input_field: HTMLDivElement;
    load_field: HTMLDivElement;

    show_more_wrapper: HTMLFieldSetElement;
    submit_wrapper: HTMLFieldSetElement;
    show_more_button: HTMLButtonElement;
    submit_button: HTMLButtonElement;

    el: ColumnView;

    constructor() {
        this.subtitle = el("p", { class: "subtitle is-3" }, "Select a file");
        this.error_label = el("div", { className: "message-body is-hidden" }, "");
        this.error_box = el("article", { className: "message is-danger" }, this.error_label);
        this.upload_view = el("div");

        this.el = new ColumnView("is-half", [
            el("p", { class: "title is-1" }, "Add Transactions"),
            el("hr"),
            this.subtitle,
            this.error_box,
            this.upload_view
        ]);
    }

    onmount() {
        mount(this.upload_view, new UploadSelect(this));
    }

    onremount() {
        this.el.set_column_args("is-half");
        this.set_error(null);
        this.set_subtitle("Select a file");
        setChildren(this.upload_view, [
            new UploadSelect(this)
        ]);
    }

    set_error(error_msg: string | null) {
        if (error_msg !== null) {
            this.error_label.textContent = error_msg;
            this.error_label.className = "message-body";
        } else {
            this.error_label.textContent = "";
            this.error_label.className = "message-body is-hidden";
        }
    }

    set_subtitle(subtitle: string) {
        this.subtitle.innerText = subtitle;
    }

    async load_file(file: File) {
        await add_upload(file)
            .then((resp) => {
                this.set_error(null);
                this.set_subtitle("Select the types of each column");

                this.el.set_column_args("is-full");
                setChildren(this.upload_view, [
                    new UploadPreview(this, resp.upload_id, resp.headers, resp.header_suggestions, resp.row_count)
                ]);
            })
    }

    draw_submitted() {
        this.el.set_column_args("is-half");
        this.set_error(null);
        this.set_subtitle("");
        setChildren(this.upload_view, [
            new UploadSubmitted()
        ]);
    }
}


class UploadSelect implements RedomComponent {
    el: HTMLDivElement;

    upload_page: UploadPage;
    file_field: HTMLInputElement;
    load_button: HTMLButtonElement;

    constructor(upload_page: UploadPage) {
        this.upload_page = upload_page;

        this.file_field = el("input", { type: "file", class: "input" });
        this.load_button = el("button", { class: "button is-link" }, "Load file");
        this.el = el("div", [
            el("div.field", [
                el("label.label", "File upload"),
                el("div.control", this.file_field)
            ]),
            el("div.field", el("div.control", this.load_button))
        ]);

        this.load_button.onclick = async (evt) => {
            evt.preventDefault();

            if (this.file_field.files == null || this.file_field.files.length != 1) {
                this.upload_page.set_error("Please select one file to upload.");
                return
            }

            this.upload_page.set_error(null);
            await this.upload_page.load_file(this.file_field.files[0]);
        };
    }
}


class UploadPreview implements RedomComponent {
    upload_page: UploadPage;

    upload_id: string;
    header_suggestions: string[];
    required_headers: string[];
    column_count: number;
    upload_row_count: number;

    current_row_count: number;
    header_selections: string[];

    el: HTMLDivElement;
    table: Table;
    show_more_button: HTMLButtonElement;
    show_more_wrapper: HTMLFieldSetElement;
    submit_button: HTMLButtonElement;
    submit_wrapper: HTMLFieldSetElement;

    constructor(
        upload_page: UploadPage,
        upload_id: string,
        headers: string[],
        header_suggestions: string[],
        row_count: number
    ) {
        this.upload_page = upload_page;

        this.upload_id = upload_id;
        this.header_suggestions = header_suggestions;
        this.column_count = headers.length;
        this.upload_row_count = row_count;

        this.current_row_count = 0;
        this.header_selections = this.header_suggestions;

        let option_configs = header_suggestions.map((suggestion) => {
            return HEADER_OPTIONS.map(option => {
                return {
                    value: option,
                    selected: option == suggestion
                } as OptionConfig;
            });
        });

        this.table = new Table(headers.map(h => '"' + h + '"'));

        let suggestion_row = new Tr(TdDropdown);
        suggestion_row.update(
            option_configs,
            { callback: (column_index, selection) => this.process_update(column_index, selection) }
        );
        this.table.add_rows([suggestion_row]);

        this.el = el("div", [
            this.table,
            el("div", { className: "field is-grouped" }, [
                this.show_more_wrapper = el("fieldset",
                    el("div.control",
                        this.show_more_button = el("button", { class: "button" }, "Show More")
                    )
                ),
                this.submit_wrapper = el("fieldset",
                    el("div.control",
                        this.submit_button = el("button", { class: "button is-link" }, "Submit")
                    )
                )
            ]),
        ]);

        this.show_more_button.onclick = evt => {
            evt.preventDefault();
            this.add_rows();
        };

        this.submit_button.onclick = async evt => {
            evt.preventDefault();
            if (!this.check_error()) {
                await submit_upload(this.upload_id, this.header_selections);
                this.upload_page.draw_submitted();
            }
        };

        this.add_rows();
        this.check_error();
    }

    async add_rows(): Promise<void> {
        let remaining_rows = Math.max(0, this.upload_row_count - this.current_row_count);
        let row_count = Math.min(10, remaining_rows);

        if (row_count > 0) {
            let resp = await get_upload_rows(this.upload_id, this.current_row_count, row_count);
            let rows: Tr[] = [];
            for (let i = 0; i < resp.cells.length; i += this.column_count) {
                let row = new Tr(Td);
                row.update(resp.cells.slice(i, i + this.column_count));
                rows.push(row);
            }
            this.table.add_rows(rows);
            this.current_row_count += row_count;
        }

        if (this.current_row_count == this.upload_row_count) {
            this.show_more_wrapper.setAttribute("disabled", "true");
        }
    }

    process_update(column_index: number, selection: string): void {
        this.header_selections[column_index] = selection;
        this.check_error();
    }

    check_error(): boolean {
        let missing_required: string[] = [];
        for (let i in REQUIRED_HEADERS) {
            if (!this.header_selections.includes(REQUIRED_HEADERS[i])) {
                missing_required.push(REQUIRED_HEADERS[i]);
            }
        }

        if (missing_required.length == 0) {
            this.upload_page.set_error(null);
            this.submit_wrapper.removeAttribute("disabled");
            return false;
        } else {
            this.upload_page.set_error(`Missing required headers: ${missing_required.join(", ")}`);
            this.submit_wrapper.setAttribute("disabled", "true");
            return true;
        }
    }
}


class UploadSubmitted implements RedomComponent {
    el: HTMLElement;

    constructor() {
        this.el = el("article", { className: "message is-primary is-large" }, [
            el("div.message-header", "Upload complete"),
            el("div.message-body", "You can now return to the home page")
        ]);
    }
}
