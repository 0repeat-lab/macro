# Workspace product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Employees manage assigned work and collaborate across tasks, projects, conversations, documents, email and calendars. Administrators oversee delivery and access. HR-authorized staff manage employment workflows. Customers require explicitly scoped access to their own approved project information.

## Product Purpose

Provide one connected employee workspace with fast task capture, understandable navigation, context-preserving detail editing and integrated business workflows. Extend the existing collaboration product rather than duplicate its document, conversation, search or permission engines.

## Operating Context

This fork is being evaluated with existing Frappe ERP and HR sites, a Stalwart mail/calendar server and a restricted customer portal. Those services already contain business data. Their records must survive the introduction of the new workspace. Native HR and Stalwart integration are planned additions, not verified upstream capabilities.

## Capabilities and Constraints

- Employee, administrator and customer access remains explicit. A matching email domain does not grant an administrator or HR role.
- HR screens use Frappe-backed records; payroll facts and employment details are never inferred by AI.
- Actual hours remain distinct from estimates, attendance and approved financial records.
- Provider integration and shared sign-in are separate capabilities and require separate validation.
- Keep customer reporting isolated from internal team discussions and private HR or mailbox content.
- Preserve existing virtualization, optimistic updates, saved views and collaboration mechanisms where they satisfy the desired behavior.
- Product performance must be measured against the real application; marketing captures and fixture results are not production benchmarks.

## Brand Commitments

The user prefers ClickUp-like task-management usability and a modern, smooth, easy-to-use workspace across other surfaces. This is an interaction reference; it is not an instruction to copy brand assets or recreate every ClickUp feature.

## Evidence on Hand

- Existing task implementation: `src/features/tasks-view/`.
- Shared board interaction implementation: `src/components/kanban/`.
- Committed task captures: `../../.github/readme/tasks-list.png` and `../../.github/readme/framed/tasks.png`.
- The screenshot maintenance notes explicitly identify older task controls. Use current rendered components to establish the implementation baseline.
- No live runtime/performance measurement has been completed for this fork yet.

## Product Principles

1. Keep the next useful action obvious and preserve a person's place while they act.
2. Reuse a single source of truth for each business record.
3. Grant access by verified identity, membership and capability, with hard privacy boundaries.
4. Make pending, failed and offline operations visible and recoverable without duplicate effects.
5. Keep the maintained fork small by isolating deployment-specific adapters and upstreaming general improvements where accepted.

## Open Decisions

Proposed initial task default: compact list, with a board alternative and each person's later view/density choice remembered. The user confirmed task management and wider workspace UX as the scope, but did not select a particular default view. Final deployment capacity, runtime domain, licensing clarification and measured performance budgets remain implementation gates.
