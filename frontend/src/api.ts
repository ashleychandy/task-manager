import axios from "axios";

// ---------- Constants ----------

export const ASSIGNEE_EMAIL = "jamesbond@example.com";

// ---------- Types ----------

export type Role = "admin" | "staff";
export type TaskStatus = "todo" | "in_progress" | "done";
export type TaskPriority = "low" | "medium" | "high";

export interface User {
  email: string;
  role: Role;
}

export interface Task {
  id: string;
  title: string;
  description: string;
  status: TaskStatus;
  priority: TaskPriority;
  created_by_id: string;
  assigned_to_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface MyTask {
  id: string;
  title: string;
  status: TaskStatus;
  priority: TaskPriority;
  assigned_to: string | null;
}

export interface LoginResponse {
  message: string;
  login_challenge_id: string;
  expires_in: number;
}

export interface VerifyResponse {
  access_token: string;
  user: User;
}

export interface CreateTaskInput {
  title: string;
  description: string;
  status: TaskStatus;
  priority: TaskPriority;
}

export interface AssignResponse {
  assigned: number;
  assignee_email: string;
}

export interface MyTasksResponse {
  user: User;
  tasks: MyTask[];
  summary: { total_assigned_tasks: number };
  cache: { hit: boolean };
}

// ---------- Auth token (memory only) ----------

let authToken: string | null = null;

export function setAuthToken(token: string | null) {
  authToken = token;
}

// ---------- Axios instance ----------

const client = axios.create({
  baseURL: import.meta.env.VITE_API_BASE_URL,
});

client.interceptors.request.use((config) => {
  if (authToken) {
    config.headers.Authorization = `Bearer ${authToken}`;
  }
  return config;
});

// ---------- API functions ----------

export async function login(email: string, password: string) {
  const res = await client.post<LoginResponse>("/auth/login", {
    email,
    password,
  });
  return res.data;
}

export async function verify2fa(loginChallengeId: string, code: string) {
  const res = await client.post<VerifyResponse>("/auth/verify-2fa", {
    login_challenge_id: loginChallengeId,
    code,
  });
  return res.data;
}

export async function createTask(input: CreateTaskInput) {
  const res = await client.post<Task>("/tasks", input);
  return res.data;
}

export async function getTasks() {
  const res = await client.get<{ tasks: Task[] }>("/tasks");
  return res.data.tasks;
}

export async function assignTasks(taskIds: string[]) {
  const res = await client.post<AssignResponse>("/tasks/assign", {
    task_ids: taskIds,
    assignee_email: ASSIGNEE_EMAIL,
  });
  return res.data;
}

export async function getMyTasks() {
  const res = await client.get<MyTasksResponse>("/tasks/view-my-tasks");
  return res.data;
}

// ---------- Error helpers ----------

// Returns the backend's {"error": "..."} message, or a generic fallback.
export function errorMessage(err: unknown): string {
  if (axios.isAxiosError<{ error?: string }>(err)) {
    const message = err.response?.data?.error;
    if (message) return message;
  }
  return "Something went wrong. Please try again.";
}

// Lets components check for a 403 without importing Axios.
export function isForbidden(err: unknown): boolean {
  return axios.isAxiosError(err) && err.response?.status === 403;
}
