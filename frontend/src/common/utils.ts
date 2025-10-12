import { BASE_URL } from '../constants';
import Response from '../models/response';
import { Dictionary } from './types';


export const loadData = async (endpoint: string, params?: Dictionary<string | number>) => {
    console.log("Loading data");
    const urlParams = new URLSearchParams();
    for (const key in params) {
        if (params[key] !== undefined) {
            urlParams.append(key, params[key].toString());
        }
    }
    console.log("URL Params", urlParams);
    try {
        const url = params ? `${BASE_URL}/api/v1/${endpoint}?${urlParams}` : `${BASE_URL}/api/v1/${endpoint}`;
        const response = await fetch(url, {
            method: 'GET',
            headers: {
                'Content-Type': 'application/json',
            },
        });
        return await response.json();
    } catch (error) {
        console.error('Error:', error);
        const msg = `Error: ${error}`;
        return new Response(500, msg);
    }
}
export const loadDataWithParams = async (endpoint: string, params: Dictionary<string>) => {
    console.log("Loading data");
    try {
        const response = await fetch(`${BASE_URL}/api/v1/${endpoint}?${params}`, {
            method: 'GET',
            headers: {
                'Content-Type': 'application/json',
            },
        });
        return await response.json();
    } catch (error) {
        console.error('Error:', error);
        const msg = `Error: ${error}`;
        return new Response(500, msg);
    }
}

export const loadKeyValue = async (reference: string) => {
    console.log("Loading key values");
    try {
        const response = await fetch(`${BASE_URL}/api/v1/values/reference/${reference}`, {
            method: 'GET',
            headers: {
                'Content-Type': 'application/json',
            },
        });
        return await response.json();
    } catch (error) {
        console.error('Error:', error);
        const msg = `Error: ${error}`;
        return new Response(500, msg);
    }
}

