import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useForm } from "react-hook-form";
import { createTask, errorMessage, isForbidden } from "../api";
import type { CreateTaskInput, TaskPriority, TaskStatus } from "../api";

const STATUSES: TaskStatus[] = ["todo", "in_progress", "done"];
const PRIORITIES: TaskPriority[] = ["low", "medium", "high"];

export default function CreateTaskForm() {
  const queryClient = useQueryClient();

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors },
  } = useForm<CreateTaskInput>({
    defaultValues: {
      title: "",
      description: "",
      status: "todo",
      priority: "medium",
    },
  });

  const mutation = useMutation({
    mutationFn: createTask,
    onSuccess: () => {
      reset();
      queryClient.invalidateQueries({ queryKey: ["tasks"] });
    },
  });

  return (
    <form onSubmit={handleSubmit((values) => mutation.mutate(values))}>
      <label className="field">
        Title
        <input
          type="text"
          {...register("title", { required: "Title is required" })}
        />
        {errors.title && <span className="error">{errors.title.message}</span>}
      </label>

      <label className="field">
        Description
        <textarea rows={3} {...register("description")} />
      </label>

      <label className="field">
        Status
        <select {...register("status")}>
          {STATUSES.map((status) => (
            <option key={status} value={status}>
              {status}
            </option>
          ))}
        </select>
      </label>

      <label className="field">
        Priority
        <select {...register("priority")}>
          {PRIORITIES.map((priority) => (
            <option key={priority} value={priority}>
              {priority}
            </option>
          ))}
        </select>
      </label>

      <button type="submit" disabled={mutation.isPending}>
        {mutation.isPending ? "Creating…" : "Create task"}
      </button>

      {mutation.isSuccess && <p className="success">Task created</p>}
      {mutation.isError && (
        <p className="error">
          {isForbidden(mutation.error)
            ? "Blocked (403): only Admin can create tasks."
            : errorMessage(mutation.error)}
        </p>
      )}
    </form>
  );
}
