export interface Dictionary<T> {
    [name: string]: T | undefined | null;
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

export type LanguageCode = "es" | "en";

// Define el objeto de constantes para usar como valores
export const Language = {
    ES: "es" as LanguageCode,
    EN: "en" as LanguageCode,
};

export type CrudAction = "create" | "read" | "update" | "delete";

export const Crud = {
    CREATE: "create" as CrudAction,
    READ:   "read"   as CrudAction,
    UPDATE: "update" as CrudAction,
    DELETE: "delete" as CrudAction,
};
