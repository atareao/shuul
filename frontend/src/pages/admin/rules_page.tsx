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
    { key: 'id', label: 'Id', type: 'number', value: 0, editable: false, fixed: 'left', width: 80, tab: 'General' },
    { key: 'active', label: 'Active', type: 'boolean', value: true, width: 80, visible: true, tab: 'General' },
    { key: 'allow', label: 'Allow', type: 'boolean', value: false, width: 80, visible: true, tab: 'General' },
    { key: 'store', label: 'Store', type: 'boolean', value: true, width: 80, visible: true, tab: 'General' },
    { key: 'weight', label: 'Weight', type: 'number', value: 100, width: 80, visible: true, tab: 'General' },
    { key: 'ip_address', label: 'IP Address', type: 'string', value: "", width: 150, filterKey: "ip_address", visible: true, tab: 'Match' },
    { key: 'protocol', label: 'Protocol', type: 'string', value: "", width: 120, filterKey: "protocol", visible: true, tab: 'Match' },
    { key: 'fqdn', label: 'FQDN', type: 'string', value: "", width: 200, filterKey: "fqdn", visible: true, tab: 'Match' },
    { key: 'path', label: 'Path', type: 'string', value: "", width: 140, filterKey: "path", visible: true, tab: 'Match' },
    { key: 'query', label: 'Query', type: 'string', value: "", width: 180, filterKey: "query", visible: true, tab: 'Match' },
    { key: 'city_name', label: 'City Name', type: 'string', value: "", width: 150, filterKey: "city_name", visible: true, tab: 'Match' },
    { key: 'country_name', label: 'Contry Name', type: 'string', value: "", width: 150, filterKey: "country_name", visible: true, tab: 'Match' },
    { key: 'country_code', label: 'Contry Code', type: 'string', value: "", width: 150, filterKey: "country_code", visible: true, tab: 'Match' },
    // Rate limiting fields
    { key: 'rate_limit_enabled', label: 'Rate Limit', type: 'boolean', value: false, width: 100, visible: true, tab: 'Rate Limit' },
    { key: 'max_retry', label: 'Max Retry', type: 'number', value: 5, width: 100, visible: true, tab: 'Rate Limit' },
    { key: 'find_time_seconds', label: 'Find Time (s)', type: 'number', value: 600, width: 120, visible: true, tab: 'Rate Limit' },
    { key: 'ban_time_seconds', label: 'Ban Time (s)', type: 'number', value: 3600, width: 120, visible: true, tab: 'Rate Limit' },
    { key: 'bantime_increment', label: 'Escalate', type: 'boolean', value: false, width: 100, visible: true, tab: 'Rate Limit' },
    { key: 'bantime_multipliers', label: 'Multipliers', type: 'string', value: "", width: 120, visible: true, tab: 'Rate Limit' },
    { key: 'bantime_maxtime_seconds', label: 'Max Ban (s)', type: 'number', value: 604800, width: 120, visible: true, tab: 'Rate Limit' },
    { key: 'ban_count_decay_days', label: 'Decay (d)', type: 'number', value: 30, width: 100, visible: true, tab: 'Rate Limit' },
    { key: 'ignoreip', label: 'Ignore IPs', type: 'string', value: "", width: 150, visible: true, tab: 'Rate Limit' },
    { key: 'webhook', label: 'Webhook', type: 'string', value: "", width: 200, visible: false, tab: 'General' },
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
