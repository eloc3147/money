import { MoneyError } from "../money";

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


export async function api_post(endpoint: RequestInfo, body: BodyInit | null, content_type: string): Promise<any> {
    return await api_request(
        endpoint, { method: "post", body: body, headers: new Headers({ "content-type": content_type }) }
    );
}


export async function api_json_post(endpoint: RequestInfo, body: any): Promise<any> {
    return await api_post(endpoint, JSON.stringify(body), "application/json");
}


export async function api_get(endpoint: string, parameters?: Record<string, string>): Promise<any> {
    if (typeof parameters !== 'undefined') {
        endpoint += "?" + new URLSearchParams(parameters);
    }

    return await api_request(endpoint, { method: "get" });
}
