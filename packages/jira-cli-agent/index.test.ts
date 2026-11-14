import assert from "node:assert/strict"
import { test } from "node:test"
import { fileURLToPath } from "node:url"
import { server } from "./index.ts"

const skillsDir = fileURLToPath(new URL("./skills", import.meta.url))

const run = async (config: Record<string, any>) => {
  const hooks = await server()
  await hooks.config(config)
  return config
}

test("an empty config gets the default jira-cli server and the skills directory", async () => {
  const config = await run({})
  assert.deepEqual(config.mcp["jira-cli"], { type: "local", command: ["jira", "mcp"] })
  assert.deepEqual(config.skills.paths, [skillsDir])
})

test("a user jira-cli entry is left unchanged", async () => {
  const user = { type: "local", command: ["/opt/jira", "mcp"], environment: { A: "1" }, timeout: 20000, enabled: false }
  const config = await run({ mcp: { "jira-cli": structuredClone(user) } })
  assert.deepEqual(config.mcp["jira-cli"], user)
})

test("another user MCP entry is left unchanged", async () => {
  const other = { type: "local", command: ["x"] }
  const config = await run({ mcp: { other: structuredClone(other) } })
  assert.deepEqual(config.mcp.other, other)
  assert.deepEqual(config.mcp["jira-cli"], { type: "local", command: ["jira", "mcp"] })
})

test("existing skills paths and keys are kept and the directory is appended", async () => {
  const config = await run({ skills: { paths: ["/a"], urls: ["u"] } })
  assert.deepEqual(config.skills, { paths: ["/a", skillsDir], urls: ["u"] })
})

test("running the hook twice lists the skills directory once", async () => {
  const config = await run({})
  await run(config)
  assert.deepEqual(config.skills.paths, [skillsDir])
})
