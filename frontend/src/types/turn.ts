import { Part } from './part';

export interface Turn {
  id: string;
  userMessage: string;
  parts: Part[];
  isComplete: boolean;
  timestamp: number;
}
