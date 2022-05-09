import { MoneyError } from "./money";

export const HEADER_OPTIONS = [
    "-",
    "Date",
    "Name",
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


export interface AddUploadResponse {
    upload_id: string,
    headers: string[],
    header_suggestions: string[],
}

export interface GetUploadRowsResponse {
    rows: string[][],
}

async function api_request(uri: RequestInfo, init_data: RequestInit): Promise<any> {
    let resp = await fetch(uri, init_data)
        .then(async (resp) => await resp.json() as MoneyResponse);

    if (resp.status == "ok") {
        return (resp as MoneyMsg).response;
    } else if (resp.status == "error") {
        throw new MoneyApiError((resp as MoneyErrorMsg).msg, uri)
    } else {
        throw new MoneyApiError(`Unexpected response status: ${resp.status}`, uri)
    }
}

export async function add_upload(file_contents: string | ArrayBuffer): Promise<AddUploadResponse> {
    return await api_request("/api/upload/", { method: "post", body: file_contents }) as AddUploadResponse;
}

}
