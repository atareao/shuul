import React from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Button, Space } from 'antd';
import { EditFilled, DeleteFilled, PlusOutlined } from '@ant-design/icons';
import type Item from "@/models/rate_limit_profile";

// Importamos CustomTable y los tipos necesarios
import CustomTable from '@/components/custom_table';
import type { FieldDefinition } from '@/common/types';
import type { DialogMessages } from '@/components/dialogs/custom_dialog';
import RateLimitProfileDialog from '@/components/dialogs/rate_limit_profile_dialog';

// 1. Constantes de configuración
const TITLE = "Rate Limit Profiles";
const ENDPOINT = "rate-limit-profiles";

// Definición de los campos (tipados para Item, que es RateLimitProfile)
const FIELDS: FieldDefinition<Item>[] = [
    { key: 'id', label: 'Id', type: 'number', value: 0, editable: false, fixed: 'left', width: 80 },
    { key: 'name', label: 'Name', type: 'string', value: "", width: 150, filterKey: "name", visible: true },
    { key: 'description', label: 'Description', type: 'string', value: "", width: 200, filterKey: "description", visible: true },
    { key: 'max_retry', label: 'Max Retry', type: 'number', value: 5, width: 100, visible: true },
    { key: 'find_time_seconds', label: 'Find Time (s)', type: 'number', value: 600, width: 120, visible: true },
    { key: 'ban_time_seconds', label: 'Ban Time (s)', type: 'number', value: 3600, width: 120, visible: true },
    { key: 'bantime_increment', label: 'Escalate', type: 'boolean', value: false, width: 100, visible: true },
    { key: 'bantime_multipliers', label: 'Multipliers', type: 'string', value: "", width: 120, visible: true },
    { key: 'bantime_maxtime_seconds', label: 'Max Ban (s)', type: 'number', value: 604800, width: 120, visible: true },
    { key: 'ban_count_decay_days', label: 'Decay (d)', type: 'number', value: 30, width: 100, visible: true },
    { key: 'fail_codes', label: 'Fail Codes', type: 'string', value: "401,403,404", width: 150, visible: true },
];

// Mensajes específicos para el diálogo
const DIALOG_MESSAGES: DialogMessages = {
    createTitle: 'Create Rate Limit Profile',
    readTitle: 'View Rate Limit Profile',
    updateTitle: 'Update Rate Limit Profile',
    deleteTitle: 'Delete Rate Limit Profile',
    confirmDeleteMessage: (id: number | string) => `Are you sure you want to delete rate limit profile "${id}"?`,
};

// 2. Definición de Props y Clase
interface Props {
    navigate: any;
    t: (key: string) => string;
}

export class InnerPage extends React.Component<Props, {}> {

    // 3. Método para renderizar el botón "Añadir"
    private renderHeaderAction = (onCreate: () => void) => {
        return (
            <Button
                type="primary"
                onClick={onCreate}
                icon={<PlusOutlined />}
            >
                {this.props.t("Add Profile")}
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

    // 5. El método render
    render = () => {
        return (
            <CustomTable<Item>
                title={TITLE}
                endpoint={ENDPOINT}
                fields={FIELDS}
                dialogMessages={DIALOG_MESSAGES}
                t={this.props.t}
                hasActions={true}
                renderHeaderAction={this.renderHeaderAction}
                renderActionColumn={this.renderActionColumn}
                dialogRenderer={(params) => (
                    <RateLimitProfileDialog
                        dialogMode={params.dialogMode}
                        selectedItem={params.selectedItem}
                        handleCloseDialog={params.handleCloseDialog}
                        endpoint={params.endpoint}
                        dialogMessages={params.dialogMessages}
                        t={this.props.t}
                    />
                )}
            />
        );
    }
}

// 6. Componente funcional (wrapper) para conectar Hooks
export default function Page() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerPage navigate={navigate} t={t} />;
}