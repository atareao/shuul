import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Flex } from "antd";
import { EyeOutlined } from '@ant-design/icons';
import CustomTable from '@/components/custom_table'; 
import type { FieldDefinition } from '@/common/types'; 
import type Item from "@/models/record"; 
import RequestDetailDialog from '@/components/dialogs/request_detail_dialog';

const TITLE = "Requests";
const ENDPOINT = "requests";

// Simplified fields — only show: created_at, rule_name, ip_address, fqdn, path, user_agent, country_name
// Widths are deliberately tight to force ellipsis truncation
const RecordsFields: FieldDefinition<Item>[] = [
    { key: 'created_at', label: 'Created at', type: 'date', width: 120}, 
    { key: 'rule_name', label: 'Rule', type: 'string', width: 120 },
    { key: 'ip_address', label: 'IP Address', type: 'string', filterKey: 'ip_address', width: 120 },
    { key: 'fqdn', label: 'FQDN', type: 'string', filterKey: 'fqdn', width: 140 },
    { key: 'path', label: 'Path', type: 'string', filterKey: 'path', width: 140 },
    { key: 'user_agent', label: 'Agent', type: 'string', filterKey: 'user_agent', width: 140 },
    { key: 'country_name', label: 'Country', type: 'string', filterKey: 'country_name', width: 100 },
];

export default function Page() {
    const { t } = useTranslation();
    const [detailRecord, setDetailRecord] = useState<Item | null>(null);

    // Adapt labels using translation function
    const translatedFields: FieldDefinition<Item>[] = RecordsFields.map(field => ({
        ...field,
        label: t(field.label),
    }));

    return (
        <>
            <RequestDetailDialog
                record={detailRecord}
                onClose={() => setDetailRecord(null)}
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
                    <Flex gap="small">
                        <Button
                            size="small"
                            icon={<EyeOutlined />}
                            onClick={() => setDetailRecord(item)}
                        >
                            {t('Details')}
                        </Button>
                    </Flex>
                )}
            />
        </>
    );
}