import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography, Table, Input } from 'antd';
import type { GetProp, TableProps } from 'antd';
import type { SorterResult } from 'antd/es/table/interface';
const { Text } = Typography;

import { loadData, mapsEqual } from '@/common/utils';
import type { Dictionary } from '@/common/types';
import type Record from "@/models/record";

type ColumnsType<T extends object = object> = TableProps<T>['columns'];
type TablePaginationConfig = Exclude<GetProp<TableProps, 'pagination'>, boolean>;

interface TableParams {
    pagination?: TablePaginationConfig;
    sortField?: SorterResult<any>['field'];
    sortOrder?: SorterResult<any>['order'];
}


interface Props {
    navigate: any
    t: any
}

interface State {
    records: Record[];
    loading: boolean;
    tableParams: TableParams;
    filters: Map<string, string>;
}
const COLUMNS: string[] = ["created_at", "ip_address", "protocol", "fqdn",
    "path", "query", "city_name", "country_name", "country_code", "rule_id"];

export class InnerPage extends react.Component<Props, State> {

    constructor(props: Props) {
        super(props);
        const initialFilters = new Map<string, string>();
        COLUMNS.forEach(col => initialFilters.set(col, ""));
        this.state = {
            records: [],
            loading: false,
            tableParams: {
                pagination: {
                    current: 1,
                    pageSize: 10,
                },
            },
            filters: initialFilters,
        }
        console.log("Initial state:", this.state);
    }

    getColumns = (): ColumnsType<Record> => {
        return COLUMNS.map((col) => {
            const filterValue = this.state.filters.get(col) || "";
            return {
                title: <Input
                    key={`filter-input-${col}-${this.state.filters.get(col)}`}
                    placeholder={col}
                    defaultValue={filterValue}
                    onKeyUp={(e) => {
                        if(e.key === "Enter") {
                            const value = (e.target as HTMLInputElement).value;
                            console.log(`Filter ${col} by value: ${value}`);
                            console.log(this.state.filters);
                            console.log(e);
                            this.setState((prevState) => {
                                const newFilters = new Map(prevState.filters);
                                newFilters.set(col, value);
                                console.log(newFilters);
                                return {
                                    filters: newFilters
                                }
                            },
                                ()=>{
                                    console.log(`Actualizado con Enter: ${Array.from(this.state.filters.entries())}`);
                                    this.fetchData();
                                }
                            );
                        }; // Handled in onPressEnter
                    }}
                />,
                dataIndex: col,
                key: col,
                sorter: true,
                render: (text) => <Text>{text}</Text>,
            }
        })
    }

    setTableParams = (tableParams: TableParams) => {
        this.setState({ ...this.state, tableParams });
    }
    setData = (records: Record[]) => {
        this.setState({ ...this.state, records });
    }

    handleTableChange: TableProps<Record>['onChange'] = (
        pagination: any,
        sorter: any) => {
        this.setTableParams({
            pagination,
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
        Object.entries(this.state.filters).forEach(([key, value]) => {
            if (value && value.length > 0) {
                params[key] = value;
            }
        });
        console.log(`Fetch params: ${JSON.stringify(params)}`);
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
        console.log("Component did update");
        console.log("filters", prevState.filters, this.state.filters)
        const filtersHaveChanged = !mapsEqual(prevState.filters, this.state.filters);
        if (prevState.tableParams.pagination?.current !== this.state.tableParams.pagination?.current ||
            this.state.tableParams.pagination?.pageSize !== prevState.tableParams.pagination?.pageSize ||
            this.state.tableParams.sortField !== prevState.tableParams.sortField ||
            this.state.tableParams.sortOrder !== prevState.tableParams.sortOrder ||
            filtersHaveChanged
        ) {
            await this.fetchData();
        }
    }

    render = () => {
        const title = this.props.t("Records");
        if (this.state.loading) {
            <Text>{this.props.t("Loading...")}</Text>
        }
        const columns = this.getColumns();
        return (
            <Flex vertical justify="center" align="center" >
                <Text>{title}</Text>
                <div style={{ height: 20 }} />
                <Table<Record>
                    columns={columns}
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
