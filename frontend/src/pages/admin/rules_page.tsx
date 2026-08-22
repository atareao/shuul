import React from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Button, Space } from 'antd';
import { EditFilled, DeleteFilled, PlusOutlined } from '@ant-design/icons';
import type Item from "@/models/rule"; // Alias para Rule

// Importamos CustomTable y los tipos necesarios
import CustomTable from '@/components/custom_table';
import type { FieldDefinition } from '@/common/types';
import type { DialogMessages } from '@/components/dialogs/custom_dialog';

// 1. Constantes de configuración (fuera de la clase)
const TITLE = "Rules";
const ENDPOINT = "rules";

// Definición de los campos (tipados para Item, que es Rule)
const FIELDS: FieldDefinition<Item>[] = [
    { key: 'id', label: 'Id', type: 'number', value: 0, editable: false, fixed: 'left', width: 80 },
    { key: 'active', label: 'Active', dialogTab: 'General', type: 'boolean', value: true, width: 80, visible: true },
    { key: 'allow', label: 'Allow', dialogTab: 'General', type: 'boolean', value: false, width: 80, visible: true },
    { key: 'store', label: 'Store', dialogTab: 'General', type: 'boolean', value: true, width: 80, visible: true },
    { key: 'weight', label: 'Weight', dialogTab: 'General', type: 'number', value: 100, width: 80, visible: true },
    { key: 'ip_address', label: 'IP Address', dialogTab: 'Matching', type: 'string', value: "", width: 150, filterKey: "ip_address", visible: true },
    { key: 'protocol', label: 'Protocol', dialogTab: 'Matching', type: 'string', value: "", width: 120, filterKey: "protocol", visible: true },
    { key: 'fqdn', label: 'FQDN', dialogTab: 'Matching', type: 'string', value: "", width: 200, filterKey: "fqdn", visible: true },
    { key: 'path', label: 'Path', dialogTab: 'Matching', type: 'string', value: "", width: 140, filterKey: "path", visible: true },
    { key: 'query', label: 'Query', dialogTab: 'Matching', type: 'string', value: "", filterKey: "query", visible: true },
    { key: 'city_name', label: 'City Name', dialogTab: 'Matching', type: 'string', value: "", width: 150, filterKey: "city_name", visible: true },
    { key: 'country_name', label: 'Country Name', dialogTab: 'Matching', type: 'string', value: "", width: 150, filterKey: "country_name", visible: true },
    { key: 'country_code', label: 'Country Code', dialogTab: 'Matching', type: 'string', value: "", width: 150, filterKey: "country_code", visible: true },
    // Rate limiting fields
    { key: 'rate_limit_enabled', label: 'Rate Limit', dialogTab: 'Rate limiting', type: 'boolean', value: false, width: 100, visible: true, help: 'Enable rate limiting for this rule' },
    { key: 'max_retry', label: 'Max Retry', dialogTab: 'Rate limiting', type: 'number', value: 5, width: 100, visible: true, min: 1, help: 'Requests allowed before ban' },
    { key: 'find_time_seconds', label: 'Find Time (s)', dialogTab: 'Rate limiting', type: 'number', value: 600, width: 120, visible: true, min: 1, help: 'Sliding window in seconds' },
    { key: 'ban_time_seconds', label: 'Ban Time (s)', dialogTab: 'Rate limiting', type: 'number', value: 3600, width: 120, visible: true, min: 1, help: 'Base ban duration in seconds' },
    { key: 'bantime_increment', label: 'Escalate', dialogTab: 'Rate limiting', type: 'boolean', value: false, width: 100, visible: true, help: 'Increase ban time for repeat offenses' },
    { key: 'bantime_multipliers', label: 'Multipliers', dialogTab: 'Rate limiting', type: 'string', value: "1,2,4,8", width: 120, visible: true, help: 'Comma-separated values, e.g. 1,2,4,8' },
    { key: 'bantime_maxtime_seconds', label: 'Max Ban (s)', dialogTab: 'Rate limiting', type: 'number', value: 604800, width: 120, visible: true, min: 1, help: 'Upper limit for escalated bans' },
    { key: 'ban_count_decay_days', label: 'Decay (d)', dialogTab: 'Rate limiting', type: 'number', value: 30, width: 100, visible: true, min: 1, help: 'Days before escalation resets' },
    { key: 'ignoreip', label: 'Ignore IPs', dialogTab: 'Rate limiting', type: 'string', value: "", width: 150, visible: true, help: 'Comma-separated IP addresses to ignore' },
    { key: 'webhook', label: 'Webhook', dialogTab: 'Rate limiting', type: 'string', value: "", width: 200, visible: true, help: 'Optional HTTP(S) endpoint called for ban events' },
];

// Mensajes específicos para el CustomDialog de Rules
const RULE_DIALOG_MESSAGES: DialogMessages = {
    createTitle: 'Create Rule',
    readTitle: 'View Rule',
    updateTitle: 'Update Rule',
    deleteTitle: 'Delete Rule',
    confirmDeleteMessage: (id: number | string) => `Are you sure you want to delete rule "${id}"?`,
};

// 2. Definición de Props y Clase
interface Props {
    navigate: any; // Propiedad de useNavigate (aunque no se usa aquí)
    t: (key: string) => string; // Propiedad de useTranslation
}

// La clase ya no necesita State, ya que CustomTable maneja el estado de la tabla.
export class InnerPage extends React.Component<Props, {}> { 

    // 3. Método para renderizar el botón "Añadir"
    private renderHeaderAction = (onCreate: () => void) => {
        return (
            <Button
                type="primary"
                onClick={onCreate} // Llama al manejador interno de CustomTable para abrir el diálogo CREATE
                icon={<PlusOutlined />}
            >
                {this.props.t("Add Rule")}
            </Button>
        );
    };

    // 4. Método para renderizar la columna de acciones
    private renderActionColumn = (item: Item, onEdit: (item: Item) => void, onDelete: (item: Item) => void) => {
        return (
            <Space size="middle">
                <Button onClick={() => onEdit(item)} title={this.props.t('Edit')}>
                    <EditFilled />
                </Button>
                <Button onClick={() => onDelete(item)} title={this.props.t('Delete')} danger>
                    <DeleteFilled />
                </Button>
            </Space>
        );
    };

    // 5. El método render ahora solo devuelve el CustomTable
    render = () => {
        // La clase ya no tiene this.state, this.columns, fetchData, etc.
        // Toda la complejidad se delega a CustomTable.
        return (
            <CustomTable<Item> 
                title={TITLE}
                endpoint={ENDPOINT}
                fields={FIELDS}
                dialogMessages={RULE_DIALOG_MESSAGES} 
                t={this.props.t}
                hasActions={true}
                renderHeaderAction={this.renderHeaderAction}
                renderActionColumn={this.renderActionColumn}
            />
        );
    }
}

// 6. Componente funcional (wrapper) para conectar Hooks
export default function Page() {
    const navigate = useNavigate();
    // useTranslation debe estar en un componente funcional o en un componente de clase con un wrapper
    const { t } = useTranslation(); 
    return <InnerPage navigate={navigate} t={t} />;
}
