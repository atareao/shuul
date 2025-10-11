import react from "react";
import {
    BrowserRouter,
    Routes,
    Route,
} from "react-router";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";
import Backend from "i18next-http-backend";

import { Button } from "@/components/ui/button";
import { AuthContextProvider } from "./components/auth_context";
import ModeContext, { ModeContextProvider } from "./components/mode_context";
import { ThemeProvider } from "@/components/theme-provider";
import './App.css'

import PublicLayout from "./layouts/public_layout";
import AdminLayout from "./layouts/admin_layout";

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
export default class App extends react.Component {
    static contextType = ModeContext;
    declare context: React.ContextType<typeof ModeContext>;

    render = () => {
        return (
            <AuthContextProvider>
                <ModeContextProvider>
                    <ThemeProvider defaultTheme="dark" storageKey="vite-ui-theme">
                        <BrowserRouter>
                            <Routes>
                                <Route path="/" element={<PublicLayout />} >
                                    <Route index element={<HomePage />} />
                                    <Route path="services" element={<ServicesPage />} />
                                    <Route path="about" element={<AboutPage />} />
                                </Route>
                                <Route path="/admin" element={<AdminLayout />} >
                                    <Route index element={<HelpPage />} />
                                </Route>
                                </Routes>
                            </BrowserRouter>
                    </ThemeProvider>
                </ModeContextProvider>
            </AuthContextProvider>
        );
    }
}
