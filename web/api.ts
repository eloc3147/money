export const HEADER_OPTIONS = [
    "-",
    "Date",
    "Name",
    "Description",
    "Amount",
];

export interface AddUploadResponse {
    upload_id: string,
    headers: string[],
    header_suggestions: string[],
}

export async function add_upload(file_contents: string | ArrayBuffer): Promise<AddUploadResponse> {
    return await fetch("/api/upload/", { method: "post", body: file_contents })
        .then(async (resp) => await resp.json() as AddUploadResponse);
}
