import { useQuery } from "@tanstack/react-query";
import { errorMessage, getMyTasks } from "../api";
import type { User } from "../api";
import CreateTaskForm from "./CreateTaskForm";

type Props = {
  user: User;
  onLogout: () => void;
};

export default function Staff({ user, onLogout }: Props) {
  // staleTime: Infinity => fetch once on load, never refetch on its own.
  // This keeps `cache.hit` meaningful: first load is false, the manual refetch is true.
  const query = useQuery({
    queryKey: ["my-tasks"],
    queryFn: getMyTasks,
    staleTime: Infinity,
  });
  const data = query.data;

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
        <h2>My tasks</h2>

        {query.isPending && <p>Loading tasks…</p>}
        {query.isError && <p className="error">{errorMessage(query.error)}</p>}

        {data && (
          <>
            <p>Total assigned tasks: {data.summary.total_assigned_tasks}</p>
            <p>Cache hit: {String(data.cache.hit)}</p>

            {data.tasks.length === 0 ? (
              <p>No tasks assigned</p>
            ) : (
              <ul className="list">
                {data.tasks.map((task) => (
                  <li key={task.id} className="row">
                    <span className="title">{task.title}</span>
                    <span className="meta">{task.status}</span>
                    <span className="meta">{task.priority}</span>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}

        <div className="actions">
          <button
            type="button"
            disabled={query.isFetching}
            onClick={() => query.refetch()}
          >
            {query.isFetching ? "Fetching…" : "Fetch tasks again"}
          </button>
        </div>
      </section>

      <section className="card">
        <h2>Try creating a task</h2>
        <CreateTaskForm />
      </section>
    </>
  );
}
