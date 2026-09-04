import { useContext, useEffect } from "react";
import AuthContext from "@/components/auth_context";

export default function LogoutPage() {
  const { logout } = useContext(AuthContext);

  useEffect(() => {
    logout();
  }, [logout]);

  return null;
}
