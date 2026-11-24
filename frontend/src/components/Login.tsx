import { useMutation } from "@tanstack/react-query";
import { useForm } from "react-hook-form";
import { errorMessage, login } from "../api";

type FormValues = {
  email: string;
  password: string;
};

type Props = {
  onSuccess: (loginChallengeId: string) => void;
};

export default function Login({ onSuccess }: Props) {
  const {
    register,
    handleSubmit,
    formState: { errors },
  } = useForm<FormValues>();

  const mutation = useMutation({
    mutationFn: (values: FormValues) => login(values.email, values.password),
    onSuccess: (data) => onSuccess(data.login_challenge_id),
  });

  return (
    <section className="card">
      <h2>Login</h2>
      <form onSubmit={handleSubmit((values) => mutation.mutate(values))}>
        <label className="field">
          Email
          <input
            type="email"
            autoComplete="username"
            {...register("email", { required: "Email is required" })}
          />
          {errors.email && (
            <span className="error">{errors.email.message}</span>
          )}
        </label>

        <label className="field">
          Password
          <input
            type="password"
            autoComplete="current-password"
            {...register("password", { required: "Password is required" })}
          />
          {errors.password && (
            <span className="error">{errors.password.message}</span>
          )}
        </label>

        <button type="submit" disabled={mutation.isPending}>
          {mutation.isPending ? "Signing in…" : "Login"}
        </button>

        {mutation.isError && (
          <p className="error">{errorMessage(mutation.error)}</p>
        )}
      </form>
    </section>
  );
}
