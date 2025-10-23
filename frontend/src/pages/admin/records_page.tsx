import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography, Table, Input } from 'antd';
import type { GetProp, TableProps, TableColumnsType } from 'antd';
import type { SorterResult } from 'antd/es/table/interface';
import { CheckOutlined, CloseOutlined } from '@ant-design/icons';
const { Text } = Typography;
type TablePaginationConfig = Exclude<GetProp<TableProps, 'pagination'>, boolean>;

import { loadData, mapsEqual, toCapital } from '@/common/utils';
import type Item from "@/models/record";

const TITLE = "Records";
const ENDPOINT = "records";


interface Props {
    navigate: any
    t: any
}

interface State {
    items: Item[];
    loading: boolean;
    pagination?: TablePaginationConfig;
    sortField?: SorterResult<any>['field'];
    sortOrder?: SorterResult<any>['order'];
    filters: Map<string, string>;
}

export class InnerPage extends react.Component<Props, State> {
    columns: TableColumnsType<Item>;

    constructor(props: Props) {
        super(props);
        const initialFilters = new Map<string, string>();
        this.fields.map(field => initialFilters.set(field.key, ""));
        this.state = {
            items: [],
            loading: false,
            pagination: {
                current: 1,
                pageSize: 10,
            },
            filters: initialFilters,
        }
        console.log("Initial state:", this.state);
        this.columns = this.getColumns();
    }

    fields = [
        { key: 'created_at', label: this.props.t('Created at'), type: 'date', value: new Date()},
        { key: 'ip_address', label: this.props.t('IP Address'), type: 'string', value: "" },
        { key: 'protocol', label: this.props.t('Protocol'), type: 'string', value: "" },
        { key: 'fqdn', label: this.props.t('FQDN'), type: 'string', value: "" },
        { key: 'path', label: this.props.t('Path'), type: 'string', value: "" },
        { key: 'query', label: this.props.t('Query'), type: 'string', value: "" },
        { key: 'city_name', label: this.props.t('City Name'), type: 'string', value: "" },
        { key: 'country_name', label: this.props.t('Contry Name'), type: 'string', value: "" },
        { key: 'country_code', label: this.props.t('Contry Code'), type: 'string', value: "" },
        { key: 'rule_id', label: this.props.t('Rule Id'), type: 'number', value: 0 },
    ];

    getColumns = (): TableColumnsType<Item> => {
        let columns = this.fields.map((field): {
            title: React.ReactNode | string;
            dataIndex?: string;
            key: string;
            sorter?: (a: Item, b: Item) => number;
            ellipsis?: boolean;
            render?: (text: any) => React.ReactNode;
        } => {
            const filterValue = this.state.filters.get(field.key) || "";
            return {
                title:
                    <Flex vertical justify="flex-end" align="left" gap="middle" >
                        <Text>{toCapital(field.key.replaceAll("_", " "))}</Text>
                        {(field.type === 'string' || field.type === 'number') &&
                            <Input
                                key={`filter-input-${field.key}-${this.state.filters.get(field.key)}`}
                                placeholder={field.key}
                                defaultValue={filterValue}
                                onKeyUp={(e) => {
                                    if (e.key === "Enter") {
                                        const value = (e.target as HTMLInputElement).value;
                                        console.log(`Filter ${field.key} by value: ${value}`);
                                        console.log(this.state.filters);
                                        console.log(e);
                                        this.setState((prevState) => {
                                            const newFilters = new Map(prevState.filters);
                                            newFilters.set(field.key, value.trim().replaceAll("*", "%"));
                                            console.log(newFilters);
                                            return {
                                                filters: newFilters
                                            }
                                        },
                                            () => {
                                                console.log(`Actualizado con Enter: ${Array.from(this.state.filters.entries())}`);
                                            }
                                        );
                                    }; // Handled in onPressEnter
                                }}
                            />
                        }
                    </Flex>,
                dataIndex: field.key,
                key: field.key,
                sorter: (a: any, b: any) => {
                    const columnName = field.key as keyof Item;
                    if (!a && !b) return 0; // Both are undefined/null, consider them equal
                    if (!a) return 1; // Undefined 'a' goes to the end (or -1 if you prefer it at the start)
                    if (!b) return -1; // Undefined 'b' goes to the end (or 1 if you prefer it at the start)
                    const valA = a[columnName];
                    const valB = b[columnName];
                    if (valA === valB) return 0; // Equality
                    if (!valA && !valB) return 0; // Both are undefined/null, consider them equal
                    if (!valA) return 1; // Undefined 'a' goes to the end (or -1 if you prefer it at the start)
                    if (!valB) return -1; // Undefined 'b' goes to the end (or 1 if you prefer it at the start)
                    return valA > valB ? 1 : -1;
                },
                ellipsis: true,
                render: (content: any) => {
                    if (field.type === 'boolean') {
                        if (content) {
                            return <CheckOutlined />
                        } else {
                            return <CloseOutlined />
                        }
                    } else {
                        return <Text>{content}</Text>
                    }
                }
            }
        });
        return columns;
    }

    setData = (items: Item[]) => {
        this.setState({ ...this.state, ...items });
    }

    handleTableChange: TableProps<Item>['onChange'] = async (
        pagination: any,
        filters: any,
        sorter: any,
        extra: any,
    ) => {
        console.log("=== Table change detected. Start ===");
        console.log(`pagination: ${JSON.stringify(pagination)}`);
        console.log(`filters: ${JSON.stringify(filters)}`);
        console.log(`sorter: ${JSON.stringify(sorter)}`);
        console.log(`extra: ${JSON.stringify(extra)}`);
        console.log("=== Table change detected. End ===");
        this.setState({
            pagination: {
                ...this.state.pagination,
                ...pagination
            },
            sortOrder: sorter.order,
            sortField: sorter.field,
        });
        if (pagination.pageSize !== this.state.pagination?.pageSize) {
            this.setData([]);
        }
    }

    fetchData = async () => {
        console.log("Fetching data");
        let sortBy = this.state.sortField;
        if (sortBy === undefined || sortBy.toString().trim() === '') {
            sortBy = 'created_at';
        } else {
            sortBy = sortBy.toString().trim();
        }
        const params: Map<string, string> = new Map([
            ["page", this.state.pagination?.current?.toString() || "1"],
            ["limit", this.state.pagination?.pageSize?.toString() || "10"],
            ["sort_by", sortBy],
            ["asc", this.state.sortOrder === 'ascend' ? 'true' : 'false'],
        ]);
        console.log("Current filters:", this.state.filters);
        this.state.filters.forEach((value, key) => {
            console.log(`Filter entry: ${key} -> ${value}`);
            if (value && value.length > 0) {
                params.set(key, value);
            }
        });
        console.log(`Fetch params: ${JSON.stringify(params)}`);
        const responseJson = await loadData<Item[]>(ENDPOINT, params);
        console.log(`Response: ${JSON.stringify(responseJson)}`);
        if (responseJson.status === 200 && responseJson.data) {
            this.setState({
                items: responseJson.data,
                loading: false,
                pagination: {
                    ...this.state.pagination,
                    current: responseJson.pagination?.page || 1,
                    pageSize: responseJson.pagination?.limit || 10,
                    total: responseJson.pagination?.records || 0,
                }
            });
        }
    }

    componentDidMount = async () => {
        await this.fetchData();
    }

    componentDidUpdate = async (_prevProps: Props, prevState: State) => {
        console.log("Component did update");
        console.log(`sortOrder: prev=${prevState.sortOrder} curr=${this.state.sortOrder}`);
        console.log(`sortField: prev=${prevState.sortField} curr=${this.state.sortField}`);
        const filtersHaveChanged = !mapsEqual(prevState.filters, this.state.filters);
        if (prevState.pagination?.current !== this.state.pagination?.current ||
            this.state.pagination?.pageSize !== prevState.pagination?.pageSize ||
            this.state.sortField !== prevState.sortField ||
            this.state.sortOrder !== prevState.sortOrder ||
            filtersHaveChanged
        ) {
            await this.fetchData();
        }
    }

    render = () => {
        const title = this.props.t(TITLE);
        if (this.state.loading) {
            <Text>{this.props.t("Loading...")}</Text>
        }
        return (
            <Flex vertical justify="center" align="center" >
                <Text>{title}</Text>
                <div style={{ height: 20 }} />
                <Table<Item>
                    columns={this.columns}
                    rowKey={record => record.id.toString()}
                    dataSource={this.state.items}
                    sortDirections={this.state.sortOrder ? [this.state.sortOrder] : ['ascend', 'descend']}
                    pagination={this.state.pagination}
                    loading={this.state.loading}
                    onChange={this.handleTableChange}
                />
            </Flex>

        );
    }
};

export default function Page() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerPage navigate={navigate} t={t} />;
}
