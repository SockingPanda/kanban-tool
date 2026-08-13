const modalFocusableSelector = "button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])"

/**
 * 让焦点陷阱与浏览器 Tab 顺序一致。原生 `details` 在关闭时仍保留
 * `summary` 的焦点能力，但会隐藏其余后代；隐藏后代不能成为陷阱边界。
 */
export function modalFocusableElements(root: ParentNode): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(modalFocusableSelector)).filter((element) => {
    const closedDetails = element.closest("details:not([open])")
    if (closedDetails !== null && element.tagName !== "SUMMARY") return false
    return element.closest("[hidden], [aria-hidden='true'], [inert]") === null
  })
}
