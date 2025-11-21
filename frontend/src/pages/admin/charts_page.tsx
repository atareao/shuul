import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography } from 'antd';
import { ResponsivePie } from '@nivo/pie';
const { Text } = Typography;
import { loadData } from "@/common/utils";

interface Props {
    navigate: any
    t: any
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
            top_countries: top_countries.status === 200 ? top_countries.data as Array< [string, number, number]> : [],
        });

    }

    render = () => {
        const { top_countries } = this.state;
        const data = top_countries.map(item => ({ id: item[0], label: item[1], value: item[2] }));
        console.log("Pie chart data:", data);
        return (
            <Flex vertical justify="center" align="center" >
                <Text>Charts</Text>
                <Flex style={{ height: 400, width: 600 }}>
                <ResponsivePie /* or Pie for fixed dimensions */
                    data={data}
                    margin={{ top: 40, right: 80, bottom: 80, left: 80 }}
                    innerRadius={0.5}
                    padAngle={0.6}
                    cornerRadius={2}
                    activeOuterRadiusOffset={8}
                    arcLinkLabelsSkipAngle={10}
                    arcLinkLabelsTextColor="#333333"
                    arcLinkLabelsThickness={2}
                    arcLinkLabelsColor={{ from: 'color' }}
                    arcLabelsSkipAngle={10}
                    arcLabelsTextColor={{ from: 'color', modifiers: [['darker', 2]] }}
                    legends={[
                        {
                            anchor: 'bottom',
                            direction: 'row',
                            translateY: 56,
                            itemWidth: 100,
                            itemHeight: 18,
                            symbolShape: 'circle'
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
    return <InnerPage navigate={navigate} t={t} />;
}

