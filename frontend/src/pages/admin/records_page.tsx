import { useTranslation } from "react-i18next";
import { CustomTable } from '@/components/custom_table'; 
import type { FieldDefinition } from '@/common/types'; 
import type Item from "@/models/record"; 

const TITLE = "Records";
const ENDPOINT = "records";

// Definición de las columnas/campos para la tabla de Records
const RecordsFields: FieldDefinition<Item>[] = [
    { key: 'created_at', label: 'Created at', type: 'date'}, 
    { key: 'ip_address', label: 'IP Address', type: 'string' },
    { key: 'protocol', label: 'Protocol', type: 'string' },
    { key: 'fqdn', label: 'FQDN', type: 'string' },
    { key: 'path', label: 'Path', type: 'string' },
    { key: 'query', label: 'Query', type: 'string' },
    { key: 'city_name', label: 'City Name', type: 'string' },
    { key: 'country_name', label: 'Country Name', type: 'string' }, 
    { key: 'country_code', label: 'Country Code', type: 'string' },
    { key: 'rule_id', label: 'Rule Id', type: 'number' },
];


export default function Page() {
    const { t } = useTranslation();
    // No necesitamos 'navigate' si no se usa

    // Adaptamos las etiquetas para usar la función 't'
    const translatedFields: FieldDefinition<Item>[] = RecordsFields.map(field => ({
        ...field,
        label: t(field.label),
    }));

    return (
        // Usamos el CustomTable con el tipo Item para la inferencia de TypeScript
        <CustomTable<Item>
            title={TITLE}
            endpoint={ENDPOINT}
            fields={translatedFields}
            t={t}
        />
    );
}
