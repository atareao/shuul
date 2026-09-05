import React from "react";
import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";
import { Button, Space, Select, Flex } from "antd";
import { EditFilled, DeleteFilled, PlusOutlined } from "@ant-design/icons";
import type Item from "@/models/rule"; // Alias para Rule

// Importamos CustomTable y los tipos necesarios
import CustomTable from "@/components/custom_table";
import type { FieldDefinition } from "@/common/types";
import type { DialogMessages } from "@/components/dialogs/custom_dialog";
import RuleDialog from "@/components/dialogs/rule_dialog";

// 1. Constantes de configuración (fuera de la clase)
const TITLE = "Rules";
const ENDPOINT = "rules";

// Definición de los campos (tipados para Item, que es Rule)
const FIELDS: FieldDefinition<Item>[] = [
  {
    key: "id",
    label: "Id",
    type: "number",
    value: 0,
    width: 60,
    visible: true,
    fixed: "left",
    sortKey: "id",
  },
  {
    key: "active",
    label: "Active",
    type: "boolean",
    value: true,
    width: 45,
    visible: true,
  },
  {
    key: "allow",
    label: "Allow",
    type: "boolean",
    value: false,
    width: 45,
    visible: true,
    render: (_, record) =>
      record.pipeline === "jail" ? "-" : record.allow ? "✓" : "✗",
  },
  {
    key: "weight",
    label: "Weight",
    type: "number",
    value: 100,
    width: 60,
    visible: true,
    sortKey: "weight",
    render: (_, record) =>
      record.pipeline === "jail" ? "-" : String(record.weight ?? 100),
  },
  {
    key: "name",
    label: "Name",
    type: "string",
    value: "",
    width: 150,
    editable: true,
    visible: true,
    sortKey: "name",
  },
  {
    key: "description",
    label: "Description",
    type: "string",
    value: "",
    width: 200,
    visible: true,
    sortKey: "description",
  },
  {
    key: "mode",
    label: "Mode",
    type: "string",
    value: "",
    width: 70,
    visible: true,
    sortKey: "mode",
    render: (_, record) => (record.pipeline === "jail" ? "-" : record.mode),
  },
  {
    key: "rate_limit_profile_name",
    label: "Profile",
    type: "string",
    value: "",
    width: 140,
    visible: true,
    sortKey: "rate_limit_profile_name",
  },
  {
    key: "pipeline",
    label: "Pipeline",
    type: "tag",
    value: "waf",
    width: 80,
    visible: true,
    sortKey: "pipeline",
    options: [
      { value: "waf", label: "WAF", color: "blue" },
      { value: "jail", label: "Jail", color: "green" },
    ],
  },
];

// Mensajes específicos para el CustomDialog de Rules
const RULE_DIALOG_MESSAGES: DialogMessages = {
  createTitle: "Create Rule",
  readTitle: "View Rule",
  updateTitle: "Update Rule",
  deleteTitle: "Delete Rule",
  confirmDeleteMessage: (id: number | string) =>
    `Are you sure you want to delete rule "${id}"?`,
};

// 2. Definición de Props y Clase
interface Props {
  navigate: any; // Propiedad de useNavigate (aunque no se usa aquí)
  t: (key: string) => string; // Propiedad de useTranslation
}

// La clase ya no necesita State, ya que CustomTable maneja el estado de la tabla.
export class InnerPage extends React.Component<
  Props,
  { pipelineFilter: string }
> {
  constructor(props: Props) {
    super(props);
    this.state = { pipelineFilter: "all" };
  }

  // 3. Método para renderizar el botón "Añadir"
  private renderHeaderAction = (onCreate: () => void) => {
    return (
      <Button
        type="primary"
        onClick={onCreate} // Llama al manejador interno de CustomTable para abrir el diálogo CREATE
        icon={<PlusOutlined />}
      >
        {this.props.t("Add Rule")}
      </Button>
    );
  };

  // 4. Método para renderizar la columna de acciones
  private renderActionColumn = (
    item: Item,
    onEdit: (item: Item) => void,
    onDelete: (item: Item) => void,
  ) => {
    return (
      <Space size="middle">
        <Button onClick={() => onEdit(item)} title={this.props.t("Edit")}>
          <EditFilled />
        </Button>
        <Button
          onClick={() => onDelete(item)}
          title={this.props.t("Delete")}
          danger
        >
          <DeleteFilled />
        </Button>
      </Space>
    );
  };

  // 5. El método render ahora solo devuelve el CustomTable
  render = () => {
    // Construir parámetros para el filtro de pipeline (server-side)
    const params = new Map<string, string>();
    if (this.state.pipelineFilter !== "all") {
      params.set("pipeline", this.state.pipelineFilter);
    }

    return (
      <CustomTable<Item>
        title={TITLE}
        endpoint={ENDPOINT}
        fields={FIELDS}
        params={params}
        dialogMessages={RULE_DIALOG_MESSAGES}
        t={this.props.t}
        hasActions={true}
        defaultSortField="id"
        renderHeaderAction={this.renderHeaderAction}
        renderActionColumn={this.renderActionColumn}
        extraHeaderContent={
          <Flex align="center" gap="small">
            <Select
              value={this.state.pipelineFilter}
              onChange={(value) => this.setState({ pipelineFilter: value })}
              style={{ width: 140 }}
              size="small"
              options={[
                { value: "all", label: "All Pipelines" },
                { value: "waf", label: "WAF" },
                { value: "jail", label: "Jail" },
              ]}
            />
          </Flex>
        }
        // El filtro de pipeline ahora es server-side vía params
        dialogRenderer={(params) => (
          <RuleDialog
            dialogMode={params.dialogMode}
            selectedItem={params.selectedItem}
            handleCloseDialog={params.handleCloseDialog}
            endpoint={params.endpoint}
            dialogMessages={params.dialogMessages}
            t={this.props.t}
          />
        )}
      />
    );
  };
}

// 6. Componente funcional (wrapper) para conectar Hooks
export default function Page() {
  const navigate = useNavigate();
  // useTranslation debe estar en un componente funcional o en un componente de clase con un wrapper
  const { t } = useTranslation();
  return <InnerPage navigate={navigate} t={t} />;
}
