import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography, Space, Spin, InputNumber, Select } from 'antd';
import { ResponsivePie } from '@nivo/pie';
import { ResponsiveLine } from '@nivo/line';
const { Title } = Typography;
import { loadData } from "@/common/utils";
import ModeContext from "@/components/mode_context";

interface TimeSeriesPoint {
    x: string;
    y: number;
}

interface TimeSeries {
    id: string; // country_code
    data: TimeSeriesPoint[];
}

interface Props {
    navigate: any;
    t: any;
    isDarkMode: boolean;
}

interface State {
    loading: boolean;
    top_countries: Array<[string, number, number]>;
    top_rules: Array<[string, number, number]>;
    evolution_data: Array<TimeSeries>;
    unit: string;
    last: number;
}

export class InnerPage extends react.Component<Props, State> {

    constructor(props: Props) {
        super(props);
        this.state = {
            loading: true,
            top_countries: [],
            top_rules: [],
            evolution_data: [],
            unit: 'day',
            last: 7,
        }
    }

    refreshData = async (all?: boolean, newUnit?: string, newLast?: number) => {

        // Usar los valores pasados o, si no se pasan, usar el estado actual.
        const unit = newUnit || this.state.unit;
        const last = newLast || this.state.last;

        if (all) {
            this.setState({ loading: true });
            const top_countries = await loadData("requests/top_countries");
            const top_rules = await loadData("requests/top_rules");

            // 2. USAR unit y last EN LA LLAMADA
            const evolution_data = await loadData(`requests/evolution?unit=${unit}&last=${last}`);

            console.log("Evolution Call:", `requests/evolution?unit=${unit}&last=${last}`); // <<== IMPRIMIR LA LLAMADA COMPLETA
            console.log("Evolution Data:", evolution_data);

            this.setState({
                loading: false,
                top_countries: top_countries.status === 200 ? top_countries.data as Array<[string, number, number]> : [],
                top_rules: top_countries.status === 200 ? top_rules.data as Array<[string, number, number]> : [],
                evolution_data: evolution_data.status === 200 ? evolution_data.data as Array<TimeSeries> : [],
            });
        } else {
            this.setState({ loading: true });

            // 3. USAR unit y last EN LA LLAMADA
            const evolution_data = await loadData(`requests/evolution?unit=${unit}&last=${last}`);

            console.log("Evolution Call:", `requests/evolution?unit=${unit}&last=${last}`); // <<== IMPRIMIR LA LLAMADA COMPLETA
            console.log("Evolution Data:", evolution_data);

            this.setState({
                loading: false,
                evolution_data: evolution_data.status === 200 ? evolution_data.data as Array<TimeSeries> : [],
            });
        }
    }

    componentDidMount = async () => {
        await this.refreshData(true);
    }

    render = () => {
        const { isDarkMode } = this.props;
        const { top_countries, top_rules, evolution_data, loading } = this.state;
        const top_countries_data = top_countries.map(item => ({ id: item[0], label: item[1], value: Math.round(item[2] * 100) / 100 }));
        const top_rules_data = top_rules.map(item => ({ id: item[0], label: item[1], value: Math.round(item[2] * 100) / 100 }));
        if (loading) {
            return (
                <Flex vertical justify="center" align="center" >
                    <Spin size="large" />
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
        // Define la configuración de la escala X y el formato del eje X
        const isHourly = this.state.unit === 'hour';
        const xScaleConfig = {
            type: 'time' as const,
            format: 'iso' as  const,
            //format: '%Y-%m-%dT%H:%M:%S' as const,
            precision: isHourly ? 'hour' as const : 'day' as const, // <-- Precision dinamica
            useUTC: false
        };
        const axisBottomFormat = isHourly ? '%Hh' : '%d';
        const axisBottomLegend = isHourly ? 'Time (Hour)' : 'Date (Day)';
        const legendOffset = isHourly ? 45 : 36;
        const valid_evolution_data = evolution_data
            // Asegurarse de que la serie y su array de datos existen
            .filter(series => series && series.data && Array.isArray(series.data))
            // Para cada serie, filtrar los puntos donde 'x' es null, undefined, o una cadena vacía
            .map(series => ({
                ...series,
                data: series.data.filter(point => point && point.x && point.x.length > 0) // <-- FILTRADO CLAVE
            }));
        return (
            <Flex vertical justify="center" align="center" >
                <Title level={2}>Charts</Title>
                {/* 5. Añadir la sección de la gráfica de evolución */}
                <Flex vertical style={{ height: 400, width: '90%', maxWidth: 1200, marginBottom: 100 }}>
                    <Flex vertical justify="center" align="center">
                        <Title level={3}>Request Evolution</Title>
                        <Flex justify="center" align="center" gap="middle">
                            <InputNumber
                                min={1}
                                defaultValue={7}
                                value={this.state.last}
                                onChange={(value) => {
                                    const newLast = value || 7;
                                    this.setState({ last: newLast},  async () => await this.refreshData(false, undefined, newLast));
                                }} />
                            <Select
                                defaultValue="day"
                                value={this.state.unit}
                                onChange={(value) => {
                                    const newUnit = value || 'day';
                                    this.setState({ unit: value }, async () => await this.refreshData(false, newUnit, undefined));
                                }}
                                options={[
                                    { value: 'day', label: 'day' },
                                    { value: 'hour', label: 'hour' },
                                ]}
                            />
                        </Flex>
                    </Flex>
                    <ResponsiveLine
                        theme={theme}
                        data={valid_evolution_data}
                        margin={{ top: 50, right: 110, bottom: 50, left: 60 }}
                        xScale={xScaleConfig}
                        yScale={{ type: 'linear', min: 'auto', max: 'auto', stacked: false, reverse: false }}
                        curve="monotoneX"
                        enablePoints={true}
                        axisTop={null}
                        axisRight={null}
                        axisBottom={{
                            tickSize: 5,
                            tickPadding: 5,
                            tickRotation: 0,
                            format: axisBottomFormat,
                            legend: axisBottomLegend,
                            legendOffset: legendOffset,
                            legendPosition: 'middle',
                            truncateTickAt: 0
                        }}
                        axisLeft={{
                            tickSize: 5,
                            tickPadding: 5,
                            tickRotation: 0,
                            legend: 'Requests',
                            legendOffset: -50,
                            legendPosition: 'middle',
                            truncateTickAt: 0
                        }}
                        pointSize={10}
                        pointColor={{ theme: 'background' }}
                        pointBorderWidth={2}
                        pointBorderColor={{ from: 'serieColor' }}
                        useMesh={true}
                        legends={[
                            {
                                anchor: 'bottom-right',
                                direction: 'column',
                                justify: false,
                                translateX: 100,
                                translateY: 0,
                                itemsSpacing: 0,
                                itemDirection: 'left-to-right',
                                itemWidth: 80,
                                itemHeight: 20,
                                itemOpacity: 0.75,
                                symbolSize: 12,
                                symbolShape: 'circle',
                                symbolBorderColor: 'rgba(0, 0, 0, .5)',
                                effects: [
                                    {
                                        on: 'hover',
                                        style: {
                                            itemBackground: 'rgba(0, 0, 0, .03)',
                                            itemOpacity: 1
                                        }
                                    }
                                ]
                            }
                        ]}
                    />
                </Flex>
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

