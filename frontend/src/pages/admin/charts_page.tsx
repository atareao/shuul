import react, { lazy, Suspense } from "react";
import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";
import { Flex, Typography, Spin, InputNumber, Select, Card, Tabs } from "antd";
import { loadData } from "@/common/utils";
import ModeContext from "@/components/mode_context";
import { ConfigProvider } from "@ant-design/charts";
import SummaryCards from "@/components/charts/summary_cards";
import EvolutionStacked from "@/components/charts/evolution_stacked";
import BlockRateChart from "@/components/charts/block_rate_chart";
import TopMethods from "@/components/charts/top_methods";
import TopPaths from "@/components/charts/top_paths";
import EvolutionByMethod from "@/components/charts/evolution_by_method";

const Pie = lazy(() => import("@/components/charts/antd_pie"));

interface Props {
  navigate: any;
  t: any;
  isDarkMode: boolean;
}

interface State {
  loading: boolean;
  error: boolean;
  top_countries: Array<[string, number, number]>;
  top_rules: Array<[string, number, number]>;
  top_methods: Array<[string, number, number]>;
  top_paths: Array<[string, number, number]>;
  top_fqdns: Array<[string, number, number]>;
  evolution_data: Array<{ id: string; data: Array<{ x: string; y: number }> }>;
  evolution_by_method: Array<{
    id: string;
    data: Array<{ x: string; y: number }>;
  }>;
  unit: string;
  last: number;
  total: number;
  allowed: number;
  blocked: number;
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
      top_methods: [],
      top_paths: [],
      top_fqdns: [],
      evolution_data: [],
      evolution_by_method: [],
      unit: "hour",
      last: 24,
      total: 0,
      allowed: 0,
      blocked: 0,
    };
  }

  refreshData = async (all?: boolean, newUnit?: string, newLast?: number) => {
    const unit = newUnit || this.state.unit;
    const last = newLast || this.state.last;

    try {
      if (all) {
        this.setState({ loading: true, error: false });
        const [
          top_countries_res,
          top_rules_res,
          evolution_data_res,
          top_methods_res,
          top_paths_res,
          top_fqdns_res,
          evolution_by_method_res,
          stats_total_res,
          stats_blocked_res,
        ] = await Promise.all([
          loadData("stats/top_countries"),
          loadData("stats/top_rules"),
          loadData(
            "stats/evolution",
            new Map([
              ["unit", unit],
              ["last", last.toString()],
            ]),
          ),
          loadData("stats/top_methods"),
          loadData("stats/top_paths"),
          loadData("stats/top_fqdns"),
          loadData(
            "stats/evolution_by_method",
            new Map([
              ["unit", unit],
              ["last", last.toString()],
            ]),
          ),
          loadData("stats/info", new Map([["option", "total"]])),
          loadData("stats/info", new Map([["option", "filtered"]])),
        ]);

        const total =
          stats_total_res.status === 200 ? (stats_total_res.data as number) : 0;
        const blocked =
          stats_blocked_res.status === 200
            ? (stats_blocked_res.data as number)
            : 0;
        const allowed = total - blocked;

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
          top_methods:
            top_methods_res.status === 200
              ? (top_methods_res.data as Array<[string, number, number]>)
              : [],
          top_paths:
            top_paths_res.status === 200
              ? (top_paths_res.data as Array<[string, number, number]>)
              : [],
          top_fqdns:
            top_fqdns_res.status === 200
              ? (top_fqdns_res.data as Array<[string, number, number]>)
              : [],
          evolution_data:
            evolution_data_res.status === 200
              ? (evolution_data_res.data as Array<{
                  id: string;
                  data: Array<{ x: string; y: number }>;
                }>)
              : [],
          evolution_by_method:
            evolution_by_method_res.status === 200
              ? (evolution_by_method_res.data as Array<{
                  id: string;
                  data: Array<{ x: string; y: number }>;
                }>)
              : [],
          total,
          allowed,
          blocked,
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
    const {
      top_countries,
      top_rules,
      top_methods,
      top_paths,
      top_fqdns,
      evolution_data,
      evolution_by_method,
      loading,
      error,
    } = this.state;

    const topCountriesData = top_countries.map(([name, count]) => ({
      name,
      value: count,
    }));
    const topRulesData = top_rules.map(([name, count]) => ({
      name,
      value: count,
    }));
    const topMethodsData = top_methods.map(([name, count]) => ({
      name,
      value: count,
    }));
    const topPathsData = top_paths.map(([name, count]) => ({
      name,
      value: count,
    }));
    const topFqdnsData = top_fqdns.map(([name, count]) => ({
      name,
      value: count,
    }));

    // Format ISO timestamps as short labels según la unidad
    const fmtTime = (iso: string) => {
      const d = new Date(iso);
      if (this.state.unit === "hour" || this.state.unit === "minute") {
        return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
      }
      return `${String(d.getDate()).padStart(2, "0")}/${String(d.getMonth() + 1).padStart(2, "0")}`;
    };

    const evolutionFlatData = evolution_data.flatMap((series) =>
      series.data.map((point) => ({
        category: series.id,
        time: fmtTime(point.x),
        requests: point.y,
      })),
    );

    const blockRateData = evolution_data
      .filter((s) => s.id === "blocked")
      .flatMap((series) =>
        series.data.map((point) => {
          const allowedSeries = evolution_data.find((s) => s.id === "allowed");
          const allowedPoint = allowedSeries?.data.find((p) => p.x === point.x);
          const total = point.y + (allowedPoint?.y ?? 0);
          return {
            time: fmtTime(point.x),
            rate: total > 0 ? (point.y / total) * 100 : 0,
          };
        }),
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
          <Card
            title="Top FQDNs"
            size="small"
            style={{ flex: 1, minWidth: 350 }}
          >
            <div style={{ height: 300 }}>
              {topFqdnsData.length > 0 ? (
                <Suspense fallback={<Spin />}>
                  <Pie
                    data={topFqdnsData}
                    angleField="value"
                    colorField="name"
                    color={COLORS}
                    innerRadius={0.5}
                    label={{
                      text: "name",
                      style: { fontWeight: "bold" },
                    }}
                    legend={{
                      color: { position: "right", rowPadding: 4 },
                    }}
                  />
                </Suspense>
              ) : (
                <Flex
                  justify="center"
                  align="center"
                  style={{ height: "100%" }}
                >
                  <Typography.Text type="secondary">
                    No FQDN data available
                  </Typography.Text>
                </Flex>
              )}
            </div>
          </Card>
        </Flex>
      );
    }

    const { isDarkMode } = this.props;

    return (
      <Flex vertical gap="large" style={{ padding: 24 }}>
        <ConfigProvider
          common={{ theme: { type: isDarkMode ? "dark" : "classic" } }}
        >
          <Tabs
            defaultActiveKey="evolution"
            items={[
              {
                key: "evolution",
                label: "Evolution",
                children: (
                  <Flex vertical gap="large">
                    <SummaryCards
                      total={this.state.total}
                      allowed={this.state.allowed}
                      blocked={this.state.blocked}
                    />
                    <Card
                      title="Request Evolution (stacked)"
                      size="small"
                      extra={
                        <Flex gap="middle">
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
                      }
                    >
                      <div style={{ height: 350 }}>
                        {evolutionFlatData.length > 0 ? (
                          <EvolutionStacked
                            data={evolutionFlatData}
                            isDarkMode={isDarkMode}
                          />
                        ) : (
                          <Flex
                            justify="center"
                            align="center"
                            style={{ height: "100%" }}
                          >
                            <Typography.Text type="secondary">
                              No evolution data available
                            </Typography.Text>
                          </Flex>
                        )}
                      </div>
                    </Card>
                    <Card title="Block Rate (%)" size="small">
                      <div style={{ height: 200 }}>
                        {blockRateData.length > 0 ? (
                          <BlockRateChart
                            data={blockRateData}
                            isDarkMode={isDarkMode}
                          />
                        ) : (
                          <Flex
                            justify="center"
                            align="center"
                            style={{ height: "100%" }}
                          >
                            <Typography.Text type="secondary">
                              No block rate data available
                            </Typography.Text>
                          </Flex>
                        )}
                      </div>
                    </Card>
                    <Card title="Evolution by Method" size="small">
                      <div style={{ height: 300 }}>
                        {evolution_by_method.length > 0 ? (
                          <EvolutionByMethod
                            data={evolution_by_method.flatMap((series) =>
                              series.data.map((point) => ({
                                category: series.id,
                                time: fmtTime(point.x),
                                requests: point.y,
                              })),
                            )}
                            isDarkMode={isDarkMode}
                          />
                        ) : (
                          <Flex
                            justify="center"
                            align="center"
                            style={{ height: "100%" }}
                          >
                            <Typography.Text type="secondary">
                              No method evolution data available
                            </Typography.Text>
                          </Flex>
                        )}
                      </div>
                    </Card>
                  </Flex>
                ),
              },
              {
                key: "rankings",
                label: "Rankings",
                children: (
                  <Flex gap="large" wrap>
                    <Card
                      title="Top Countries"
                      size="small"
                      style={{ width: "calc(50% - 12px)" }}
                    >
                      <div style={{ height: 300 }}>
                        {topCountriesData.length > 0 ? (
                          <Suspense fallback={<Spin />}>
                            <Pie
                              data={topCountriesData}
                              angleField="value"
                              colorField="name"
                              color={COLORS}
                              innerRadius={0.5}
                              label={{
                                text: "name",
                                style: { fontWeight: "bold" },
                              }}
                              legend={{
                                color: { position: "right", rowPadding: 4 },
                              }}
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
                      style={{ width: "calc(50% - 12px)" }}
                    >
                      <div style={{ height: 300 }}>
                        {topRulesData.length > 0 ? (
                          <Suspense fallback={<Spin />}>
                            <Pie
                              data={topRulesData}
                              angleField="value"
                              colorField="name"
                              color={COLORS}
                              innerRadius={0.5}
                              label={{
                                text: "name",
                                style: { fontWeight: "bold" },
                              }}
                              legend={{
                                color: { position: "right", rowPadding: 4 },
                              }}
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
                    <Card
                      title="Top Methods"
                      size="small"
                      style={{ width: "calc(50% - 12px)" }}
                    >
                      <div style={{ height: 300 }}>
                        {topMethodsData.length > 0 ? (
                          <TopMethods
                            data={topMethodsData}
                            isDarkMode={isDarkMode}
                          />
                        ) : (
                          <Flex
                            justify="center"
                            align="center"
                            style={{ height: "100%" }}
                          >
                            <Typography.Text type="secondary">
                              No method data available
                            </Typography.Text>
                          </Flex>
                        )}
                      </div>
                    </Card>
                    <Card
                      title="Top Paths"
                      size="small"
                      style={{ width: "calc(50% - 12px)" }}
                    >
                      <div style={{ height: 300 }}>
                        {topPathsData.length > 0 ? (
                          <TopPaths
                            data={topPathsData}
                            isDarkMode={isDarkMode}
                          />
                        ) : (
                          <Flex
                            justify="center"
                            align="center"
                            style={{ height: "100%" }}
                          >
                            <Typography.Text type="secondary">
                              No path data available
                            </Typography.Text>
                          </Flex>
                        )}
                      </div>
                    </Card>
                    <Card
                      title="Top FQDNs"
                      size="small"
                      style={{ width: "calc(50% - 12px)" }}
                    >
                      <div style={{ height: 300 }}>
                        {topFqdnsData.length > 0 ? (
                          <Suspense fallback={<Spin />}>
                            <Pie
                              data={topFqdnsData}
                              angleField="value"
                              colorField="name"
                              color={COLORS}
                              innerRadius={0.5}
                              label={{
                                text: "name",
                                style: { fontWeight: "bold" },
                              }}
                              legend={{
                                color: { position: "right", rowPadding: 4 },
                              }}
                            />
                          </Suspense>
                        ) : (
                          <Flex
                            justify="center"
                            align="center"
                            style={{ height: "100%" }}
                          >
                            <Typography.Text type="secondary">
                              No FQDN data available
                            </Typography.Text>
                          </Flex>
                        )}
                      </div>
                    </Card>
                  </Flex>
                ),
              },
            ]}
          />
        </ConfigProvider>
      </Flex>
    );
  };
}

export default function ChartsPage() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  return (
    <ModeContext.Consumer>
      {({ isDarkMode }) => {
        return <InnerPage navigate={navigate} t={t} isDarkMode={isDarkMode} />;
      }}
    </ModeContext.Consumer>
  );
}
