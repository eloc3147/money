/*jshint esversion: 6 */

import { el, setChildren } from "https://redom.js.org/redom.es.min.js";
import { Table, ColumnView } from "/components.js";


export class UploadPage {
    constructor(args) {
        this.client = args.client;
        this.session = null;
        this.preview = null;
        this.submit_button = null;
        this.error_label = null;
        this.submit_wrapper = null;

        this.el = new ColumnView("is-half", [
            this.title = el("p", { class: "title is-1" }, "Add Transactions"),
            this.subtitle = el("p", { class: "subtitle is-3" }, "Select a file"),
            el("div.field", [
                el("label.label", "File upload"),
                el("div.control", this.file_field = el("input.input", { type: "file" }))
            ]),
            el("div.field",
                el("div.control", this.load_button = el("button", { class: "button is-link" }, "Load file"))
            )
        ]);

        this.load_button.onclick = evt => {
            evt.preventDefault();

            var reader = new FileReader();
            reader.onloadend = _evt => {
                if (reader == null) {
                    console.log("Error: reader is null.");
                    return;
                }

                this.session = this.client.load_file(reader);
                this.draw_preview();
            };

            reader.readAsText(this.file_field.files[0]);
        };
    }

    draw_preview() {
        let row_count = Math.min(10, this.session.get_row_count());

        this.preview = new Table();
        this.preview.set_contents(
            this.session.get_headers().map(h => {
                return '"' + h + '"';
            }),
            this.session.get_header_suggestions(),
            this.session.get_row_slice(0, row_count),
            (column_index, selection) => {
                this.process_update(column_index, selection);
            }
        );

        this.subtitle.innerText = "Select the types of each column";

        this.el.set_column_args("is-full");
        this.el.set_contents([
            this.title,
            this.subtitle,
            el("article", { className: "message is-danger" },
                this.error_label = el("div", { className: "message-body is-hidden" }, "")
            ),
            this.preview,
            el("div", { className: "field is-grouped" }, [
                this.submit_wrapper = el("fieldset",
                    el("div.control",
                        this.submit_button = el("button", { class: "button is-link" }, "Load file")
                    )
                )
            ]),
        ]);

        this.submit_button.onclick = evt => {
            evt.preventDefault();
            if (this.session.get_selection_error() === undefined) {
                // this.session.submit_data();
                this.draw_submitted();
            }
        };

        this.file_field = null;
        this.load_button = null;
    }

    process_update(column_index, selection) {
        this.session.update_header_selection(column_index, selection);

        let selection_error = this.session.get_selection_error();
        if (selection_error !== undefined) {
            this.error_label.textContent = selection_error;
            this.error_label.className = "message-body";
            this.submit_wrapper.setAttribute("disabled", true);
        } else {
            this.error_label.textContent = "";
            this.error_label.className = "message-body is-hidden";
            this.submit_wrapper.removeAttribute("disabled");
        }
    }

    draw_submitted() {
        this.error_label.textContent = "Data submitted.";
        this.error_label.className = "message-body";

        this.el.set_contents([
            this.title,
            el("article", { className: "message is-primary is-large" }, [
                el("div.message-header", "Upload complete"),
                el("div.message-body", "You can now return to the home page")
            ])
        ]);
    }
}