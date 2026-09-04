import React from "react";
import { Navigate } from "react-router";
import { Button, Flex } from "antd";

import AuthContext from "@/components/auth_context";
import { BASE_URL } from "@/constants";
import Logo from "@/assets/logo.svg";

interface State {
  redirect: boolean;
  redirectToAdmin: boolean;
}

// SSO-only login: no local registration, no local login.
// If SSO is not configured, the backend will refuse to start.
export default class LoginPage extends React.Component<{}, State> {
  static contextType = AuthContext;
  declare context: React.ContextType<typeof AuthContext>;

  constructor(props: {}) {
    console.log("Constructing login page");
    super(props);
    this.state = {
      redirect: false,
      redirectToAdmin: false,
    };
  }

  handleSsoLogin = () => {
    window.location.href = `${BASE_URL}/api/v1/auth/sso`;
  };

  render = () => {
    console.log("Rendering login page");

    // If already logged in, redirect to admin
    if (this.context.isLoggedIn) {
      return <Navigate to="/admin/" />;
    }

    return (
      <Flex justify="center" align="center" style={{ minHeight: "100vh" }}>
        <Flex gap="middle" align="center" vertical>
          <img src={Logo} alt="Logo" style={{ width: 200, marginBottom: 20 }} />
          <Button type="primary" size="large" onClick={this.handleSsoLogin}>
            Sign in with PocketID
          </Button>
        </Flex>
      </Flex>
    );
  };
}
