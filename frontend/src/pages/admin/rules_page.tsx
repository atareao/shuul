import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography, Table, Input, Button } from 'antd';
import type { GetProp, TableProps, TableColumnsType } from 'antd';
import type { SorterResult } from 'antd/es/table/interface';
import { EditFilled, DeleteFilled, CheckOutlined, CloseOutlined } from '@ant-design/icons';
const { Text } = Typography;

import { loadData, mapsEqual, toCapital } from '@/common/utils';
import type { DialogMode } from '@/common/types';
import { DialogModes } from '@/common/types';
import type Rule from "@/models/rule";
import RuleDialog from "@/components/rule_dialog";

type TablePaginationConfig = Exclude<GetProp<TableProps, 'pagination'>, boolean>;

interface Props {
    navigate: any
    t: any
}

interface State {
    rules: Rule[];
    loading: boolean;
    pagination?: TablePaginationConfig;
    sortField?: SorterResult<any>['field'];
    sortOrder?: SorterResult<any>['order'];
    filters: Map<string, string>;
    selectedRule?: Rule,
    ruleDialogMode: DialogMode,
}

export class InnerPage extends react.Component<Props, State> {
    columns: TableColumnsType<Rule>;

    constructor(props: Props) {
        super(props);
        const initialFilters = new Map<string, string>();
        this.fields.map(field => initialFilters.set(field.key, ""));
        this.state = {
            rules: [],
            loading: false,
            pagination: {
                current: 1,
                pageSize: 10,
            },
            filters: initialFilters,
            ruleDialogMode: DialogModes.NONE,
        }
        console.log("Initial state:", this.state);
        this.columns = this.getColumns();
    }

    fields = [
        { key: 'id', label: this.props.t('Id'), type: 'number', value: 0 },
        { key: 'active', label: this.props.t('Active'), type: 'boolean', value: true },
        { key: 'allow', label: this.props.t('Allow'), type: 'boolean', value: false },
        { key: 'weight', label: this.props.t('Weight'), type: 'number', value: 100 },
        { key: 'ip_address', label: this.props.t('IP Address'), type: 'string', value: "" },
        { key: 'protocol', label: this.props.t('Protocol'), type: 'string', value: "" },
        { key: 'fqdn', label: this.props.t('FQDN'), type: 'string', value: "" },
        { key: 'path', label: this.props.t('Path'), type: 'string', value: "" },
        { key: 'query', label: this.props.t('Query'), type: 'string', value: "" },
        { key: 'city_name', label: this.props.t('City Name'), type: 'string', value: "" },
        { key: 'country_name', label: this.props.t('Contry Name'), type: 'string', value: "" },
        { key: 'country_code', label: this.props.t('Contry Code'), type: 'string', value: "" },
    ]

    getColumns = (): TableColumnsType<Rule> => {
        let columns = this.fields.map((field): {
            title: React.ReactNode | string;
            dataIndex?: string;
            key: string;
            sorter?: (a: Rule, b: Rule) => number;
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
                    const columnName = field.key as keyof Rule;
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
            render: (rule: Rule) => (
                <Flex justify="center" align="center">
                    <Button
                        onClick={async () => {
                            console.log("Edit rule", rule);
                            this.setState({
                                selectedRule: rule,
                                ruleDialogMode: DialogModes.UPDATE,
                            });
                        }}
                    >
                        <EditFilled />
                    </Button>
                    <Button
                        onClick={async () => {
                            console.log("Delete rule", rule);
                            this.setState({
                                selectedRule: rule,
                                ruleDialogMode: DialogModes.DELETE,
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

    setData = (rules: Rule[]) => {
        this.setState({ ...this.state, rules });
    }

    handleTableChange: TableProps<Rule>['onChange'] = async (
        pagination: any,
        filters: any,
        sorter: any,
        extra: any,
    ) => {
        console.log("=== Table change detected ===");
        console.log(`pagination: ${JSON.stringify(pagination)}`);
        console.log(`filters: ${JSON.stringify(filters)}`);
        console.log(`sorter: ${JSON.stringify(sorter)}`);
        console.log(`extra: ${JSON.stringify(extra)}`);
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
        const responseJson = await loadData<Rule[]>("rules", params);
        console.log(`Response: ${JSON.stringify(responseJson)}`);
        if (responseJson.status === 200 && responseJson.data) {
            this.setState({
                rules: responseJson.data,
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
        const title = this.props.t("Rules");
        if (this.state.loading) {
            <Text>{this.props.t("Loading...")}</Text>
        }
        return (
            <>
                {this.state.ruleDialogMode !== DialogModes.NONE &&
                    <RuleDialog
                        rule={this.state.selectedRule}
                        dialogMode={this.state.ruleDialogMode}
                        onClose={(ok: boolean, rule) => {
                            console.log("nada", ok)
                            if (rule) {
                                if (this.state.ruleDialogMode == DialogModes.DELETE) {
                                    this.setState({
                                        ruleDialogMode: DialogModes.NONE,
                                        selectedRule: undefined,
                                        rules: [
                                            ...this.state.rules.filter((r) => r.id !== rule.id),
                                        ],
                                    });
                                } else if (this.state.ruleDialogMode == DialogModes.UPDATE) {
                                    this.setState({
                                        ruleDialogMode: DialogModes.NONE,
                                        selectedRule: undefined,
                                        rules: [
                                            ...this.state.rules.map((r) => {
                                                if (r.id === rule.id) {
                                                    return { ...r, ...rule };
                                                }
                                                return r;
                                            })
                                        ]
                                    });
                                } else if (this.state.ruleDialogMode == DialogModes.CREATE) {
                                    this.setState({
                                        ruleDialogMode: DialogModes.NONE,
                                        selectedRule: undefined,
                                        rules: [
                                            ...this.state.rules,
                                            rule
                                        ]
                                    });
                                }
                            } else {
                                this.setState({
                                    ruleDialogMode: DialogModes.NONE,
                                    selectedRule: undefined,
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
                                console.log("Clicked Add Rule button");
                                this.setState({ ruleDialogMode: DialogModes.CREATE });
                            }}
                        >
                            {this.props.t("Add Rule")}
                        </Button>
                    </Flex>
                    <Table<Rule>
                        columns={this.columns}
                        rowKey={rule => rule.id}
                        dataSource={this.state.rules}
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

export default function RulesPage() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerPage navigate={navigate} t={t} />;
}

