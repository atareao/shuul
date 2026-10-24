import react from "react";
import { useNavigate, Navigate, Outlet } from "react-router";
import type { NavigateFunction } from "react-router";
import { Button, Layout, Menu, theme } from "antd";
import type { GlobalToken, MenuProps } from "antd";
import {
  HomeOutlined,
  OrderedListOutlined,
  PieChartOutlined,
  LogoutOutlined,
  StopOutlined,
  AppstoreOutlined,
  SettingOutlined,
  RocketOutlined,
  EyeOutlined,
  MenuFoldOutlined,
  MenuUnfoldOutlined,
} from "@ant-design/icons";

import ModeSwitcher from "@/components/mode_switcher";
import AuthContext from "@/components/auth_context";
import { VERSION } from "@/constants";

const TITLE = `Shuul (${VERSION})`;
const { Content, Footer, Sider } = Layout;

type MenuItem = Required<MenuProps>["items"][number];
/** Información que Ant Design entrega al hacer clic en una entrada del menú. */
type MenuClickInfo = Parameters<NonNullable<MenuProps["onClick"]>>[0];

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

const navigations: { [key: string]: string } = {
  1: "/admin/dashboard",
  2: "/admin/rules",
  3: "/admin/rate-limit-profiles",
  4: "/admin/bans",
  5: "/admin/templates",
  6: "/admin/charts",
  7: "/admin/logs",
  8: "/admin/settings",
};

const items: MenuItem[] = [
  getItem("Dashboard", "1", <HomeOutlined />),
  getItem("Rules", "2", <OrderedListOutlined />),
  getItem("Rate Limit Profiles", "3", <RocketOutlined />),
  getItem("Bans", "4", <StopOutlined />),
  getItem("Templates", "5", <AppstoreOutlined />),
  getItem("Charts", "6", <PieChartOutlined />),
  getItem("Logs", "7", <EyeOutlined />),
  getItem("Settings", "8", <SettingOutlined />),
];

// Componente AuthGuard: solo verifica login y redirige si no está autenticado
const AuthGuard = ({ children }: { children: React.ReactNode }) => (
  <AuthContext.Consumer>
    {({ isLoggedIn }) => {
      if (isLoggedIn === false) {
        return <Navigate to="/" />;
      }
      return <>{children}</>;
    }}
  </AuthContext.Consumer>
);

interface Props {
  token: GlobalToken;
  navigate: NavigateFunction;
}
interface State {
  collapsed: boolean;
}

class InnerAdminLayout extends react.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = {
      collapsed: false,
    };
  }
  setCollapsed = (collapsed: boolean) => {
    this.setState({ collapsed });
  };

  handleMenuClick = (e: MenuClickInfo) => {
    console.log(e);
    this.props.navigate(navigations[e.key]);
  };

  render = () => {
    console.log("AdminLayout");
    console.log(window.location.pathname);
    const selectedKey =
      Object.keys(navigations).find(
        (key) => navigations[key] === window.location.pathname,
      ) || "1";
    return (
      <AuthGuard>
        <Layout style={{ minHeight: "100vh" }}>
          <Sider
            collapsible
            trigger={null}
            collapsed={this.state.collapsed}
            onCollapse={(value) => this.setCollapsed(value)}
            styles={{ body: { display: "flex", flexDirection: "column" } }}
          >
            <div className="demo-logo-vertical" />
            <Menu
              theme="dark"
              defaultSelectedKeys={["1"]}
              selectedKeys={[selectedKey]}
              mode="inline"
              items={items}
              onClick={(e) => {
                this.handleMenuClick(e);
              }}
            />
            {/* El pie es hermano del menú dentro del cuerpo del `Sider`, no lo
                envuelve: así el menú nunca queda dentro del pie. El
                `marginTop: auto` lo ancla al fondo sin depender de CSS global.
                Con la barra colapsada el raíl mide 80 px, así que los tres
                controles pasan a columna para no desbordarlo. */}
            <div
              style={{
                marginTop: "auto",
                display: "flex",
                flexDirection: this.state.collapsed ? "column" : "row",
                alignItems: "center",
                justifyContent: "center",
                gap: 8,
                padding: 8,
              }}
            >
              <Button
                variant="solid"
                aria-label="Log out"
                onClick={() => this.props.navigate("/admin/logout")}
              >
                <LogoutOutlined />
              </Button>
              <ModeSwitcher />
              <Button
                variant="solid"
                aria-label={
                  this.state.collapsed ? "Expand sidebar" : "Collapse sidebar"
                }
                onClick={() => this.setCollapsed(!this.state.collapsed)}
              >
                {this.state.collapsed ? (
                  <MenuUnfoldOutlined />
                ) : (
                  <MenuFoldOutlined />
                )}
              </Button>
            </div>
          </Sider>
          <Layout>
            <Content style={{ margin: "0 16px" }}>
              <div
                style={{
                  padding: 24,
                  minHeight: 360,
                }}
              >
                <Outlet />
              </div>
            </Content>
            <Footer style={{ textAlign: "center" }}>
              ©{new Date().getFullYear()} {TITLE}
            </Footer>
          </Layout>
        </Layout>
      </AuthGuard>
    );
  };
}

export default function AdminLayout() {
  const navigate = useNavigate();
  const { token } = theme.useToken();
  return <InnerAdminLayout navigate={navigate} token={token} />;
}
