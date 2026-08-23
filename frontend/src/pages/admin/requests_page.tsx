import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "antd";
import { PlusOutlined } from '@ant-design/icons';
import CustomTable from '@/components/custom_table'; 
import type { FieldDefinition } from '@/common/types'; 
import type Item from "@/models/record"; 
import CreateRuleFromRequestDialog from '@/components/dialogs/create_rule_from_request_dialog';

const TITLE = "Requests";
const ENDPOINT = "requests";

// Definición de las columnas/campos para la tabla de Records
const RecordsFields: FieldDefinition<Item>[] = [
    { key: 'created_at', label: 'Created at', type: 'date', width: 170}, 
    { key: 'ip_address', label: 'IP Address', type: 'string', filterKey: 'ip_address', width: 140 },
    { key: 'protocol', label: 'Protocol', type: 'string', filterKey: 'protocol', width: 100 },
    { key: 'fqdn', label: 'FQDN', type: 'string', filterKey: 'fqdn', width: 200 },
    { key: 'path', label: 'Path', type: 'string', filterKey: 'path', width: 250 },
    { key: 'query', label: 'Query', type: 'string', filterKey: 'query', width: 200 },
    { key: 'city_name', label: 'City Name', type: 'string', filterKey: 'city_name', width: 150 },
    { key: 'country_name', label: 'Country Name', type: 'string', filterKey: 'country_name', width: 150 },
    { key: 'country_code', label: 'Country Code', type: 'string', filterKey: 'country_code', width: 130 },
    { key: 'rule_id', label: 'Rule Id', type: 'number', filterKey: 'rule_id', width: 80, fixed: 'right' }
];


export default function Page() {
    const { t } = useTranslation();
    const [ruleDialogItem, setRuleDialogItem] = useState<Item | null>(null);

    // Adaptamos las etiquetas para usar la función 't'
    const translatedFields: FieldDefinition<Item>[] = RecordsFields.map(field => ({
        ...field,
        label: t(field.label),
    }));

    return (
        <>
            <CreateRuleFromRequestDialog
                record={ruleDialogItem}
                onClose={() => setRuleDialogItem(null)}
                t={t}
            />
            <CustomTable<Item>
                title={TITLE}
                endpoint={ENDPOINT}
                fields={translatedFields}
                t={t}
                defaultSortField="created_at"
                defaultSortDesc={true}
                autoRefresh={true}
                autoRefreshInterval={30}
                hasActions={true}
                renderActionColumn={(item) => (
                    <Button
                        size="small"
                        icon={<PlusOutlined />}
                        onClick={() => setRuleDialogItem(item)}
                    >
                        {t('Rule')}
                    </Button>
                )}
            />
        </>
    );
}
