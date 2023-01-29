import { api_post, api_get, api_json_post } from "./base";

export interface AddUploadResponse {
    upload_id: string,
    headers: string[],
    header_suggestions: string[],
    row_count: number
}

export async function add_upload(file: File): Promise<AddUploadResponse> {
    return await api_post("upload/", file, "application/octet-stream") as AddUploadResponse;
}


export interface GetUploadRowsResponse {
    cells: string[]
}

export async function get_upload_rows(
    upload_id: string,
    row_index: number,
    row_count: number
): Promise<GetUploadRowsResponse> {
    return await api_get(
        `upload/${upload_id}/rows`, { row_index: row_index.toString(), row_count: row_count.toString() }
    ) as GetUploadRowsResponse;
}


export interface GetUploadRowsResponse {
    rows: string[]
}

export async function submit_upload(upload_id: string, header_selections: string[]): Promise<GetUploadRowsResponse> {
    return await api_json_post(
        `upload/${upload_id}/submit`, { header_selections: header_selections }
    ) as GetUploadRowsResponse;
}
