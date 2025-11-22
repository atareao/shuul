import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography, Space } from 'antd';
import { ResponsivePie } from '@nivo/pie';
const { Title } = Typography;
import { loadData } from "@/common/utils";
import ModeContext from "@/components/mode_context";

interface Props {
    navigate: any;
    t: any;
    isDarkMode: boolean;
}

interface State {
    loading: boolean;
    top_countries: Array<[string, number, number]>,
    top_rules: Array<[string, number, number]>,
}

export class InnerPage extends react.Component<Props, State> {

    constructor(props: Props) {
        super(props);
        this.state = {
            loading: true,
            top_countries: [],
            top_rules: [],
        }
    }

    componentDidMount = async () => {
        const top_countries = await loadData("requests/top_countries");
        const top_rules = await loadData("requests/top_rules");
        this.setState({
            loading: false,
            top_countries: top_countries.status === 200 ? top_countries.data as Array<[string, number, number]> : [],
            top_rules: top_countries.status === 200 ? top_rules.data as Array<[string, number, number]> : [],
        });

    }

    render = () => {
        const { isDarkMode } = this.props;
        const { top_countries, top_rules, loading } = this.state;
        const top_countries_data = top_countries.map(item => ({ id: item[0], label: item[1], value: Math.round(item[2] * 100) / 100 }));
        const top_rules_data = top_rules.map(item => ({ id: item[0], label: item[1], value: Math.round(item[2] * 100) / 100 }));
        if (loading) {
            return (
                <Flex vertical justify="center" align="center" >
                    <Title>Loading</Title>
                </Flex>
            );
        }
        let fontColor = "#222222";
        let toolTipBgColor = "#fff";
        if (isDarkMode) {
            fontColor = "#eeeeee";
            toolTipBgColor = "#333";
        }
        const theme = {
            text: {
                fontSize: 16,
                fill: fontColor,
            },
            tooltip: {
                container: {
                    background: toolTipBgColor,
                }
            }
        }
        return (
            <Flex vertical justify="center" align="center" >
                <Title level={2}>Charts</Title>
                <Flex justify="center" align="center" gap={50} wrap>
                    <Flex vertical style={{ height: 400, width: 600 }}>
                        <Space direction="vertical" align="center">
                        <Title level={3}>Top countries</Title>
                        </Space>
                        <ResponsivePie /* or Pie for fixed dimensions */
                            theme={theme}
                            data={top_countries_data}
                            margin={{ top: 40, right: 80, bottom: 80, left: 80 }}
                            innerRadius={0.5}
                            padAngle={0.6}
                            cornerRadius={2}
                            activeOuterRadiusOffset={8}
                            arcLinkLabelsSkipAngle={10}
                            arcLinkLabelsThickness={2}
                            arcLinkLabelsColor={{ from: 'color' }}
                            arcLabelsSkipAngle={10}
                            arcLabelsTextColor={{ from: 'color', modifiers: [['darker', 3]] }}
                            legends={[
                                {
                                    anchor: 'bottom',
                                    direction: 'row',
                                    translateY: 60,
                                    itemWidth: 100,
                                    itemHeight: 18,
                                }
                            ]}
                        />
                    </Flex>
                    <Flex vertical style={{ height: 400, width: 600 }}>
                        <Space direction="vertical" align="center">
                        <Title level={3}>Top rules</Title>
                        </Space>
                        <ResponsivePie /* or Pie for fixed dimensions */
                            theme={theme}
                            data={top_rules_data}
                            margin={{ top: 40, right: 80, bottom: 80, left: 80 }}
                            innerRadius={0.5}
                            padAngle={0.6}
                            cornerRadius={2}
                            activeOuterRadiusOffset={8}
                            arcLinkLabelsSkipAngle={10}
                            arcLinkLabelsThickness={2}
                            arcLinkLabelsColor={{ from: 'color' }}
                            arcLabelsSkipAngle={10}
                            arcLabelsTextColor={{ from: 'color', modifiers: [['darker', 3]] }}
                            legends={[
                                {
                                    anchor: 'bottom',
                                    direction: 'row',
                                    translateY: 60,
                                    itemWidth: 100,
                                    itemHeight: 18,
                                }
                            ]}
                        />
                    </Flex>
                </Flex>
            </Flex>

        );
    }
};

export default function ChartsPage() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return (
        <ModeContext.Consumer>
            {({ isDarkMode }) => {
                console.log(`Rendering App with isDarkMode ${isDarkMode}`);

                return <InnerPage
                    navigate={navigate}
                    isDarkMode={isDarkMode}
                    t={t}
                />;
            }}
        </ModeContext.Consumer>
    );
}

