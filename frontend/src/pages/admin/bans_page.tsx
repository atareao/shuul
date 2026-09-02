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
    { key: 'id', label: 'IP', type: 'string', value: "", width: 140, visible: true, filterKey: "ip_address", sortKey: "ip_address" },
    { key: 'reason', label: 'Reason', type: 'string', value: "", width: 250, visible: true },
    { key: 'ban_duration_seconds', label: 'Duration (s)', type: 'number', value: 0, width: 100, visible: true },
    { key: 'time_remaining_seconds', label: 'Remaining (s)', type: 'number', value: 0, width: 100, visible: true },
    { key: 'escalation_level', label: 'Level', type: 'number', value: 0, width: 60, visible: true },
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
                defaultSortField="ip_address"
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