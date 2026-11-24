import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { assignTasks, errorMessage, getTasks } from "../api";
import type { User } from "../api";
import CreateTaskForm from "./CreateTaskForm";

const REQUIRED_COUNT = 3;

type Props = {
  user: User;
  onLogout: () => void;
};

export default function Admin({ user, onLogout }: Props) {
  const queryClient = useQueryClient();
  const [selected, setSelected] = useState<string[]>([]);

  const tasksQuery = useQuery({ queryKey: ["tasks"], queryFn: getTasks });
  const tasks = tasksQuery.data ?? [];

  const assignMutation = useMutation({
    mutationFn: assignTasks,
    onSuccess: () => {
      setSelected([]);
      queryClient.invalidateQueries({ queryKey: ["tasks"] });
    },
  });

  function toggle(id: string) {
    assignMutation.reset();
    setSelected((prev) =>
      prev.includes(id)
        ? prev.filter((taskId) => taskId !== id)
        : [...prev, id],
    );
  }

  return (
    <>
      <header className="bar">
        <div>
          <strong>{user.email}</strong>{" "}
          <span className="badge">{user.role}</span>
        </div>
        <button type="button" className="secondary" onClick={onLogout}>
          Logout
        </button>
      </header>

      <section className="card">
        <h2>Create a task</h2>
        <CreateTaskForm />
      </section>

      <section className="card">
        <h2>All tasks</h2>

        {tasksQuery.isPending && <p>Loading tasks…</p>}
        {tasksQuery.isError && (
          <p className="error">{errorMessage(tasksQuery.error)}</p>
        )}

        {tasksQuery.isSuccess && (
          <>
            <p className="hint">Tasks: {tasks.length}</p>

            {tasks.length === 0 ? (
              <p>No tasks yet</p>
            ) : (
              <ul className="list">
                {tasks.map((task) => (
                  <li key={task.id} className="row">
                    <label className="check">
                      <input
                        type="checkbox"
                        checked={selected.includes(task.id)}
                        onChange={() => toggle(task.id)}
                      />
                      <span className="title">{task.title}</span>
                    </label>
                    <span className="meta">{task.status}</span>
                    <span className="meta">{task.priority}</span>
                    <span className="meta">
                      {task.assigned_to_id ? "Assigned" : "Unassigned"}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}

        <div className="actions">
          <button
            type="button"
            disabled={
              selected.length !== REQUIRED_COUNT || assignMutation.isPending
            }
            onClick={() => assignMutation.mutate(selected)}
          >
            {assignMutation.isPending
              ? "Assigning…"
              : "Assign selected to James Bond"}
          </button>
          <span className="hint">
            Selected: {selected.length} / {REQUIRED_COUNT}
          </span>
        </div>

        {assignMutation.isSuccess && (
          <p className="success">3 tasks assigned to James Bond</p>
        )}
        {assignMutation.isError && (
          <p className="error">{errorMessage(assignMutation.error)}</p>
        )}
      </section>
    </>
  );
}
