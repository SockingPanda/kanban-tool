/** Freeze the effective owner when a maintenance confirmation intent is created. */
export function maintenanceOwnerForAction(input: string, actor: string, runtimeActor: string): string {
  return input.trim() || actor.trim() || runtimeActor
}
