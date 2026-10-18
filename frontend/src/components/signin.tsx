import React from "react";
import { useTranslation } from "react-i18next";
import { Card, Input, Button } from "antd";

/** Firma exacta de la función de traducción devuelta por `useTranslation`. */
type Translate = ReturnType<typeof useTranslation>["t"];

/** Datos del formulario enviados al consumidor del componente. */
export interface UserCredentials {
  email: string;
  password: string;
}

export type SignInSubmitHandler = (userData: UserCredentials) => void;

interface Props {
  t: Translate;
  onSubmit: SignInSubmitHandler;
  responseMessage: string;
}

interface State {
  email: string;
  password: string;
}

class InnerSignIn extends React.Component<Props, State> {
  constructor(props: Props) {
    console.log("Constructing sign in form");
    super(props);
    this.reset();
  }

  reset = () => {
    this.state = {
      email: "",
      password: "",
    };
  };

  handleEmailChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    console.log("Email changed");
    this.setState({ email: e.target.value });
  };

  handlePasswordChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    console.log("Password changed");
    this.setState({ password: e.target.value });
  };

  handleSubmit = (e: React.MouseEvent<HTMLElement>) => {
    e.preventDefault();
    console.log("Submitting sign in form");
    const userData: UserCredentials = {
      email: this.state.email,
      password: this.state.password,
    };
    console.log(`Submitting user data: ${JSON.stringify(userData)}`);
    this.props.onSubmit(userData); // Pass user data to onSubmit function
  };
  render = () => {
    console.log("Rendering sign in form");
    const { t } = this.props;
    return (
      <Card title="Login" style={{ width: 300 }}>
        <Input
          id="email"
          type="email"
          required
          placeholder={t("Email")}
          value={this.state.email}
          onChange={this.handleEmailChange}
        />
        <br />
        <br />
        <Input
          id="password"
          type="password"
          required
          placeholder={t("Password")}
          value={this.state.password}
          onChange={this.handlePasswordChange}
        />
        <br />
        <br />
        <Button onClick={this.handleSubmit}>{t("Sign in")}</Button>
      </Card>
    );
  };
}

interface SignInProps {
  onSubmit: SignInSubmitHandler;
  responseMessage: string;
}

export default function SignIn(props: SignInProps) {
  const { t } = useTranslation();
  return (
    <InnerSignIn
      t={t}
      onSubmit={props.onSubmit}
      responseMessage={props.responseMessage}
    />
  );
}
