import react, { lazy, Suspense } from "react";
import { Flex, Spin } from "antd";

const Column = lazy(() => import("@/components/charts/antd_column"));

interface Props {
  data: Array<{ category: string; time: string; requests: number }>;
  isDarkMode: boolean;
}

export default class EvolutionStacked extends react.Component<Props> {
  render = () => {
    const { data } = this.props;
    if (data.length === 0) {
      return (
        <Flex justify="center" align="center" style={{ height: 300 }}>
          <span>No evolution data available</span>
        </Flex>
      );
    }
    return (
      <Suspense fallback={<Spin />}>
        <Column
          data={data}
          xField="time"
          yField="requests"
          seriesField="category"
          style={{ maxWidth: "100%" }}
          legend={{
            color: { position: "top", layout: { justifyContent: "center" } },
          }}
          axis={{
            x: { title: "Time", labelAutoRotate: true },
            y: { title: "Requests" },
          }}
          transform={[{ type: "stackY" }]}
        />
      </Suspense>
    );
  };
}
