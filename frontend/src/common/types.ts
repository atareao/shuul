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

export type DialogMode = "create" | "read" | "update" | "delete" | "none";

export const DialogModes = {
    CREATE: "create" as DialogMode,
    READ:   "read"   as DialogMode,
    UPDATE: "update" as DialogMode,
    DELETE: "delete" as DialogMode,
    NONE:   "none"   as DialogMode,
};
