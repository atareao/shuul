import react, { lazy, Suspense } from "react";
import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";
import { Flex, Typography, Spin, InputNumber, Select, Card } from "antd";
const { Title } = Typography;
import { loadData } from "@/common/utils";
import ModeContext from "@/components/mode_context";

const Line = lazy(() => import("@/components/charts/antd_line"));
const Pie = lazy(() => import("@/components/charts/antd_pie"));

interface Props {
  navigate: any;
  t: any;
}

interface State {
  loading: boolean;
  error: boolean;
  top_countries: Array<[string, number, number]>;
  top_rules: Array<[string, number, number]>;
  evolution_data: Array<{ id: string; data: Array<{ x: string; y: number }> }>;
  unit: string;
  last: number;
}

const COLORS = [
  "#fa541c",
  "#1890ff",
  "#52c41a",
  "#faad14",
  "#722ed1",
  "#13c2c2",
  "#eb2f96",
  "#fa8c16",
  "#a0d911",
  "#2f54eb",
];

export class InnerPage extends react.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = {
      loading: true,
      error: false,
      top_countries: [],
      top_rules: [],
      evolution_data: [],
      unit: "day",
      last: 7,
    };
  }

  refreshData = async (all?: boolean, newUnit?: string, newLast?: number) => {
    const unit = newUnit || this.state.unit;
    const last = newLast || this.state.last;

    try {
      if (all) {
        this.setState({ loading: true, error: false });
        const [top_countries_res, top_rules_res, evolution_data_res] =
          await Promise.all([
            loadData("stats/top_countries"),
            loadData("stats/top_rules"),
            loadData(
              "stats/evolution",
              new Map([
                ["unit", unit],
                ["last", last.toString()],
              ]),
            ),
          ]);

        this.setState({
          loading: false,
          top_countries:
            top_countries_res.status === 200
              ? (top_countries_res.data as Array<[string, number, number]>)
              : [],
          top_rules:
            top_rules_res.status === 200
              ? (top_rules_res.data as Array<[string, number, number]>)
              : [],
          evolution_data:
            evolution_data_res.status === 200
              ? (evolution_data_res.data as Array<{
                  id: string;
                  data: Array<{ x: string; y: number }>;
                }>)
              : [],
        });
      } else {
        this.setState({ loading: true, error: false });
        const evolution_data = await loadData(
          "stats/evolution",
          new Map([
            ["unit", unit],
            ["last", last.toString()],
          ]),
        );
        this.setState({
          loading: false,
          evolution_data:
            evolution_data.status === 200
              ? (evolution_data.data as Array<{
                  id: string;
                  data: Array<{ x: string; y: number }>;
                }>)
              : [],
        });
      }
    } catch (err) {
      console.error("Failed to load charts data:", err);
      this.setState({ loading: false, error: true });
    }
  };

  componentDidMount = async () => {
    try {
      await this.refreshData(true);
    } catch (err) {
      console.error("Failed to load charts data on mount:", err);
      this.setState({ loading: false, error: true });
    }
  };

  render = () => {
    const { top_countries, top_rules, evolution_data, loading, error } =
      this.state;

    const topCountriesData = top_countries.map(([name, count]) => ({
      name,
      value: count,
    }));
    const topRulesData = top_rules.map(([name, count]) => ({
      name,
      value: count,
    }));
    const evolutionFlatData = evolution_data.flatMap((series) =>
      series.data.map((point) => ({
        category: series.id,
        time: point.x,
        requests: point.y,
      })),
    );

    if (loading) {
      return (
        <Flex
          vertical
          justify="center"
          align="center"
          style={{ minHeight: 400 }}
        >
          <Spin size="large" />
        </Flex>
      );
    }

    if (error) {
      return (
        <Flex
          vertical
          justify="center"
          align="center"
          style={{ minHeight: 400 }}
        >
          <Card style={{ width: 400, textAlign: "center" }}>
            <Typography.Text type="danger" style={{ fontSize: 16 }}>
              Failed to load charts data
            </Typography.Text>
          </Card>
        </Flex>
      );
    }

    return (
      <Flex vertical gap="large" style={{ padding: 24 }}>
        <Title level={2}>Charts</Title>

        {/* Evolution Line Chart */}
        <Card title="Request Evolution" size="small">
          <Flex justify="flex-end" gap="middle" style={{ marginBottom: 16 }}>
            <InputNumber
              min={1}
              value={this.state.last}
              onChange={(value) => {
                const newLast = value || 7;
                this.setState({ last: newLast }, () =>
                  this.refreshData(false, undefined, newLast),
                );
              }}
            />
            <Select
              value={this.state.unit}
              onChange={(value) => {
                const newUnit = value || "day";
                this.setState({ unit: value }, () =>
                  this.refreshData(false, newUnit, undefined),
                );
              }}
              options={[
                { value: "day", label: "day" },
                { value: "hour", label: "hour" },
              ]}
              style={{ width: 100 }}
            />
          </Flex>
          <div style={{ height: 400 }}>
            {evolutionFlatData.length > 0 ? (
              <Suspense fallback={<Spin />}>
                <Line
                  data={evolutionFlatData}
                  xField="time"
                  yField="requests"
                  seriesField="category"
                  smooth
                  point={{ shapeField: "circle", sizeField: 3 }}
                  legend={{
                    color: {
                      position: "top",
                      layout: { justifyContent: "center" },
                    },
                  }}
                  axis={{ x: { title: "Time" }, y: { title: "Requests" } }}
                  slider={{}}
                />
              </Suspense>
            ) : (
              <Flex justify="center" align="center" style={{ height: "100%" }}>
                <Typography.Text type="secondary">
                  No evolution data available
                </Typography.Text>
              </Flex>
            )}
          </div>
        </Card>

        {/* Pie charts row */}
        <Flex gap="large" wrap>
          <Card
            title="Top Countries"
            size="small"
            style={{ flex: 1, minWidth: 350 }}
          >
            <div style={{ height: 350 }}>
              {topCountriesData.length > 0 ? (
                <Suspense fallback={<Spin />}>
                  <Pie
                    data={topCountriesData}
                    angleField="value"
                    colorField="name"
                    color={COLORS}
                    innerRadius={0.5}
                    label={{ text: "name", style: { fontWeight: "bold" } }}
                    legend={{ color: { position: "right", rowPadding: 4 } }}
                  />
                </Suspense>
              ) : (
                <Flex
                  justify="center"
                  align="center"
                  style={{ height: "100%" }}
                >
                  <Typography.Text type="secondary">
                    No country data available
                  </Typography.Text>
                </Flex>
              )}
            </div>
          </Card>
          <Card
            title="Top Rules"
            size="small"
            style={{ flex: 1, minWidth: 350 }}
          >
            <div style={{ height: 350 }}>
              {topRulesData.length > 0 ? (
                <Suspense fallback={<Spin />}>
                  <Pie
                    data={topRulesData}
                    angleField="value"
                    colorField="name"
                    color={COLORS}
                    innerRadius={0.5}
                    label={{ text: "name", style: { fontWeight: "bold" } }}
                    legend={{ color: { position: "right", rowPadding: 4 } }}
                  />
                </Suspense>
              ) : (
                <Flex
                  justify="center"
                  align="center"
                  style={{ height: "100%" }}
                >
                  <Typography.Text type="secondary">
                    No rule data available
                  </Typography.Text>
                </Flex>
              )}
            </div>
          </Card>
        </Flex>
      </Flex>
    );
  };
}

export default function ChartsPage() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  return (
    <ModeContext.Consumer>
      {() => {
        return <InnerPage navigate={navigate} t={t} />;
      }}
    </ModeContext.Consumer>
  );
}
