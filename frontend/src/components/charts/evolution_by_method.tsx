import react, { lazy, Suspense } from "react";
import { Flex, Spin } from "antd";

const Line = lazy(() => import("@/components/charts/antd_line"));

interface Props {
  data: Array<{ category: string; time: string; requests: number }>;
  isDarkMode: boolean;
}

export default class EvolutionByMethod extends react.Component<Props> {
  render = () => {
    const { data } = this.props;
    if (data.length === 0) {
      return (
        <Flex justify="center" align="center" style={{ height: 250 }}>
          <span>No method evolution data available</span>
        </Flex>
      );
    }
    return (
      <Suspense fallback={<Spin />}>
        <Line
          data={data}
          xField="time"
          yField="requests"
          seriesField="category"
          smooth
          point={{ shapeField: "circle", sizeField: 2 }}
          legend={{
            color: { position: "top", layout: { justifyContent: "center" } },
          }}
          axis={{
            x: { title: "Time" },
            y: { title: "Requests" },
          }}
        />
      </Suspense>
    );
  };
}
