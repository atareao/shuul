import react from "react";
import { useNavigate, Navigate, Outlet } from "react-router";
import type { NavigateFunction } from "react-router";
import { Layout, Menu, theme } from "antd";
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
  MoonOutlined,
  SunOutlined,
} from "@ant-design/icons";

import AuthContext from "@/components/auth_context";
import ModeContext from "@/components/mode_context";
import { TITLE } from "@/constants";
import Logo from "@/assets/logo.svg";

const { Content, Footer, Sider } = Layout;

/** Identificador de la barra lateral, para que el control de colapsar la anuncie. */
const SIDER_ID = "admin-sider";

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
  navigate: NavigateFunction;
  /** Token de Ant Design del tema activo, para el color del texto de la marca. */
  token: GlobalToken;
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

  /**
   * Despacha las entradas del pie por su clave. El tema no se resuelve aquí:
   * `toggleMode` llega desde el consumidor del contexto de tema.
   */
  handleFooterClick = (e: MenuClickInfo, toggleMode: () => void) => {
    switch (e.key) {
      case "theme":
        toggleMode();
        break;
      case "logout":
        this.props.navigate("/admin/logout");
        break;
      case "collapse":
        this.setCollapsed(!this.state.collapsed);
        break;
    }
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
          {/* `trigger={null}` es imprescindible: sin él antd pintaría su
              disparador por defecto dentro de la barra. El disparador real es
              la entrada de colapso del pie, así que ni `collapsible` ni
              `onCollapse` tienen ya sentido (no habría nada que los disparase).
              La barra se pega a la ventana y ocupa su altura: así el pie nunca
              queda por debajo del pliegue y es el menú de navegación el que se
              desplaza dentro. */}
          <Sider
            id={SIDER_ID}
            trigger={null}
            collapsed={this.state.collapsed}
            style={{ position: "sticky", top: 0, height: "100vh" }}
            styles={{ body: { display: "flex", flexDirection: "column" } }}
          >
            {/* La marca encabeza la barra: icono siempre, nombre solo cuando la
                barra está desplegada. */}
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                gap: 8,
                padding: "16px 0",
                color: this.props.token.colorTextLightSolid,
              }}
            >
              <img src={Logo} alt="Shuul" style={{ height: 32, width: 32 }} />
              {!this.state.collapsed && <span>Shuul</span>}
            </div>
            <Menu
              theme="dark"
              defaultSelectedKeys={["1"]}
              selectedKeys={[selectedKey]}
              mode="inline"
              items={items}
              style={{ flex: 1, overflowY: "auto", minHeight: 0 }}
              onClick={(e) => {
                this.handleMenuClick(e);
              }}
            />
            {/* El pie es un `Menu` hermano del de navegación, no lo envuelve:
                así el menú nunca queda dentro del pie. Sus tres entradas no son
                seleccionables y se pliegan a iconos con la barra. */}
            <ModeContext.Consumer>
              {({ isDarkMode, toggleMode }) => (
                <Menu
                  theme="dark"
                  mode="inline"
                  selectable={false}
                  onClick={(e) => this.handleFooterClick(e, toggleMode)}
                >
                  <Menu.Item
                    key="theme"
                    icon={isDarkMode ? <MoonOutlined /> : <SunOutlined />}
                  >
                    Theme
                  </Menu.Item>
                  <Menu.Item key="logout" icon={<LogoutOutlined />}>
                    Log out
                  </Menu.Item>
                  <Menu.Item
                    key="collapse"
                    icon={
                      this.state.collapsed ? (
                        <MenuUnfoldOutlined />
                      ) : (
                        <MenuFoldOutlined />
                      )
                    }
                    aria-expanded={!this.state.collapsed}
                    aria-controls={SIDER_ID}
                  >
                    Collapse
                  </Menu.Item>
                </Menu>
              )}
            </ModeContext.Consumer>
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
