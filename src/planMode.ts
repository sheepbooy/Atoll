import {
  type IslandSnapshot,
  type PermissionRequest,
} from "./tauri";

export function isPlanModeCommand(command: string): boolean {
  return (
    command.startsWith("AskUserQuestion:") ||
    command === "AskUserQuestion" ||
    command.startsWith("ExitPlanMode:") ||
    command === "ExitPlanMode"
  );
}

export function snapshotHasPlanPending(snapshot: IslandSnapshot): boolean {
  return snapshot.recent.some(
    (request) => request.status === "pending" && isPlanModeCommand(request.command),
  );
}

export function getPlanModeType(request: PermissionRequest): "question" | "exitPlan" | null {
  if (isPlanModeCommand(request.command)) {
    if (
      request.command.startsWith("AskUserQuestion:") ||
      request.command === "AskUserQuestion"
    ) {
      return "question";
    }
    return "exitPlan";
  }
  return null;
}

export interface PlanQuestionOption {
  label: string;
  description: string;
}

export interface PlanQuestion {
  question: string;
  header: string;
  options: PlanQuestionOption[];
  multiSelect: boolean;
}

export interface PlanQuestionCardProps {
  request: PermissionRequest;
  onResolve: (snapshot: IslandSnapshot) => void;
}

export function parsePlanContent(toolInput: unknown): string | null {
  if (!toolInput || typeof toolInput !== "object") {
    return null;
  }
  const plan = (toolInput as { plan?: unknown }).plan;
  if (typeof plan !== "string") {
    return null;
  }
  const trimmed = plan.trim();
  return trimmed.length > 0 ? trimmed : null;
}

export function parsePlanQuestions(toolInput: unknown): PlanQuestion[] {
  if (!toolInput || typeof toolInput !== "object") {
    return [];
  }
  const questions = (toolInput as { questions?: unknown }).questions;
  if (!Array.isArray(questions)) {
    return [];
  }
  return questions.flatMap((entry) => {
    if (!entry || typeof entry !== "object") {
      return [];
    }
    const record = entry as Record<string, unknown>;
    const question = typeof record.question === "string" ? record.question : "";
    const header = typeof record.header === "string" ? record.header : "";
    const multiSelect = Boolean(record.multiSelect);
    const options = Array.isArray(record.options)
      ? record.options.flatMap((option) => {
          if (!option || typeof option !== "object") {
            return [];
          }
          const optionRecord = option as Record<string, unknown>;
          const label = typeof optionRecord.label === "string" ? optionRecord.label : "";
          const description =
            typeof optionRecord.description === "string" ? optionRecord.description : "";
          if (!label) {
            return [];
          }
          return [{ label, description }];
        })
      : [];
    if (!question || options.length === 0) {
      return [];
    }
    return [{ question, header, options, multiSelect }];
  });
}

export function getOriginalQuestions(toolInput: unknown): unknown[] {
  if (!toolInput || typeof toolInput !== "object") return [];
  const questions = (toolInput as { questions?: unknown }).questions;
  return Array.isArray(questions) ? questions : [];
}

export const OTHER_SENTINEL = "__atoll_other__";
