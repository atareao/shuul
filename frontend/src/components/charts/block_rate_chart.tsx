import react, { lazy, Suspense } from "react";
import { Flex, Spin } from "antd";

const Line = lazy(() => import("@/components/charts/antd_line"));

interface Props {
  data: Array<{ time: string; rate: number }>;
  isDarkMode: boolean;
}

export default class BlockRateChart extends react.Component<Props> {
  render = () => {
    const { data } = this.props;
    if (data.length === 0) {
      return (
        <Flex justify="center" align="center" style={{ height: 200 }}>
          <span>No block rate data available</span>
        </Flex>
      );
    }
    return (
      <Suspense fallback={<Spin />}>
        <Line
          data={data}
          xField="time"
          yField="rate"
          smooth
          point={{ shapeField: "circle", sizeField: 3 }}
          axis={{
            x: { title: "Time" },
            y: { title: "Block Rate (%)", max: 100 },
          }}
          style={{ lineWidth: 2, stroke: "#fa541c" }}
          legend={false}
        />
      </Suspense>
    );
  };
}
