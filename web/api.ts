import { MoneyError } from "./money";


export const HEADER_OPTIONS = [
    "-",
    "Date",
    "Name",
    "Description",
    "Amount",
];


export const REQUIRED_HEADERS = [
    "Date",
    "Description",
    "Amount",
];


interface MoneyMsg {
    status: string,
    response: any
}


interface MoneyErrorMsg {
    status: string,
    msg: string
}


type MoneyResponse = MoneyMsg | MoneyErrorMsg;


class MoneyApiError extends MoneyError {
    endpoint: RequestInfo;

    constructor(message: string, endpoint: RequestInfo) {
        super(message);
        this.endpoint = endpoint;
    }
}


// Base functions
async function api_request(endpoint: RequestInfo, init_data: RequestInit): Promise<any> {
    let resp = await fetch("/api/" + endpoint, init_data)
        .then(async (resp) => await resp.json() as MoneyResponse);

    if (resp.status == "ok") {
        return (resp as MoneyMsg).response;
    } else if (resp.status == "error") {
        throw new MoneyApiError((resp as MoneyErrorMsg).msg, endpoint)
    } else {
        throw new MoneyApiError(`Unexpected response status: ${resp.status}`, endpoint)
    }
}


async function api_post(endpoint: RequestInfo, body: BodyInit | null, content_type: string): Promise<any> {
    return await api_request(
        endpoint, { method: "post", body: body, headers: new Headers({ "content-type": content_type }) }
    );
}


async function api_json_post(endpoint: RequestInfo, body: any): Promise<any> {
    return await api_post(endpoint, JSON.stringify(body), "application/json");
}


async function api_get(endpoint: string, parameters?: Record<string, string>): Promise<any> {
    if (typeof parameters !== 'undefined') {
        endpoint += "?" + new URLSearchParams(parameters);
    }

    return await api_request(endpoint, { method: "get" });
}


// Upload endpoints
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


// Account endpoints
export interface ListAccountsResponse {
    accounts: string[]
}

export async function get_accounts(): Promise<ListAccountsResponse> {
    return await api_get("account/") as ListAccountsResponse;
}

export async function add_account(name: string): Promise<void> {
    await api_json_post("account/", { name: name });
}