import { useMutation } from "@tanstack/react-query";
import { useForm } from "react-hook-form";
import { errorMessage, setAuthToken, verify2fa } from "../api";
import type { User } from "../api";

type FormValues = {
  code: string;
};

type Props = {
  challengeId: string;
  onVerified: (user: User) => void;
  onBack: () => void;
};

export default function Otp({ challengeId, onVerified, onBack }: Props) {
  const {
    register,
    handleSubmit,
    formState: { errors },
  } = useForm<FormValues>();

  const mutation = useMutation({
    mutationFn: (values: FormValues) => verify2fa(challengeId, values.code),
    onSuccess: (data) => {
      setAuthToken(data.access_token);
      onVerified(data.user);
    },
  });

  return (
    <section className="card">
      <h2>Enter your code</h2>
      <p className="hint">
        Check your Mailtrap inbox (in development, GET /dev/email-logs/latest)
      </p>
      <form onSubmit={handleSubmit((values) => mutation.mutate(values))}>
        <label className="field">
          6-digit code
          <input
            type="text"
            inputMode="numeric"
            maxLength={6}
            autoComplete="one-time-code"
            {...register("code", {
              required: "Code is required",
              pattern: {
                value: /^\d{6}$/,
                message: "Code must be exactly 6 digits",
              },
            })}
          />
          {errors.code && <span className="error">{errors.code.message}</span>}
        </label>

        <div className="actions">
          <button type="submit" disabled={mutation.isPending}>
            {mutation.isPending ? "Verifying…" : "Verify"}
          </button>
          <button type="button" className="secondary" onClick={onBack}>
            Back to login
          </button>
        </div>

        {mutation.isError && (
          <p className="error">{errorMessage(mutation.error)}</p>
        )}
      </form>
    </section>
  );
}
