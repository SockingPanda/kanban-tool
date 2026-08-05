import { ApiTransport } from "../../transport"
import { transition } from "./transition"
import type { RequestOptions, Task } from "../../types"
export async function releaseTask(api: ApiTransport, task: Task, claimToken: string, options: RequestOptions = {}): Promise<Task> {
    return transition(api, task, "release", { claim_token: claimToken }, options) as Promise<Task>
  }
