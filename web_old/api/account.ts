import { api_get, api_json_post } from "./base";

export interface ListAccountsResponse {
    accounts: string[]
}

export async function get_accounts(): Promise<ListAccountsResponse> {
    return await api_get("account/") as ListAccountsResponse;
}

export async function add_account(name: string): Promise<void> {
    await api_json_post("account/", { name: name });
}