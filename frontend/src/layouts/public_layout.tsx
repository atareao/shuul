import react from "react";
import { Outlet } from "react-router";
import { Layout, theme } from "antd";
import type { GlobalToken } from "antd";

const { Footer, Content } = Layout;

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
        {/* El contenido público es un contenedor flexible en columna: así la
            portada ocupa el alto disponible en vez de imponerle el suyo, y
            quitar la cabecera no deja un desplazamiento residual. */}
        <Content
          style={{
            padding: "24px",
            flex: 1,
            display: "flex",
            flexDirection: "column",
          }}
        >
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
