import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography, Avatar, Card } from 'antd';

import { loadData } from "@/common/utils";
import {VERSION} from "@/constants";
import Logo from "@/assets/logo.svg";

const TITLE = `Shuul (${VERSION})`;

interface Props {
    navigate: any
    t: any
}

interface State {
    loading: boolean;
    total_rules: number,
    total_active_rules: number,
    total_requests: number,
    total_filtered_requests: number,
    total_active_bans: number,
}


export class InnerPage extends react.Component<Props, State> {

    constructor(props: Props) {
        super(props);
        this.state = {
            loading: true,
            total_rules: 0,
            total_active_rules: 0,
            total_requests: 0,
            total_filtered_requests: 0,
            total_active_bans: 0,
        }
    }
    componentDidMount = async () => {

        const [rules_info_res, total_requests_res, total_filtered_requests_res, total_active_bans_res] = await Promise.all([
            loadData("rules/info/all"),
            loadData("requests/info", new Map([["option", "total"]])),
            loadData("requests/info", new Map([["option", "filtered"]])),
            loadData("bans/info", new Map()),
        ]);
        console.log("Totals loaded:", rules_info_res, total_requests_res, total_filtered_requests_res, total_active_bans_res);
        this.setState({
            loading: false,
            total_rules: rules_info_res.status === 200 ? (rules_info_res.data as any).total : 0,
            total_active_rules: rules_info_res.status === 200 ? (rules_info_res.data as any).active : 0,
            total_requests: total_requests_res.status === 200 ? total_requests_res.data as number : 0,
            total_filtered_requests: total_filtered_requests_res.status === 200 ? total_filtered_requests_res.data as number : 0,
            total_active_bans: total_active_bans_res.status === 200 ? total_active_bans_res.data as number : 0,
        });

    }

    render = () => {
        return (
            <Flex vertical justify="center" align="center" gap="middle" style={{ height: '100vh' }}>
                <Card loading={this.state.loading} style={{ minWidth: 600 }}>
                    <Card.Meta
                        avatar={<Avatar src={Logo} />}
                        title={<Typography.Title level={3} style={{ margin: 0 }}>{TITLE}</Typography.Title>}
                        description={
                            <>
                                <Typography.Title
                                    level={4}
                                    style={{ margin: 5, cursor: "pointer" }}
                                    onClick={() => this.props.navigate("/admin/rules")}
                                >
                                    {`${this.props.t("Total of rules")}: ${this.state.total_rules}`}
                                </Typography.Title>
                                <Typography.Title
                                    level={4}
                                    style={{ margin: 5, cursor: "pointer" }}
                                    onClick={() => this.props.navigate("/admin/rules")}
                                >
                                    {`${this.props.t("Total of active rules")}: ${this.state.total_active_rules}`}
                                </Typography.Title>
                                <Typography.Title
                                    level={4}
                                    style={{ margin: 5, cursor: "pointer" }}
                                    onClick={() => this.props.navigate("/admin/records")}
                                >
                                    {`${this.props.t("Total of requests")}: ${this.state.total_requests}`}
                                </Typography.Title>
                                <Typography.Title
                                    level={4}
                                    style={{ margin: 5, cursor: "pointer" }}
                                    onClick={() => this.props.navigate("/admin/records")}
                                >
                                    {`${this.props.t("Total of filtered requests")}: ${this.state.total_filtered_requests}`}
                                </Typography.Title>
                                <Typography.Title
                                    level={4}
                                    style={{ margin: 5, cursor: "pointer" }}
                                    onClick={() => this.props.navigate("/admin/bans")}
                                >
                                    {`${this.props.t("Active bans")}: ${this.state.total_active_bans}`}
                                </Typography.Title>
                            </>
                        }
                    />
                </Card>
            </Flex >

        );
    }
}

export default function DashboardPage() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerPage navigate={navigate} t={t} />;
}
