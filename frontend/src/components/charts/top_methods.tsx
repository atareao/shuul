import react, { lazy, Suspense } from "react";
import { Flex, Spin } from "antd";

const Pie = lazy(() => import("@/components/charts/antd_pie"));

const COLORS = [
  "#fa541c",
  "#1890ff",
  "#52c41a",
  "#faad14",
  "#722ed1",
  "#13c2c2",
];

interface Props {
  data: Array<{ name: string; value: number }>;
  isDarkMode: boolean;
}

export default class TopMethods extends react.Component<Props> {
  render = () => {
    const { data } = this.props;
    if (data.length === 0) {
      return (
        <Flex justify="center" align="center" style={{ height: 250 }}>
          <span>No method data available</span>
        </Flex>
      );
    }
    return (
      <Suspense fallback={<Spin />}>
        <Pie
          data={data}
          angleField="value"
          colorField="name"
          color={COLORS}
          innerRadius={0.5}
          label={{ text: "name", style: { fontWeight: "bold" } }}
          legend={{ color: { position: "right", rowPadding: 4 } }}
        />
      </Suspense>
    );
  };
}
