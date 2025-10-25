import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography, Table, Input, Button } from 'antd';
import type { GetProp, TableProps, TableColumnsType } from 'antd';
import type { SorterResult } from 'antd/es/table/interface';
import { EditFilled, DeleteFilled, CheckOutlined, CloseOutlined } from '@ant-design/icons';
const { Text } = Typography;
type TablePaginationConfig = Exclude<GetProp<TableProps, 'pagination'>, boolean>;

import { loadData, mapsEqual } from '@/common/utils';
import type { DialogMode } from '@/common/types';
import { DialogModes } from '@/common/types';
import type Item from "@/models/ignored";
import ItemDialog from "@/components/ignored_dialog";

const TITLE = "Ignored";
const ENDPOINT = "ignored";
const FIELDS = [
        { key: 'id', label: 'Id', type: 'number', value: 0 },
        { key: 'active', label: 'Active', type: 'boolean', value: true },
        { key: 'ip_address', label: 'IP Address', type: 'string', value: "" },
        { key: 'protocol', label: 'Protocol', type: 'string', value: "" },
        { key: 'fqdn', label: 'FQDN', type: 'string', value: "" },
        { key: 'path', label: 'Path', type: 'string', value: "" },
        { key: 'query', label: 'Query', type: 'string', value: "" },
        { key: 'city_name', label: 'City Name', type: 'string', value: "" },
        { key: 'country_name', label: 'Contry Name', type: 'string', value: "" },
        { key: 'country_code', label: 'Contry Code', type: 'string', value: "" },
    ]

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
    selectedItem?: Item,
    dialogMode: DialogMode,
}

export class InnerPage extends react.Component<Props, State> {
    columns: TableColumnsType<Item>;

    constructor(props: Props) {
        super(props);
        const initialFilters = new Map<string, string>();
        FIELDS.map(field => initialFilters.set(field.key, ""));
        this.state = {
            items: [],
            loading: false,
            pagination: {
                current: 1,
                pageSize: 10,
            },
            filters: initialFilters,
            dialogMode: DialogModes.NONE,
        }
        console.log("Initial state:", this.state);
        this.columns = this.getColumns();
    }

    getColumns = (): TableColumnsType<Item> => {
        let columns = FIELDS.map((field): {
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
                        <Text>{this.props.t(field.label)}</Text>
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
        columns.push({
            title: <Text>Edit</Text>,
            key: "operation-edit",
            render: (rule: Item) => (
                <Flex justify="center" align="center">
                    <Button
                        onClick={async () => {
                            console.log("Edit rule", rule);
                            this.setState({
                                selectedItem: rule,
                                dialogMode: DialogModes.UPDATE,
                            });
                        }}
                    >
                        <EditFilled />
                    </Button>
                    <Button
                        onClick={async () => {
                            console.log("Delete rule", rule);
                            this.setState({
                                selectedItem: rule,
                                dialogMode: DialogModes.DELETE,
                            });
                        }}
                    >
                        <DeleteFilled />
                    </Button>
                </Flex>
            )
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
            <>
                {this.state.dialogMode !== DialogModes.NONE &&
                    <ItemDialog
                        item={this.state.selectedItem}
                        dialogMode={this.state.dialogMode}
                        onClose={(ok: boolean, rule) => {
                            console.log("nada", ok)
                            if (rule) {
                                if (this.state.dialogMode == DialogModes.DELETE) {
                                    this.setState({
                                        dialogMode: DialogModes.NONE,
                                        selectedItem: undefined,
                                        items: [
                                            ...this.state.items.filter((r) => r.id !== rule.id),
                                        ],
                                    });
                                } else if (this.state.dialogMode == DialogModes.UPDATE) {
                                    this.setState({
                                        dialogMode: DialogModes.NONE,
                                        selectedItem: undefined,
                                        items: [
                                            ...this.state.items.map((r) => {
                                                if (r.id === rule.id) {
                                                    return { ...r, ...rule };
                                                }
                                                return r;
                                            })
                                        ]
                                    });
                                } else if (this.state.dialogMode == DialogModes.CREATE) {
                                    this.setState({
                                        dialogMode: DialogModes.NONE,
                                        selectedItem: undefined,
                                        items: [
                                            ...this.state.items,
                                            rule
                                        ]
                                    });
                                }
                            } else {
                                this.setState({
                                    dialogMode: DialogModes.NONE,
                                    selectedItem: undefined,
                                });
                            }
                        }
                        }
                    />
                }
                <Flex vertical justify="center" align="center" gap="middle" >
                    <Flex justify="center" align="center" gap="middle" >
                        <Text>{title}</Text>
                        <Button
                            type="primary"
                            onClick={() => {
                                console.log("Clicked Add button");
                                this.setState({ dialogMode: DialogModes.CREATE });
                            }}
                        >
                            {this.props.t("Add Rule")}
                        </Button>
                    </Flex>
                    <Table<Item>
                        columns={this.columns}
                        rowKey={rule => rule.id}
                        dataSource={this.state.items}
                        sortDirections={this.state.sortOrder ? [this.state.sortOrder] : ['ascend', 'descend']}
                        pagination={this.state.pagination}
                        loading={this.state.loading}
                        onChange={this.handleTableChange}
                    />
                </Flex>
            </>
        );
    }
};

export default function Page() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerPage navigate={navigate} t={t} />;
}

