import { Part } from './part';
import { AgentType } from './agent';

export interface TaskProgressItem {
  id: string;
  name: string;
  status: string;
}

export type SseEvent =
  | { type: 'message'; content: string }
  | { type: 'part'; part: Part }
  | { type: 'agent_info'; agent_type: AgentType; agent_name: string }
  | { type: 'task_progress'; plan_id: string; tasks: TaskProgressItem[] }
  | { type: 'done'; session_id: string }
  | { type: 'error'; message: string };
