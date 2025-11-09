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

type Path<T> = T extends ReadonlyArray<infer U> ? Path<U> : T;

type NestedKeyOf<T> = (
    T extends object ?
    { [K in keyof T]-?: K extends string ?
        | K 
        | (NestedKeyOf<Path<T[K]>> extends infer R ? R extends string ? `${K}.${R}` : never : never)
        : never
    }[keyof T] : never
) extends infer U ? Exclude<U, ''> : never;

export interface FieldDefinition<T>{
    key: NestedKeyOf<T> & string; // La clave debe ser una clave de T y también un string
    label: string;
    type: 'boolean' | 'number' | 'date' | 'string';
    value?: T[keyof T & string]; // Valor inicial de ese tipo
    customSorter?: (a: T, b: T) => number;
    render?: (content: any, record: T) => React.ReactNode; 
    editable?: boolean;
    filterKey?: string;
    fixed?: 'left' | 'right';
    width?: number;
    sortKey?: string;
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

export interface PuntoPreemerMarker {
    type: string;
    latitude: number;
    longitude: number;
    icon?: any;
    name: string;
    description: string;
    address?: string;
}
