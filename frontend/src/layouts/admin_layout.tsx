import react from 'react';
import { Navigate, Outlet } from 'react-router';
import AuthContext from '../components/auth_context';

const ROLE = "admin";

export default class AdminLayout extends react.Component {
    static authContext = AuthContext;
    declare context: React.ContextType<typeof AuthContext>;

    comoponentDidMount = () => {
        console.log("ProtectedLayout.componentDidMount");
        const token = this.context;
        console.log(`token: ${JSON.stringify(token)}`);
    }

    render = () => {
        console.log("AdminLayout");
        console.log(`token: ${JSON.stringify(this.context)}`);
        if (!this.context.isLoggedIn || this.context.role !== ROLE) {
            return <Navigate to="/login" />;
        }
        return (
            <>
                <header>
                    <NavBar />
                    <Sidebar />
                </header>
                <main>
                    <Container>
                        <Outlet />
                    </Container>
                </main>
                <footer>
                </footer>
            </>
        );
    }
}
AdminLayout.contextType = AuthContext;


