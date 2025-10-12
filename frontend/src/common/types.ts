export interface Dictionary<T> {
    [name: string]: T;
}

export interface Validation {
    check: Function;
    msg: string;
}

export interface Item {
    value: number;
    label: string;
}

export interface ReferencedItem {
    value: number;
    label: string;
    reference: string;
}

export enum Language {
    ES = "es",
    EN = "en",
}
