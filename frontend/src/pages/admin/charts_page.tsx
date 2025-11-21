import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography } from 'antd';
import { ResponsivePie } from '@nivo/pie';
const { Text } = Typography;
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
}

export class InnerPage extends react.Component<Props, State> {

    constructor(props: Props) {
        super(props);
        this.state = {
            loading: true,
            top_countries: [],
        }
    }

    componentDidMount = async () => {
        const top_countries = await loadData("requests/top_countries");
        this.setState({
            loading: false,
            top_countries: top_countries.status === 200 ? top_countries.data as Array<[string, number, number]> : [],
        });

    }

    render = () => {
        const { isDarkMode } = this.props;
        const { top_countries, loading } = this.state;
        const data = top_countries.map(item => ({ id: item[0], label: item[1], value: Math.round(item[2] * 100) / 100 }));
        console.log("Pie chart data:", data);
        if (loading) {
            return (
                <Flex vertical justify="center" align="center" >
                    <Text>Loading</Text>
                </Flex>
            );
        }
        let fontColor = "#222222";
        if (isDarkMode) {
            fontColor = "#eeeeee";
        }
        return (
            <Flex vertical justify="center" align="center" >
                <Text>Charts</Text>
                <Flex style={{ height: 400, width: 600 }}>
                    <ResponsivePie /* or Pie for fixed dimensions */
                        theme={{
                            text: {
                                fontSize: 16,
                                fill: fontColor,
                            }
                        }}
                        data={data}
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
                <Text>Fin</Text>
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

