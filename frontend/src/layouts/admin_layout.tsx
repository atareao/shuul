import react from 'react';
import { useNavigate, Navigate, Outlet } from 'react-router';
import { Button, Layout, Menu, theme } from 'antd';
import type { MenuProps } from 'antd';
import {
    HomeOutlined,
    OrderedListOutlined,
    MenuUnfoldOutlined,
    PieChartOutlined,
    LogoutOutlined,
    UserOutlined,
} from '@ant-design/icons';

import ModeSwitcher from '@/components/mode_switcher';
import AuthContext from '@/components/auth_context';

const ROLE = "admin";
const { Header, Content, Footer, Sider } = Layout;

type MenuItem = Required<MenuProps>['items'][number];

function getItem(
    label: React.ReactNode,
    key: React.Key,
    icon?: React.ReactNode,
    children?: MenuItem[],
): MenuItem {
    return {
        key,
        icon,
        children,
        label,
    } as MenuItem;
}

const items: MenuItem[] = [
    getItem('Dashboard', '1', <HomeOutlined />),
    getItem('Rules', '2', <OrderedListOutlined />),
    getItem('Records', '3', <MenuUnfoldOutlined />),
    getItem('Charts', '4', <PieChartOutlined />),
    getItem('Users', '5', <UserOutlined />),
];


interface Props {
    token: any;
    navigate: any;
}
interface State {
    collapsed: boolean;
}

class InnerAdminLayout extends react.Component<Props, State> {

    constructor(props: Props) {
        super(props);
        this.state = {
            collapsed: false,
        }
    }
    setCollapsed = (collapsed: boolean) => {
        this.setState({ collapsed });
    }


    render = () => {
        console.log("AdminLayout");
        return (
            <AuthContext.Consumer>
                {({ isLoggedIn, role}) => {
                    /*
                    if( isLoggedIn === false || role !== ROLE){
                        return <Navigate to="/login" />;
                    }
                    */
                    return (
                        <Layout style={{ minHeight: '100vh' }}>
                            <Sider collapsible collapsed={this.state.collapsed} onCollapse={(value) => this.setCollapsed(value)}>
                                <div className="demo-logo-vertical" />
                                <Menu theme="dark" defaultSelectedKeys={['1']} mode="inline" items={items} />
                            </Sider>
                            <Layout>
                                <Header
                                    style={{
                                        display: 'flex',
                                        alignItems: 'center',
                                        justifyContent: 'flex-end',
                                        height: 64,
                                        paddingInline: 48,
                                    }}
                                >
                                    <Button
                                        variant="solid"
                                        onClick={() => this.props.navigate('/login')}
                                    >
                                        <LogoutOutlined />
                                    </Button>
                                    <ModeSwitcher />
                                </Header>
                                <Content style={{ margin: '0 16px' }}>
                                    <div
                                        style={{
                                            padding: 24,
                                            minHeight: 360,
                                        }}
                                    >
                                        <Outlet />
                                    </div>
                                </Content>
                                <Footer style={{ textAlign: 'center' }}>
                                    ©{new Date().getFullYear()} atareao
                                </Footer>
                            </Layout>
                        </Layout>
                    );
                }}
            </AuthContext.Consumer>
        );
    }
}


export default function AdminLayout() {
    const navigate = useNavigate();
    const { token } = theme.useToken();
    return <InnerAdminLayout navigate={navigate} token={token} />;
}
