import react from 'react';
import { useNavigate, Navigate, Outlet } from 'react-router';
import { Button, Layout, Menu, theme } from 'antd';
import type { MenuProps } from 'antd';
import {
    HomeOutlined,
    OrderedListOutlined,
    MenuUnfoldOutlined,
    PieChartOutlined,
    QuestionOutlined,
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
    navigateTo?: string,
): MenuItem {
    return {
        key,
        icon,
        children,
        label,
        navigateTo,
    } as MenuItem;
}

const navigations:{[key: string]: string}  = {
    1: "/admin/dashboard",
    2: "/admin/rules",
    3: "/admin/records",
    4: "/admin/ignored",
    5: "/admin/charts",
    6: "/admin/users",
}

const items: MenuItem[] = [
    getItem('Dashboard', '1', <HomeOutlined />),
    getItem('Rules', '2', <OrderedListOutlined />),
    getItem('Records', '3', <MenuUnfoldOutlined />),
    getItem('Ignored', '4', <QuestionOutlined />),
    getItem('Charts', '5', <PieChartOutlined />),
    getItem('Users', '6', <UserOutlined />),
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

    handleMenuClick = (e: any) => {
        console.log(e)
        this.props.navigate(navigations[e.key]);
    }

    render = () => {
        console.log("AdminLayout");
        console.log(window.location.pathname);
        const selectedKey = Object.keys(navigations).find(key => navigations[key] === window.location.pathname) || '1';
        return (
            <AuthContext.Consumer>
                {({ isLoggedIn, role}) => {
                    if( isLoggedIn === false || role !== ROLE){
                        return <Navigate to="/login" />;
                    }
                    return (
                        <Layout style={{ minHeight: '100vh' }}>
                            <Sider collapsible collapsed={this.state.collapsed} onCollapse={(value) => this.setCollapsed(value)}>
                                <div className="demo-logo-vertical" />
                                <Menu
                                    theme="dark"
                                    defaultSelectedKeys={['1']}
                                    selectedKeys={[selectedKey]}
                                    mode="inline"
                                    items={items}
                                    onClick={(e) => {this.handleMenuClick(e)}}
                                />
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
                                        onClick={() => this.props.navigate('/admin/logout')}
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
                                    ©{new Date().getFullYear()} Shuul
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
