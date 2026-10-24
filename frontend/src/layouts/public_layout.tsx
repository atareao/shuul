import react from "react";
import { Outlet } from "react-router";
import { Button, Layout, theme } from "antd";
import type { GlobalToken } from "antd";
import { LoginOutlined } from "@ant-design/icons";

import ModeSwitcher from "@/components/mode_switcher";
import { startSsoLogin } from "@/common/auth";

const { Header, Footer, Content } = Layout;

interface Props {
  /**
   * Token de Ant Design del tema activo.
   *
   * `coloBgLayout` es la clave *heredada* que lee este layout: parece un typo de
   * `colorBgLayout`, pero se conserva tal cual (en la práctica vale `undefined`)
   * para no alterar el estilo actual. Lo único que hacemos es declararla para
   * poder tipar el acceso sin `any`.
   */
  token: GlobalToken & { coloBgLayout?: string };
}

class InnerPublicLayout extends react.Component<Props> {
  render = () => {
    return (
      <Layout
        style={{
          minHeight: "100vh",
          backgroundColor: this.props.token.coloBgLayout,
          color: this.props.token.colorText,
        }}
      >
        <Header
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "flex-end",
            // La separación entre el botón de entrar y el conmutador de tema
            // es cosa del contenedor: el conmutador ya no declara márgenes
            // propios, porque dentro del pie del panel descuadrarían.
            gap: 16,
            height: 64,
            paddingInline: 48,
          }}
        >
          <Button variant="solid" onClick={() => startSsoLogin()}>
            <LoginOutlined />
          </Button>
          <ModeSwitcher />
        </Header>
        <Content style={{ padding: "24px", flex: 1 }}>
          <Outlet />
        </Content>

        <Footer style={{ textAlign: "center" }}>
          ©{new Date().getFullYear()} atareao
        </Footer>
      </Layout>
    );
  };
}

export default function PublicLayout() {
  const { token } = theme.useToken();
  return <InnerPublicLayout token={token} />;
}
