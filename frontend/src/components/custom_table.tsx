import React from "react";
import { Table, Input, Flex, Typography } from 'antd';
import type { GetProp, TableProps, TableColumnsType } from 'antd';
import type { SorterResult } from 'antd/es/table/interface';
import { CheckOutlined, CloseOutlined } from '@ant-design/icons';
const { Text } = Typography;
type TablePaginationConfig = Exclude<GetProp<TableProps, 'pagination'>, boolean>;

import { loadData, mapsEqual } from '@/common/utils'; 
import type { DialogMode, FieldDefinition } from '@/common/types'; 
import { DialogModes } from '@/common/types'
import CustomDialog from '@/components/custom_dialog'; 
import type { DialogMessages, DialogProps } from '@/components/custom_dialog';

interface ActionProps<T> {
    renderActionColumn: (item: T, onEdit: (item: T) => void, onDelete: (item: T) => void) => React.ReactNode;
    renderHeaderAction: (onCreate: () => void) => React.ReactNode;
}

type Props<T extends { id: number | string }> = {
    title: string;
    endpoint: string;
    fields: FieldDefinition<T>[];
    dialogMessages?: DialogMessages;
    t: (key: string) => string;
    hasActions?: boolean;
} & Partial<ActionProps<T>>;

interface State<T> {
    items: T[];
    loading: boolean;
    pagination: TablePaginationConfig;
    sortField?: SorterResult<any>['field'];
    sortOrder?: SorterResult<any>['order'];
    filters: Map<string, string>;
    selectedItem?: T,
    dialogMode: DialogMode,
}


export class CustomTable<T extends { id: number | string }> extends React.Component<Props<T>, State<T>> {
    columns: TableColumnsType<T>; 

    constructor(props: Props<T>) {
        super(props);

        const initialFilters = new Map<string, string>();
        props.fields.forEach(field => initialFilters.set(field.key.toString(), ""));

        this.state = {
            items: [],
            loading: false,
            pagination: { current: 1, pageSize: 10, total: 0 },
            filters: initialFilters,
            dialogMode: DialogModes.NONE, 
            selectedItem: undefined,
        };

        this.columns = this.getColumns();
    }

    private handleEdit = (item: T) => {
        this.setState({ selectedItem: item, dialogMode: DialogModes.UPDATE });
    }

    private handleDelete = (item: T) => {
        this.setState({ selectedItem: item, dialogMode: DialogModes.DELETE });
    }

    private handleCreate = () => {
        this.setState({ dialogMode: DialogModes.CREATE, selectedItem: undefined });
    }

    private handleCloseDialog = (item: T | undefined) => { 
        if (item) {
            this.setState(prevState => {
                let newItems = [...prevState.items];
                if (prevState.dialogMode === DialogModes.DELETE) {
                    newItems = newItems.filter((r) => r.id !== item.id); 
                } else if (prevState.dialogMode === DialogModes.UPDATE) {
                    newItems = newItems.map((r) => r.id === item.id ? { ...r, ...item } : r);
                } else if (prevState.dialogMode === DialogModes.CREATE) {
                    newItems = [...newItems, item];
                }
                return {
                    ...prevState,
                    items: newItems,
                    dialogMode: DialogModes.NONE,
                    selectedItem: undefined,
                };
            });
        } else {
            this.setState({
                dialogMode: DialogModes.NONE,
                selectedItem: undefined,
            });
        }
    }

    getColumns = (): TableColumnsType<T> => {
        let columns: TableColumnsType<T> = this.props.fields.map((field) => {
            const filterValue = this.state.filters.get(field.key.toString()) || "";

            const handleFilterChange = (e: React.KeyboardEvent<HTMLInputElement>) => {
                if (e.key === "Enter") {
                    const value = (e.target as HTMLInputElement).value;
                    const cleanValue = value.trim().replaceAll("*", "%"); 
                    this.setState((prevState) => {
                        if (prevState.filters.get(field.key.toString()) === cleanValue) {
                            return prevState;
                        }
                        const newFilters = new Map(prevState.filters);
                        newFilters.set(field.key.toString(), cleanValue);
                        return {
                            ...prevState, 
                            filters: newFilters,
                            pagination: { ...prevState.pagination, current: 1 },
                            items: [], 
                            loading: true, 
                        };
                    }); 
                }
            };
            const defaultRender = (content: any) => {
                if (field.type === 'boolean') {
                    return content ? <CheckOutlined style={{ color: 'green' }}/> : <CloseOutlined style={{ color: 'red' }}/>;
                }
                return <Text>{content}</Text>
            };
            return {
                title: (
                    <Flex vertical justify="flex-end" align="left" gap="middle" >
                        <Text strong>{this.props.t(field.label)}</Text>
                        {(field.type === 'string' || field.type === 'number') &&
                            <Input
                                key={`filter-input-${field.key.toString()}-${filterValue}`} 
                                placeholder={this.props.t('Filter by') + ` ${field.label}...`}
                                defaultValue={filterValue.replaceAll("%", "*")} 
                                onKeyUp={handleFilterChange}
                            />
                        }
                    </Flex>
                ),
                dataIndex: field.key.toString(),
                key: field.key.toString(),
                sorter: field.customSorter || ((a: any, b: any) => {
                    const columnName = field.key as keyof T;
                    const valA = a?.[columnName];
                    const valB = b?.[columnName];
                    if (valA === valB) return 0;
                    if (valA == null) return 1;
                    if (valB == null) return -1;
                    return valA > valB ? 1 : -1;
                }),
                ellipsis: true,
                render: field.render || defaultRender,
            };
        });
        if (this.props.hasActions && this.props.renderActionColumn) {
            columns.push({
                title: <Text>{this.props.t('Actions')}</Text>, 
                key: "operation-actions",
                align: 'center',
                width: 100, 
                render: (item: T) => this.props.renderActionColumn!(item, this.handleEdit, this.handleDelete)
            });
        }
        return columns;
    }
    handleTableChange: TableProps<T>['onChange'] = async (
        pagination: any,
        _filters: any, 
        sorter: any,
        _extra: any,
    ) => {
        const newSortField = (sorter as SorterResult<T>).field as SorterResult<any>['field'];
        const newSortOrder = (sorter as SorterResult<T>).order;

        this.setState((prevState) => {
            const isPageSizeChanged = pagination.pageSize !== prevState.pagination?.pageSize;
            const isPageChanged = prevState.pagination?.current !== pagination.current;
            return {
                ...prevState,
                pagination: { ...prevState.pagination, ...pagination },
                sortOrder: newSortOrder,
                sortField: newSortField,
                items: isPageSizeChanged ? [] : prevState.items,
                loading: (isPageSizeChanged || isPageChanged) ? true : prevState.loading,
            }
        });
    }

    fetchData = async () => {
        if (this.state.dialogMode !== DialogModes.NONE) {
            return;
        }
        this.setState({ loading: true });
        let sortBy = this.state.sortField?.toString().trim() || 'created_at';
        const params: Map<string, string> = new Map([
            ["page", this.state.pagination?.current?.toString() || "1"],
            ["limit", this.state.pagination?.pageSize?.toString() || "10"],
            ["sort_by", sortBy],
            ["asc", this.state.sortOrder === 'ascend' ? 'true' : 'false'],
        ]);
        this.state.filters.forEach((value, key) => {
            if (value && value.length > 0) {
                params.set(key, value);
            }
        });
        const responseJson = await loadData<T[]>(this.props.endpoint, params);
        if (responseJson.status === 200 && responseJson.data) {
            this.setState(prevState => ({
                ...prevState,
                items: responseJson.data!,
                loading: false,
                pagination: {
                    ...prevState.pagination,
                    current: responseJson.pagination?.page || 1,
                    pageSize: responseJson.pagination?.limit || 10,
                    total: responseJson.pagination?.records || 0,
                }
            }));
        } else {
             this.setState(prevState => ({ ...prevState, loading: false }));
        }
    }

    componentDidMount = async () => {
        await this.fetchData();
    }
    componentDidUpdate = async (_prevProps: Props<T>, prevState: State<T>) => { 
        const filtersHaveChanged = !mapsEqual(prevState.filters, this.state.filters);
        const dialogHasClosed = prevState.dialogMode !== DialogModes.NONE && this.state.dialogMode === DialogModes.NONE;

        if (dialogHasClosed) {
             await this.fetchData(); 
             return;
        } else if (this.state.dialogMode !== DialogModes.NONE) {
             return;
        }
        
        if (prevState.pagination?.current !== this.state.pagination?.current ||
            this.state.pagination?.pageSize !== prevState.pagination?.pageSize ||
            this.state.sortField !== prevState.sortField ||
            this.state.sortOrder !== prevState.sortOrder ||
            filtersHaveChanged
        ) {
            if (filtersHaveChanged) {
                 this.columns = this.getColumns(); 
            }
            await this.fetchData();
        }
    }

    render = () => {
        const titleText = this.props.t(this.props.title);
        const { hasActions, renderHeaderAction } = this.props;
        
        const dialogUI = hasActions && this.state.dialogMode !== DialogModes.NONE ? (
            <CustomDialog<T>
                endpoint={this.props.endpoint}
                fields={this.props.fields}
                dialogMessages={this.props.dialogMessages}
                data={this.state.selectedItem as DialogProps<T>['data']}
                dialogMode={this.state.dialogMode}
                onClose={this.handleCloseDialog}
            />
        ) : null;
        const headerUI = hasActions && renderHeaderAction ? (
            renderHeaderAction(this.handleCreate)
        ) : (
            <Text style={{ fontSize: '24px' }} strong>{titleText}</Text>
        );

        return (
            <>
                {dialogUI}
                <Flex vertical justify="center" align="center" gap="middle" >
                    <Flex justify="center" align="center" gap="middle" >
                        {headerUI}
                    </Flex>
                    <Table<T>
                        columns={this.columns}
                        rowKey={record => record.id.toString()}
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
}
