import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { setAuthToken } from "./api";
import type { User } from "./api";
import Login from "./components/Login";
import Otp from "./components/Otp";
import Admin from "./components/Admin";
import Staff from "./components/Staff";

type Screen =
  | { name: "login" }
  | { name: "otp"; challengeId: string }
  | { name: "dashboard"; user: User };

export default function App() {
  const queryClient = useQueryClient();
  const [screen, setScreen] = useState<Screen>({ name: "login" });

  // Clears the token and all cached data so the next user starts fresh.
  function logout() {
    setAuthToken(null);
    queryClient.clear();
    setScreen({ name: "login" });
  }

  return (
    <main className="container">
      <h1>Task Management</h1>

      {screen.name === "login" && (
        <Login
          onSuccess={(challengeId) => setScreen({ name: "otp", challengeId })}
        />
      )}

      {screen.name === "otp" && (
        <Otp
          challengeId={screen.challengeId}
          onVerified={(user) => setScreen({ name: "dashboard", user })}
          onBack={() => setScreen({ name: "login" })}
        />
      )}

      {screen.name === "dashboard" &&
        (screen.user.role === "admin" ? (
          <Admin user={screen.user} onLogout={logout} />
        ) : (
          <Staff user={screen.user} onLogout={logout} />
        ))}
    </main>
  );
}
