import React from "react";
import { Switch } from "antd";
import { MoonOutlined, SunOutlined } from "@ant-design/icons";

import ModeContext from "@/components/mode_context";

export default class ModeSwitcher extends React.Component {
  render = () => {
    return (
      <ModeContext.Consumer>
        {({ isDarkMode, toggleMode }) => {
          console.log(`Rendering ModeSwitcher with isDarkMode ${isDarkMode}`);
          return (
            <Switch
              checkedChildren={<MoonOutlined />}
              unCheckedChildren={<SunOutlined />}
              checked={isDarkMode}
              onChange={() => toggleMode()}
              aria-label="Toggle dark mode"
            />
          );
        }}
      </ModeContext.Consumer>
    );
  };
}
