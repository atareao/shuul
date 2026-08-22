import React from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Button, Space } from 'antd';
import { DeleteFilled, PlusOutlined } from '@ant-design/icons';
import type Ban from "@/models/ban";
import CustomTable from '@/components/custom_table';
import type { FieldDefinition } from '@/common/types';
import type { DialogMessages } from '@/components/dialogs/custom_dialog';
const TITLE = "Active Bans";
const ENDPOINT = "bans";

const FIELDS: FieldDefinition<Ban>[] = [
    { key: 'id', label: 'Id', type: 'number', value: 0, editable: false, fixed: 'left', width: 80, visible: false },
    { key: 'ip_address', label: 'IP Address', type: 'string', value: "", width: 150, visible: true, required: true },
    { key: 'jail_name', label: 'Jail', type: 'string', value: "manual", width: 120, visible: false },
    { key: 'banned_at', label: 'Banned At', type: 'string', value: "", width: 220, visible: false, editable: false },
    { key: 'reason', label: 'Reason', type: 'string', value: "", width: 200, visible: true },
    { key: 'ban_duration_seconds', label: 'Duration (s)', type: 'number', value: 3600, width: 120, visible: true, min: 1 },
    { key: 'time_remaining_seconds', label: 'Remaining (s)', type: 'number', value: 0, width: 140, visible: false, editable: false },
    { key: 'escalation_level', label: 'Level', type: 'number', value: 0, width: 80, visible: false },
    { key: 'expired', label: 'Expired', type: 'boolean', value: false, width: 100, visible: false, editable: false },
];

const BAN_DIALOG_MESSAGES: DialogMessages = {
    createTitle: 'Ban IP',
    readTitle: 'View Ban',
    updateTitle: 'Update Ban',
    deleteTitle: 'Unban IP',
    confirmDeleteMessage: (id: number | string) =>
        `Are you sure you want to unban IP "${id}"?`,
};

export class InnerPage extends React.Component<{ navigate: any; t: any }, {}> {
    private renderHeaderAction = (onCreate: () => void) => {
        return (
            <Button type="primary" onClick={onCreate} icon={<PlusOutlined />}>
                {this.props.t("Ban IP")}
            </Button>
        );
    };

    private renderActionColumn = (item: Ban, _onEdit: any, onDelete: (item: Ban) => void) => {
        return (
            <Space size="middle">
                <Button onClick={() => onDelete(item)} title={this.props.t('Unban')} danger>
                    <DeleteFilled />
                </Button>
            </Space>
        );
    };

    render = () => {
        return (
            <CustomTable<Ban>
                title={TITLE}
                endpoint={ENDPOINT}
                fields={FIELDS}
                dialogMessages={BAN_DIALOG_MESSAGES}
                t={this.props.t}
                hasActions={true}
                renderHeaderAction={this.renderHeaderAction}
                renderActionColumn={this.renderActionColumn}
            />
        );
    }
}

export default function Page() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerPage navigate={navigate} t={t} />;
}