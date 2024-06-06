# `select_policy` lambda

In the `send-transaction-to-approvers` workflow we need to, based on the incoming transaction that needs to be signed,
determine which Policy are we going to use. Eventually, we'll make use of a dedicated component to
hold this logic and which will be called by this lambda. For now, we'll make a first version that just
queries the policy to be used from a Launch Darkly Flag. This policy is then passed on to the
`maestro_fetch_policy` lambda which will be in charge of querying the Policy from the Maestro API.

#### This lambda is intended to be called as part of a workflow.

# Input

No input

# Output

```json
{
  "policy_name": "some.policy.name"
}
```