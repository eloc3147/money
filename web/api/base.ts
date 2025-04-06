import { MoneyError } from "../money";


interface MoneyMsg<R> {
    status: string,
    response: R
}


interface MoneyErrorMsg {
    status: string,
    msg: string
}


type MoneyResponse<R> = MoneyMsg<R> | MoneyErrorMsg;


class MoneyApiError extends MoneyError {
    endpoint: RequestInfo;

    constructor(message: string, endpoint: RequestInfo) {
        super(message);
        this.endpoint = endpoint;
    }
}

async function api_request<R>(endpoint: RequestInfo, init_data: RequestInit): Promise<R> {
    const resp = await fetch("/api/" + endpoint, init_data)
        .then(async (resp) => await resp.json() as MoneyResponse<R>);

    if (resp.status == "ok") {
        return (resp as MoneyMsg<R>).response;
    } else if (resp.status == "error") {
        throw new MoneyApiError((resp as MoneyErrorMsg).msg, endpoint)
    } else {
        throw new MoneyApiError(`Unexpected response status: ${resp.status}`, endpoint)
    }
}


export async function api_post<R>(endpoint: RequestInfo, body: BodyInit | null, content_type: string): Promise<R> {
    return await api_request(
        endpoint, { method: "post", body: body, headers: new Headers({ "content-type": content_type }) }
    );
}


export async function api_json_post<B, R>(endpoint: RequestInfo, body: B): Promise<R> {
    return await api_post(endpoint, JSON.stringify(body), "application/json");
}


export async function api_get<R>(endpoint: string, parameters?: Record<string, string>): Promise<R> {
    if (typeof parameters !== 'undefined') {
        endpoint += "?" + new URLSearchParams(parameters);
    }

    return await api_request(endpoint, { method: "get" });
}
