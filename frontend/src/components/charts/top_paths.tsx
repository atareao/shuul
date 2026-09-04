import react, { lazy, Suspense } from "react";
import { Flex, Spin } from "antd";

const Bar = lazy(() => import("@/components/charts/antd_bar"));

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

interface Props {
  data: Array<{ name: string; value: number }>;
  isDarkMode: boolean;
}

export default class TopPaths extends react.Component<Props> {
  render = () => {
    const { data } = this.props;
    if (data.length === 0) {
      return (
        <Flex justify="center" align="center" style={{ height: 250 }}>
          <span>No path data available</span>
        </Flex>
      );
    }
    return (
      <Suspense fallback={<Spin />}>
        <Bar
          data={data}
          xField="value"
          yField="name"
          seriesField="name"
          color={COLORS}
          legend={false}
          axis={{
            x: { title: "Requests" },
            y: { title: "Path", labelAutoRotate: false },
          }}
          barWidthRatio={0.6}
          style={{ maxWidth: "100%" }}
        />
      </Suspense>
    );
  };
}
