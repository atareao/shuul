import React from "react";
import { Table, Input, Flex, Typography } from 'antd';
import type { GetProp, TableProps, TableColumnsType } from 'antd';
import type { SorterResult } from 'antd/es/table/interface';
import { CheckOutlined, CloseOutlined } from '@ant-design/icons';
const { Text } = Typography;
type TablePaginationConfig = Exclude<GetProp<TableProps, 'pagination'>, boolean>;

import { loadData, mapsEqual, debounce } from '@/common/utils';
import type { DialogMode, FieldDefinition } from '@/common/types';
import { DialogModes } from '@/common/types'
import CustomDialog from '@/components/dialogs/custom_dialog';
import type { DialogMessages, DialogProps } from '@/components/dialogs/custom_dialog';

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

const getNestedValue = (obj: any, path: string): any => {
    const pathParts = path.split('.');
    let current = obj;

    for (const part of pathParts) {
        if (current && typeof current === 'object' && part in current) {
            current = current[part];
        } else {
            return undefined;
        }
    }
    return current;
};


export default class CustomTable<T extends { id: number | string }> extends React.Component<Props<T>, State<T>> {
    columns: TableColumnsType<T>;
    private debouncedSetFilter: (key: string, value: string) => void;

    constructor(props: Props<T>) {
        super(props);

        const initialFilters = new Map<string, string>();
        props.fields.forEach(field => initialFilters.set(field.key.toString(), ""));

        this.state = {
            items: [],
            loading: false,
            pagination: { current: 1, pageSize: 9, total: 0 },
            filters: initialFilters,
            dialogMode: DialogModes.NONE,
            selectedItem: undefined,
        };

        this.columns = this.getColumns();
        const updateFilterState = (key: string, value: string) => {
            const cleanValue = value.trim().replaceAll("*", "%");
            this.setState((prevState) => {
                // Si el valor no ha cambiado, no hacemos nada
                if (prevState.filters.get(key) === cleanValue) {
                    return prevState;
                }
                const newFilters = new Map(prevState.filters);
                newFilters.set(key, cleanValue);
                return {
                    ...prevState,
                    filters: newFilters,
                    pagination: { ...prevState.pagination, current: 1 },
                    items: [],
                    loading: true,
                };
            });
        };

        // 2. Debounce de la función de actualización
        // Usaremos un delay de 500ms
        this.debouncedSetFilter = debounce(updateFilterState, 500);
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
            const fieldKey = field.key.toString();
            const filterValue = this.state.filters.get(fieldKey) || "";

            const handleFilterChange = (e: React.KeyboardEvent<HTMLInputElement>) => {
                this.debouncedSetFilter(field.key.toString(), e.currentTarget.value);
            };
            const defaultRender = (content: any) => {
                if (field.type === 'boolean') {
                    return content ? <CheckOutlined style={{ color: 'green' }} /> : <CloseOutlined style={{ color: 'red' }} />;
                }
                return <Text>{content}</Text>
            };
            const isNested = fieldKey.includes('.');
            let finalRender = field.render || defaultRender;

            if (isNested && !field.render) {
                // Si es una clave anidada Y el usuario NO proveyó un render:
                finalRender = (_content: any, record: T) => {
                    // 1. Obtenemos el valor anidado de forma segura
                    const value = getNestedValue(record, fieldKey);
                    
                    // 2. Aplicamos la lógica de renderizado por defecto (boolean/texto)
                    if (field.type === 'boolean') {
                        return value ? <CheckOutlined style={{ color: 'green' }} /> : <CloseOutlined style={{ color: 'red' }} />;
                    }
                    // Retornamos el valor, o una cadena vacía si es null/undefined
                    return <Text>{value !== undefined && value !== null ? value : ''}</Text>;
                };
            }
            return {
                title: (
                    <Flex vertical justify="flex-end" align="left" gap="middle" >
                        <Text strong>{this.props.t(field.label)}</Text>
                        {(field.type === 'string' && field.filterKey) &&
                            <Input
                                placeholder={this.props.t('Filter by') + ` ${field.label}...`}
                                defaultValue={filterValue.replaceAll("%", "*")}
                                onKeyUp={handleFilterChange}
                                onClick={e => e.stopPropagation()}
                            />
                        }
                    </Flex>
                ),
                dataIndex: field.key.toString(),
                key: field.key.toString(),
                sorter: field.type !== 'boolean',
                ellipsis: true,
                width: field.width || 100,
                render: finalRender,
                fixed: field.fixed || undefined,
            };
        });
        if (this.props.hasActions && this.props.renderActionColumn) {
            columns.push({
                title: <Text>{this.props.t('Acciones')}</Text>,
                key: "operation-actions",
                align: 'center',
                width: 10,
                fixed: 'right',
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
        const rawSortField = (sorter as SorterResult<T>).field as SorterResult<any>['field'];
        const fieldDefinition = this.props.fields.find(f => f.key === rawSortField);
        const newSortField = fieldDefinition?.sortKey || rawSortField;
        const newSortOrder = (sorter as SorterResult<T>).order;

        this.setState((prevState) => {
            const isPageSizeChanged = pagination.pageSize !== prevState.pagination?.pageSize;
            const isPageChanged = prevState.pagination?.current !== pagination.current;

            // 2. Actualizar el estado con los NUEVOS valores de ordenación y paginación
            return {
                ...prevState,
                pagination: { ...prevState.pagination, ...pagination },
                sortOrder: newSortOrder, // 'descend' ahora se guarda aquí
                sortField: newSortField,
                items: (isPageSizeChanged || isPageChanged) ? [] : [...prevState.items],
                // El loading se puede gestionar aquí, o dejarlo en componentDidUpdate
                loading: (isPageSizeChanged || isPageChanged || prevState.sortOrder !== newSortOrder || prevState.sortField !== newSortField) ? true : prevState.loading,
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
        ]);
        const sortOrder = this.state.sortOrder;
        if (sortOrder === 'ascend') {
            params.set("asc", 'true');
        } else if (sortOrder === 'descend') {
            params.set("asc", 'false');
        }
        this.state.filters.forEach((value, fieldKey) => { // fieldKey es 'value.name_es'
            if (value && value.length > 0) {
                const fieldDefinition = this.props.fields.find(f => f.key === fieldKey);
                const apiFilterKey = fieldDefinition?.filterKey || fieldKey; // Usa 'filterKey' o la clave anidada
                params.set(apiFilterKey, value); 
            }
        });
        const responseJson = await loadData<T[]>(this.props.endpoint, params);
        console.log(responseJson.data);
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
                        style={{ width: '100%' }}
                        columns={this.columns}
                        rowKey={record => record.id.toString()}
                        dataSource={this.state.items}
                        sortDirections={['ascend', 'descend']}
                        pagination={this.state.pagination}
                        loading={this.state.loading}
                        onChange={this.handleTableChange}
                        scroll={{ x: 'max-content' }}
                    />
                </Flex>
            </>
        );
    }
}
