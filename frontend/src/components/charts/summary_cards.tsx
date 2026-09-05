import react from "react";
import { Flex, Card, Statistic } from "antd";

interface Props {
  total: number;
  allowed: number;
  blocked: number;
}

export default class SummaryCards extends react.Component<Props> {
  render = () => {
    const { total, allowed, blocked } = this.props;
    const blockRate = total > 0 ? ((blocked / total) * 100).toFixed(1) : "0.0";
    return (
      <Flex gap="middle" wrap>
        <Card size="small" style={{ flex: 1, minWidth: 150 }}>
          <Statistic title="Total" value={total} />
        </Card>
        <Card size="small" style={{ flex: 1, minWidth: 150 }}>
          <Statistic
            title="Allowed"
            value={allowed}
            valueStyle={{ color: "#52c41a" }}
          />
        </Card>
        <Card size="small" style={{ flex: 1, minWidth: 150 }}>
          <Statistic
            title="Blocked"
            value={blocked}
            valueStyle={{ color: "#fa541c" }}
          />
        </Card>
        <Card size="small" style={{ flex: 1, minWidth: 150 }}>
          <Statistic
            title="Block Rate"
            value={blockRate}
            suffix="%"
            valueStyle={{
              color: blocked > allowed ? "#fa541c" : "#52c41a",
            }}
          />
        </Card>
      </Flex>
    );
  };
}
