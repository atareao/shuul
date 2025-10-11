import React from 'react';

export interface ModeContextInterface {
    mode: string
    toggleMode: Function
}

const ModeContext = React.createContext<ModeContextInterface>({
    mode: "dark",
    toggleMode: Function,
});


interface Props {
    children: React.ReactNode
}

interface State {
    mode: string
}

export class ModeContextProvider extends React.Component<Props, State> {

    constructor(props: any) {
        console.log("Constructing AuthContextProvider");
        super(props);
        this.state = {
            mode: this.retrieveMode()
        }
    }

    retrieveMode = () => {
        console.log("Retrieving mode");
        let mode = localStorage.getItem("mode");
        if (mode === undefined || mode === null) {
            return "light";
        }
        return mode;
    }

    toggleMode = () => {
        const oldState = this.state.mode;
        const newState = this.state.mode === "light"?"dark":"light";
        this.setState({ mode: newState });
        localStorage.setItem("mode", newState);
        console.log(`Change mode from ${oldState} to ${newState}`);
    }

    render() {
        console.log(`Rendering ModeContextProvider ${this.state.mode}`);
        return (
            <ModeContext.Provider value={{
                mode: this.state.mode,
                toggleMode: this.toggleMode,
            }}>
                {this.props.children}
            </ModeContext.Provider>
        )
    }
}
export default ModeContext;



