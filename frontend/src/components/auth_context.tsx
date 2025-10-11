import React from 'react';
import { jwtDecode } from 'jwt-decode';

declare module 'jwt-decode' {
    export interface JwtPayload {
        role: string
    }
}

export interface AuthContextInterface {
    token: string | null
    isLoggedIn: boolean
    isAdmin: boolean
    role: string
    login: Function
    logout: Function
}

const AuthContext = React.createContext<AuthContextInterface>({
    token: "",
    isLoggedIn: false,
    isAdmin: false,
    role: "",
    login: (token: string) => { console.log(`token: ${token}`) },
    logout: () => { }

});


interface Props {
    children: React.ReactNode
}

interface State {
    role: string
    token: string | null
    isLoggedIn: boolean
    isAdmin: boolean
}

export class AuthContextProvider extends React.Component<Props, State> {

    private logoutTimer: ReturnType<typeof setTimeout> | null = null;

    constructor(props: any) {
        console.log("Constructing AuthContextProvider");
        super(props);
        this.logoutTimer = null;
        const tokenData = this.retrieveStoredToken();
        if (tokenData && tokenData.token) {
            if(jwtDecode(tokenData.token)){
            const decoded = jwtDecode(tokenData.token);
            const role = decoded.role;
            this.state = {
                role: role,
                token: tokenData.token,
                isLoggedIn: true,
                isAdmin: role === "admin"
            }
            }
        }else{
            this.state = {
                role: "",
                token: null,
                isLoggedIn: false,
                isAdmin: false
            }
        }
    }


    calculateRemainingTime = (expirationTime: number) => {
        console.log(`Calculating remaining time for ${expirationTime}`);
        if (!expirationTime) {
            return 0;
        }
        const remainingTime = expirationTime * 1000 - new Date().getTime();
        console.log(`Remaining time: ${remainingTime / 1000} secs`);
        return remainingTime;
    }

    retrieveStoredToken = () => {
        console.log("Retrieving stored token");
        const storedToken = localStorage.getItem("token");
        let expirationTime = 0;
        if(storedToken) {
            const decoded = jwtDecode(storedToken);
            expirationTime = decoded.exp ? decoded.exp : 0;
        }
        const remainingTime = this.calculateRemainingTime(expirationTime);
        if (remainingTime <= 0) {
            localStorage.removeItem("token");
            return null;
        }

        return {
            token: storedToken,
            duration: remainingTime,
        };
    }

    logoutHandler = () => {
        console.log("Logging out");
        this.setState({
            isLoggedIn: false,
            isAdmin: false,
            token: null
        });
        localStorage.removeItem("token");
        if(this.logoutTimer) {
            clearTimeout(this.logoutTimer);
        }
        window.history.pushState({}, "", "/login");
    }

    loginHandler = (token: string) => {
        console.log(`Logging in with token: ${token}`);
        console.log("==================");
        console.log(`Encoded: ${token}`);
        const decoded = jwtDecode(token);
        console.log(`Decoded: ${decoded}`);
        console.log("==================");

        localStorage.setItem("token", token);
        const expirationTime = decoded.exp ? decoded.exp : 0;
        const remainingTime = this.calculateRemainingTime(expirationTime);
        if(remainingTime <= 0) {
            console.log("Token has already expired");
            this.logoutHandler();
            return;
        }
        this.logoutTimer = setTimeout(this.logoutHandler, remainingTime); // that will log the user out when this timer expires
        this.setState({
            isLoggedIn: !!token,
            isAdmin: decoded.role === "admin",
            role: decoded.role,
            token: token
        });
    }

    render() {
        console.log("Rendering AuthContextProvider");
        return (
            <AuthContext.Provider value={{
                token: this.state.token,
                role: this.state.role,
                isLoggedIn: this.state.isLoggedIn,
                isAdmin: this.state.isAdmin,
                login: this.loginHandler,
                logout: this.logoutHandler
            }}>
                {this.props.children}
            </AuthContext.Provider>
        )
    }
}
export default AuthContext;

