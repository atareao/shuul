import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography, Table } from 'antd';
import type { TableProps } from 'antd';
const { Text } = Typography;

import { loadData } from '@/common/utils';
import type Record  from "@/models/record";

interface Props {
    navigate: any
    t: any
}

interface State {
    records: Record[]
    loading: boolean
}

export class InnerPage extends react.Component<Props, State> {

    constructor(props: Props) {
        super(props);
        this.state = {
            records: [],
            loading: false,
        }
    }

    columns: TableProps<Record>['columns'] = [
        {
            title: this.props.t("Date"),
            dataIndex: 'created_at',
            key: 'created_at',
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("IP"),
            dataIndex: 'ip_address',
            key: 'ip_address',
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("Protocol"),
            dataIndex: 'protocol',
            key: 'protocol',
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("FQDN"),
            dataIndex: 'fqdn',
            key: 'fqdn',
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("Path"),
            dataIndex: 'path',
            key: 'path',
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("Query"),
            dataIndex: 'query',
            key: 'query',
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("City name"),
            dataIndex: 'city_name',
            key: 'city_name',
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("County name"),
            dataIndex: 'country_name',
            key: 'country_name',
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("County code"),
            dataIndex: 'country_code',
            key: 'country_code',
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("Rule ID"),
            dataIndex: 'rule_id',
            key: 'rule_id',
            render: (text) => <Text>{text}</Text>,
        },
    ];

    componentDidMount = async () => {
        console.log("Mounting page");
        const responseJson = await loadData("records" );
        if (responseJson.status === 200 && responseJson.data) {
            this.setState({ records: responseJson.data, loading: false });
        }

    }

    render = () => {
        const title = this.props.t("Records");
        if (this.state.loading) {
            <Text>{this.props.t("Loading...")}</Text>
        }
        return (
            <Flex vertical justify="center" align="center" >
                <Text>{title}</Text>
                <div style={{ height: 20 }} />
                <Table<Record>
                    columns={this.columns}
                    dataSource={this.state.records}
                />
            </Flex>

        );
    }
};

export default function RecordsPage() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerPage navigate={navigate} t={t} />;
}
