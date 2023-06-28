import neo4j, { Session } from 'neo4j-driver'

const driver = neo4j.driver(
  'bolt://localhost:7687',
  neo4j.auth.basic('neo4j', 'password'),
  { disableLosslessIntegers: true },
)
const session = driver.session()

await session.run('MATCH (n) DETACH DELETE n')
await session.run(`
  CREATE
    (:Video {name: "long", interviewID: 26}),
    (:Video {name: "kramer", interviewID: 35}),
    (:Video {name: "crimp", interviewID: 74})
`)

const createIndexQuery = `
  CREATE FULLTEXT INDEX search_interview_lines IF NOT EXISTS
  FOR (l:Line) ON EACH [l.text]
  OPTIONS {
    indexConfig: {
      \`fulltext.analyzer\`: 'english'
    }
  }
`

try {
  await session.run(createIndexQuery)
} catch (err) {
  console.error(err)
}
