import React from "react";
import {
    BrowserRouter,
    Routes,
    Route,
} from "react-router";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";
import Backend from "i18next-http-backend";
import { ConfigProvider, theme } from "antd";
import '@ant-design/v5-patch-for-react-19';

import { AuthContextProvider } from "@/components/auth_context";
import ModeContext, { ModeContextProvider } from "@/components/mode_context";
import PublicLayout from "@/layouts/public_layout";
import AdminLayout from "@/layouts/admin_layout";
import HomePage from "@/pages/home_page";
import LoginPage from "@/pages/login_page";
import HelpPage from "@/pages/help_page";
import '@/App.css'

i18n
    .use(Backend)
    .use(LanguageDetector)
    .use(initReactI18next) // passes i18n down to react-i18next
    .init({
        load: 'languageOnly',
        lng: localStorage.getItem("i18nextLng") || "es",
        fallbackLng: "en",
        debug: true,
        interpolation: {
            escapeValue: false,
        }
    });
export default class App extends React.Component {
    render = () => {
        return (
            <AuthContextProvider>
                <ModeContextProvider>
                    <ModeContext.Consumer>
                        {({ isDarkMode }) => {
                            console.log(`Rendering App with isDarkMode ${isDarkMode}`);
                            const algorithm = isDarkMode ? theme.darkAlgorithm : theme.defaultAlgorithm;
                            const customTheme = {
                                algorithm: algorithm,
                                token: {
                                    colorPrimary: "#fa541c",
                                    colorInfo: "#fa541c",
                                    fontSize: 16
                                }
                            };
                            return (
                                <ConfigProvider theme={customTheme}>
                                    <BrowserRouter>
                                        <Routes>
                                            <Route path="/" element={<PublicLayout />} >
                                                <Route index element={<HomePage />} />
                                                <Route path="login" element={<LoginPage />} />
                                            </Route>
                                            <Route path="/admin" element={<AdminLayout />} >
                                                <Route index element={<HelpPage />} />
                                            </Route>
                                        </Routes>
                                    </BrowserRouter>
                                </ConfigProvider>
                            );
                        }}
                    </ModeContext.Consumer>
                </ModeContextProvider>
            </AuthContextProvider>
        );
    }
}
