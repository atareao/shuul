import React from "react";

export type ToggleModeHandler = () => void;

export interface ModeContextInterface {
  isDarkMode: boolean;
  toggleMode: ToggleModeHandler;
}

const ModeContext = React.createContext<ModeContextInterface>({
  isDarkMode: true,
  toggleMode: () => {},
});

interface Props {
  children: React.ReactNode;
}

interface State {
  isDarkMode: boolean;
}

export class ModeContextProvider extends React.Component<Props, State> {
  constructor(props: Props) {
    console.log("Constructing AuthContextProvider");
    super(props);
    this.state = {
      isDarkMode: this.retrieveMode(),
    };
  }

  retrieveMode = () => {
    console.log("Retrieving mode");
    const mode = localStorage.getItem("mode");
    if (mode === undefined || mode === null) {
      return true;
    }
    return mode == "dark";
  };

  toggleMode = () => {
    const oldState = this.state.isDarkMode;
    const newState = !this.state.isDarkMode;
    this.setState({ isDarkMode: newState });
    localStorage.setItem("mode", newState ? "dark" : "light");
    console.log(`Change isDarkMode from ${oldState} to ${newState}`);
  };

  render() {
    console.log(`Rendering ModeContextProvider ${this.state.isDarkMode}`);
    return (
      <ModeContext.Provider
        value={{
          isDarkMode: this.state.isDarkMode,
          toggleMode: this.toggleMode,
        }}
      >
        {this.props.children}
      </ModeContext.Provider>
    );
  }
}
export default ModeContext;
