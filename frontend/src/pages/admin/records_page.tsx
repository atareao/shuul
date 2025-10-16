import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography, Table, Button, Input, Space } from 'antd';
import type { GetProp, TableProps, InputRef, TableColumnType, TableColumnsType } from 'antd';
import type { SorterResult, FilterDropdownProps } from 'antd/es/table/interface';
import { SearchOutlined } from '@ant-design/icons';
import Highlighter from 'react-highlight-words';
const { Text } = Typography;

import { loadData } from '@/common/utils';
import type { Dictionary } from '@/common/types';
import type Record from "@/models/record";

type ColumnsType<T extends object = object> = TableProps<T>['columns'];
type TablePaginationConfig = Exclude<GetProp<TableProps, 'pagination'>, boolean>;

interface TableParams {
    pagination?: TablePaginationConfig;
    sortField?: SorterResult<any>['field'];
    sortOrder?: SorterResult<any>['order'];
    filters?: Parameters<GetProp<TableProps, 'onChange'>>[1];
}

type RecordIndex = keyof Record;


interface Props {
    navigate: any
    t: any
}

interface State {
    records: Record[];
    loading: boolean;
    tableParams: TableParams;
    searchText?: string;
    searchedColumn?: RecordIndex;
}

export class InnerPage extends react.Component<Props, State> {
    searchInput: InputRef = react.createRef<InputRef>();

    constructor(props: Props) {
        super(props);
        this.state = {
            records: [],
            loading: false,
            tableParams: {
                pagination: {
                    current: 1,
                    pageSize: 10,
                }
            },
        }
    }

    handleSearch = (
        selectedKeys: string[],
        confirm: FilterDropdownProps['confirm'],
        recordIndex: RecordIndex,
    ) => {
        confirm();
        this.setState({
            searchText: selectedKeys[0],
            searchedColumn: recordIndex,
        });
    };

    handleReset = (clearFilters: () => void) => {
        clearFilters();
        this.setState({
            searchText: "",
        });
    };

    getColumnSearchProps = (dataIndex: RecordIndex): TableColumnType<Record> => ({
        filterDropdown: ({ setSelectedKeys, selectedKeys, confirm, clearFilters, close }) => (
            <div style={{ padding: 8 }} onKeyDown={(e) => e.stopPropagation()}>
                <Input
                    ref={searchInput}
                    placeholder={`Search ${dataIndex}`}
                    value={selectedKeys[0]}
                    onChange={(e) => setSelectedKeys(e.target.value ? [e.target.value] : [])}
                    onPressEnter={() => this.handleSearch(selectedKeys as string[], confirm, dataIndex)}
                    style={{ marginBottom: 8, display: 'block' }}
                />
                <Space>
                    <Button
                        type="primary"
                        onClick={() => this.handleSearch(selectedKeys as string[], confirm, dataIndex)}
                        icon={<SearchOutlined />}
                        size="small"
                        style={{ width: 90 }}
                    >
                        Search
                    </Button>
                    <Button
                        onClick={() => clearFilters && this.handleReset(clearFilters)}
                        size="small"
                        style={{ width: 90 }}
                    >
                        Reset
                    </Button>
                    <Button
                        type="link"
                        size="small"
                        onClick={() => {
                            confirm({ closeDropdown: false });
                            this.setState({ searchText: (selectedKeys as string[])[0], searchedColumn: dataIndex });
                        }}
                    >
                        Filter
                    </Button>
                    <Button
                        type="link"
                        size="small"
                        onClick={() => {
                            close();
                        }}
                    >
                        close
                    </Button>
                </Space>
            </div>
        ),
        filterIcon: (filtered: boolean) => (
            <SearchOutlined style={{ color: filtered ? '#1677ff' : undefined }} />
        ),
        onFilter: (value, record) =>
            record[dataIndex]
                .toString()
                .toLowerCase()
                .includes((value as string).toLowerCase()),
        filterDropdownProps: {
            onOpenChange(open) {
                if (open) {
                    setTimeout(() => searchInput.current?.select(), 100);
                }
            },
        },
        render: (text) =>
            searchedColumn === dataIndex ? (
                <Highlighter
                    highlightStyle={{ backgroundColor: '#ffc069', padding: 0 }}
                    searchWords={[searchText]}
                    autoEscape
                    textToHighlight={text ? text.toString() : ''}
                />
            ) : (
                text
            ),
    });

    columns: ColumnsType<Record> = [
        {
            title: this.props.t("Date"),
            dataIndex: 'created_at',
            key: 'created_at',
            sorter: true,
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("IP"),
            dataIndex: 'ip_address',
            key: 'ip_address',
            sorter: true,
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("Protocol"),
            dataIndex: 'protocol',
            key: 'protocol',
            sorter: true,
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("FQDN"),
            dataIndex: 'fqdn',
            key: 'fqdn',
            sorter: true,
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("Path"),
            dataIndex: 'path',
            key: 'path',
            sorter: true,
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("Query"),
            dataIndex: 'query',
            key: 'query',
            sorter: true,
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("City name"),
            dataIndex: 'city_name',
            key: 'city_name',
            sorter: true,
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("County name"),
            dataIndex: 'country_name',
            key: 'country_name',
            sorter: true,
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("County code"),
            dataIndex: 'country_code',
            key: 'country_code',
            sorter: true,
            render: (text) => <Text>{text}</Text>,
        },
        {
            title: this.props.t("Rule ID"),
            dataIndex: 'rule_id',
            key: 'rule_id',
            render: (text) => <Text>{text}</Text>,
        },
    ];

    setTableParams = (tableParams: TableParams) => {
        this.setState({ ...this.state, tableParams });
    }
    setData = (records: Record[]) => {
        this.setState({ ...this.state, records });
    }

    handleTableChange: TableProps<Record>['onChange'] = (
        pagination: any,
        filters: any,
        sorter: any) => {
        this.setTableParams({
            pagination,
            filters,
            sortOrder: Array.isArray(sorter) ? undefined : sorter.order,
            sortField: Array.isArray(sorter) ? undefined : sorter.field,
        });
        if (pagination.pageSize !== this.state.tableParams.pagination?.pageSize) {
            this.setData([]);
        }
    }

    fetchData = async () => {
        console.log("Fetching data");
        const params: Dictionary<string | number> = {
            page: this.state.tableParams.pagination?.current || 1,
            limit: this.state.tableParams.pagination?.pageSize || 10,
            sort_by: this.state.tableParams.sortField?.toString() || 'created_at',
            asc: this.state.tableParams.sortOrder === 'ascend' ? 'true' : 'false',
        };
        const countryCode = this.state.tableParams.filters?.country_code?.toString() || "";
        if (countryCode.length > 0) {
            params['country_code'] = countryCode;
        }
        const responseJson = await loadData<Record[]>("records", params);
        console.log(`Response: ${JSON.stringify(responseJson)}`);
        if (responseJson.status === 200 && responseJson.data) {
            this.setState({
                records: responseJson.data,
                loading: false,
                tableParams: {
                    ...this.state.tableParams,
                    pagination: {
                        current: responseJson.pagination?.page || 1,
                        pageSize: responseJson.pagination?.limit || 10,
                        total: responseJson.pagination?.records || 0,
                    }
                }
            });
        }
    }


    componentDidMount = async () => {
        await this.fetchData();
    }

    componentDidUpdate = async (_prevProps: Props, prevState: State) => {
        if (prevState.tableParams.pagination?.current !== this.state.tableParams.pagination?.current ||
            this.state.tableParams.pagination?.pageSize !== prevState.tableParams.pagination?.pageSize ||
            this.state.tableParams.sortField !== prevState.tableParams.sortField ||
            this.state.tableParams.sortOrder !== prevState.tableParams.sortOrder
        ) {
            await this.fetchData();
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
                    rowKey={record => record.id.toString()}
                    dataSource={this.state.records}
                    pagination={this.state.tableParams.pagination}
                    loading={this.state.loading}
                    onChange={this.handleTableChange}
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
